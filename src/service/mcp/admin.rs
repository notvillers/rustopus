//! Admin dashboard for the precache and the snapshot cache.
//!
//! ## This is a credential store, and is treated as one
//!
//! Precache entries hold live Octopus authcodes at rest, because the refresh job
//! runs with no user present. Three rules follow, and none of them is optional:
//!
//! 1. `/admin` has its **own** authentication — the `admin_token`, never a SOAP
//!    authcode. `main.rs` does not register this scope at all when no token is
//!    configured, so there is no unauthenticated path to it.
//! 2. The JSON API **never returns a full authcode** — not in a list, not in an
//!    edit form, not in an error. Entries are identified by a hash-derived id and
//!    displayed masked, which is why editing an entry's label or url uses `PATCH`
//!    against that id rather than a round-trip through the browser.
//! 3. `mcp_precache.toml` is written `0600` and mounted as a secret. See
//!    `DOCKER_PLAN.md`.
//!
//! Bind `/admin` to an internal interface or put it behind the VPN. The token is
//! the last line of defence, not the only one it deserves.
//!
//! ## It also manages the access blocklist
//!
//! `main.rs` registers this scope whenever an admin token is set, **not** only
//! when MCP is enabled, because the blocklist (`service/blocklist.rs`) applies to
//! the REST endpoints too and has to be manageable on the instance serving them.
//! On such an instance `AdminState::mcp_enabled` is false and every handler that
//! would touch the snapshot cache or the precache is skipped — reading them would
//! construct a cache on a process that is meant to hold none.

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse, Responder, Scope, web
};
use serde::Deserialize;
use serde_json::json;

use crate::service::{
    blocklist::{self, BlockRule, BlockScope},
    log::{elog_with_ip, log_with_ip},
    ipv4::log_ip,
    mcp::{
        cache::cache,
        oauth,
        precache::{self, PrecacheEntry},
        secrets_match,
        store
    }
};

/// Header accepted as an alternative to HTTP Basic, for curl and scripts.
const ADMIN_TOKEN_HEADER: &str = "X-Admin-Token";


/// Everything the admin handlers need, shared through actix app data.
#[derive(Clone)]
pub struct AdminState {
    token: String,
    static_dir: PathBuf,
    /// False on an instance running with `[mcp] enabled = false`, where the
    /// dashboard manages the blocklist and nothing else.
    mcp_enabled: bool
}

impl AdminState {
    pub fn new(token: String, static_dir: PathBuf, mcp_enabled: bool) -> Self {
        Self { token, static_dir, mcp_enabled }
    }
}


/// Pulls the presented token out of `X-Admin-Token` or HTTP Basic.
///
/// Basic is supported because a browser can supply it on the dashboard's own
/// asset requests, which a custom header cannot. The token never travels in a
/// query string — that would put it in every access log.
fn presented_token(request: &HttpRequest) -> Option<String> {
    if let Some(value) = request.headers().get(ADMIN_TOKEN_HEADER)
        && let Ok(token) = value.to_str() {
            return Some(token.trim().to_string())
    }

    let header = request.headers().get(actix_web::http::header::AUTHORIZATION)?;
    let value = header.to_str().ok()?;
    let encoded = value.strip_prefix("Basic ")?;
    let decoded = base64_decode(encoded.trim())?;
    let decoded = String::from_utf8(decoded).ok()?;
    // "user:password" — the username is ignored, the password is the token.
    Some(decoded.split_once(':').map_or(decoded.clone(), |(_, password)| password.to_string()))
}


/// Minimal base64 decoder for the Basic credentials.
///
/// Hand-rolled to avoid a dependency for one 40-character string; returns `None`
/// on anything malformed rather than guessing.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

    let mut buffer: u32 = 0;
    let mut bits = 0_u32;
    let mut output = Vec::new();

    for byte in input.bytes() {
        if byte == b'=' {
            break
        }
        let value = ALPHABET.iter().position(|candidate| *candidate == byte)? as u32;
        buffer = (buffer << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}


/// 401 with the challenge a browser needs to show its credential prompt.
fn unauthorized() -> HttpResponse {
    HttpResponse::Unauthorized()
        .insert_header(("WWW-Authenticate", "Basic realm=\"Rustopus admin\", charset=\"UTF-8\""))
        .content_type("text/plain")
        .body("Admin token required")
}


/// Checks the caller's token, logging rejections with the source address.
///
/// Returns the 401 response to send, so handlers read as
/// `if let Some(denied) = guard(..) { return denied }`.
async fn guard(request: &HttpRequest, state: &AdminState) -> Option<HttpResponse> {
    let allowed = presented_token(request)
        .is_some_and(|presented| secrets_match(&presented, &state.token));

    if allowed {
        return None
    }

    let ip_address = log_ip(request.clone()).await.to_string();
    elog_with_ip(&ip_address, "ADMIN: rejected request without a valid admin token");
    Some(unauthorized())
}


/// Body for creating or replacing an entry. The authcode is required here — it
/// is the only direction a secret ever travels.
#[derive(Debug, Deserialize)]
pub struct NewEntry {
    pub label: String,
    pub authcode: String,
    pub pid: i64,
    pub url: Option<String>,
    pub enabled: Option<bool>
}

/// Body for editing an existing entry. Deliberately has no `authcode` field: the
/// browser never received one, so it cannot send one back. Changing an authcode
/// means creating an entry and deleting the old one.
#[derive(Debug, Deserialize)]
pub struct EntryPatch {
    pub label: Option<String>,
    pub url: Option<String>,
    pub enabled: Option<bool>
}


/// Refuses a precache/cache operation on an instance where MCP is off. Those
/// handlers would otherwise build the snapshot cache in a process configured to
/// hold none.
fn require_mcp(state: &AdminState) -> Option<HttpResponse> {
    if state.mcp_enabled {
        return None
    }
    Some(HttpResponse::BadRequest().json(json!({
        "error": "MCP is disabled on this instance ([mcp] enabled = false); only the blocklist is manageable here"
    })))
}


/// One row per blocking rule, with its in-memory hit counters.
fn blocks_payload() -> Vec<serde_json::Value> {
    let hits = blocklist::hits();
    blocklist::rules().iter().map(|rule| {
        let id = rule.id();
        let hit = hits.get(&id).cloned().unwrap_or_default();
        json!({
            "id": id,
            "kind": rule.kind.as_str(),
            // For an authcode rule this is the mask, never the code — the hash
            // behind it is not published either, since it is a working offline
            // guess target and the dashboard has no use for it.
            "label": rule.label,
            "note": rule.note,
            "scope": rule.scope().as_str(),
            "enabled": rule.is_enabled(),
            "created_at": rule.created_at.map(|at| at.to_rfc3339()),
            "hits": hit.count,
            "last_hit": hit.last_at.map(|at| at.to_rfc3339()),
            "last_ip": hit.last_ip
        })
    }).collect()
}


/// Registered OAuth clients and the sign-ins they hold, or `null` when OAuth is
/// off — exactly as `cache` and `disk` are `null` on a REST-only instance.
///
/// No secret leaves here: a client's secret is stored hashed and was shown once
/// at creation, and a grant's authcode is rendered through `mask_authcode`.
fn oauth_payload() -> serde_json::Value {
    if !oauth::is_enabled() {
        return serde_json::Value::Null
    }

    let clients: Vec<serde_json::Value> = oauth::store::clients().iter().map(|client| json!({
        "client_id": client.client_id,
        "name": client.name,
        "redirect_uris": client.redirect_uris,
        "enabled": client.is_enabled(),
        "created_at": client.created_at.map(|at| at.to_rfc3339())
    })).collect();

    let names: std::collections::HashMap<String, String> = oauth::store::clients().into_iter()
        .map(|client| (client.client_id, client.name))
        .collect();
    let used = oauth::store::last_used();
    let precached: Vec<String> = precache::entries().iter().map(|entry| entry.id()).collect();

    let sessions: Vec<serde_json::Value> = oauth::store::grants().iter().map(|grant| json!({
        "id": grant.id,
        "client": names.get(&grant.client_id).cloned().unwrap_or_else(|| "(deleted client)".into()),
        // Masked, always — the same rule the precache rows follow.
        "authcode": grant.label,
        "pid": grant.pid,
        "created_at": grant.created_at.to_rfc3339(),
        "expires_at": grant.expires_at.to_rfc3339(),
        "last_used": used.get(&grant.id).map(|at| at.to_rfc3339()),
        // The obvious next click: a partner whose catalog is not kept warm pays
        // a cold build on their first question.
        "precached": precached.contains(&grant.precache_id())
    })).collect();

    json!({
        "enabled": true,
        "issuer": oauth::issuer(),
        "resource": oauth::resource_uri(),
        "allow_headers": oauth::allow_headers(),
        "clients": clients,
        "sessions": sessions
    })
}


/// Cache usage plus one row per configured entry, all authcodes masked, plus the
/// blocklist. On a non-MCP instance the cache and precache sections are `null`
/// and only `blocks` is populated.
async fn state_handler(request: HttpRequest, state: web::Data<AdminState>) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }

    if !state.mcp_enabled {
        return HttpResponse::Ok().json(json!({
            "mcp_enabled": false,
            "cache": null,
            "disk": null,
            "entries": [],
            "oauth": null,
            "blocks": blocks_payload()
        }))
    }

    // Settle moka's deferred maintenance so the usage figure reflects reality
    // rather than a queue of pending evictions.
    cache().settle().await;

    let stats = cache().stats();
    let runs = precache::runs();

    let entries: Vec<serde_json::Value> = precache::entries().iter().map(|entry| {
        let id = entry.id();
        let run = runs.get(&id).cloned().unwrap_or_default();
        let entry_stats = entry.cache_key().and_then(|key| stats.get(&key).cloned()).unwrap_or_default();

        json!({
            "id": id,
            "label": entry.label,
            // Masked, always. The full code is never serialized out of here.
            "authcode": crate::service::mcp::mask_authcode(&entry.authcode),
            "pid": entry.pid,
            "url": entry.url(),
            "enabled": entry.is_enabled(),
            "on_disk": entry.cache_key().is_some_and(|key| store::contains(&key)),
            "running": run.running,
            "last_run": run.last_run.map(|at| at.to_rfc3339()),
            "last_full_pull": run.last_full_pull.map(|at| at.to_rfc3339()),
            "last_duration_ms": run.last_duration_ms,
            "last_outcome": run.last_outcome,
            "bytes": entry_stats.bytes,
            "products": entry_stats.products,
            "hits": entry_stats.hits,
            "misses": entry_stats.misses,
            "hit_rate": entry_stats.hit_rate()
        })
    }).collect();

    let used = cache().used_bytes();
    let budget = cache().budget_bytes();
    let (disk_used, disk_count) = store::usage();
    let disk_budget = crate::service::config::get_mcp_settings().disk_max_bytes();

    HttpResponse::Ok().json(json!({
        "mcp_enabled": true,
        "blocks": blocks_payload(),
        "cache": {
            "used_bytes": used,
            "budget_bytes": budget,
            "usage_ratio": if budget == 0 { 0.0 } else { used as f64 / budget as f64 },
            "entries_held": cache().entry_count()
        },
        "disk": {
            "used_bytes": disk_used,
            "budget_bytes": disk_budget,
            "usage_ratio": if disk_budget == 0 { 0.0 } else { disk_used as f64 / disk_budget as f64 },
            "snapshots_stored": disk_count,
            "path": store::cache_dir().to_string_lossy()
        },
        "entries": entries,
        "oauth": oauth_payload()
    }))
}


/// Creates an entry, or replaces the one with the same `(authcode, pid)`.
async fn create_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    body: web::Json<NewEntry>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }

    let body = body.into_inner();
    if body.authcode.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({ "error": "authcode is required" }))
    }
    if body.label.trim().is_empty() {
        return HttpResponse::BadRequest().json(json!({ "error": "label is required" }))
    }

    let entry = PrecacheEntry {
        label: body.label,
        authcode: body.authcode,
        pid: body.pid,
        url: body.url,
        enabled: body.enabled
    };
    let id = entry.id();
    let masked = entry.masked();

    match precache::upsert(entry) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: precache entry saved [{}]", masked));
            HttpResponse::Ok().json(json!({ "id": id }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Edits the non-secret fields of an existing entry.
async fn patch_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>,
    body: web::Json<EntryPatch>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }

    let id = path.into_inner();
    let Some(mut entry) = precache::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such entry" }))
    };

    let body = body.into_inner();
    if let Some(label) = body.label.filter(|label| !label.trim().is_empty()) {
        entry.label = label;
    }
    if let Some(url) = body.url {
        entry.url = if url.trim().is_empty() { None } else { Some(url) };
    }
    if let Some(enabled) = body.enabled {
        entry.enabled = Some(enabled);
    }

    let masked = entry.masked();
    match precache::upsert(entry) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: precache entry updated [{}]", masked));
            HttpResponse::Ok().json(json!({ "id": id }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Removes an entry and drops whatever it had warmed.
async fn delete_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }

    let id = path.into_inner();
    let Some(entry) = precache::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such entry" }))
    };

    if let Some(key) = entry.cache_key() {
        cache().invalidate(&key).await;
    }
    let masked = entry.masked();

    match precache::remove(&id) {
        Ok(_) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: precache entry removed [{}]", masked));
            HttpResponse::Ok().json(json!({ "removed": true }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Refreshes one entry immediately, with a full pull.
///
/// Runs inline rather than detached so the dashboard can report the real
/// outcome; a full catalog build takes tens of seconds, well inside the server's
/// 1200s request timeout.
async fn refresh_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }

    let id = path.into_inner();
    let Some(entry) = precache::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such entry" }))
    };

    let ip_address = log_ip(request.clone()).await.to_string();
    log_with_ip(&ip_address, format!("ADMIN: manual refresh requested [{}]", entry.masked()));

    match precache::refresh(&entry, true).await {
        Ok(()) => HttpResponse::Ok().json(json!({ "refreshed": true })),
        Err(error) => HttpResponse::BadGateway().json(json!({ "error": error }))
    }
}


/// Drops one entry's cached snapshot without removing its configuration.
async fn evict_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }

    let id = path.into_inner();
    let Some(entry) = precache::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such entry" }))
    };

    match entry.cache_key() {
        Some(key) => {
            cache().invalidate(&key).await;
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: cache entry evicted [{}]", entry.masked()));
            HttpResponse::Ok().json(json!({ "evicted": true }))
        }
        None => HttpResponse::BadRequest().json(json!({ "error": "entry has no configured url" }))
    }
}


/// Body for adding a blocking rule.
///
/// `value` is an IP address or CIDR range for `kind = "ip"`, and the **full**
/// authcode for `kind = "authcode"` — the only direction a code travels here.
/// It is hashed on arrival and not retained; the rule that comes back is
/// identified by its mask.
#[derive(Debug, Deserialize)]
pub struct NewBlock {
    pub kind: String,
    pub value: String,
    pub note: Option<String>,
    pub scope: Option<String>
}

/// Body for editing a rule. No `value`: what a rule matches on defines its id,
/// so changing it means adding a rule and removing the old one.
#[derive(Debug, Deserialize)]
pub struct BlockPatch {
    pub note: Option<String>,
    pub scope: Option<String>,
    pub enabled: Option<bool>
}


/// Parses the scope name, defaulting to "everything" when it is absent.
fn parse_scope(scope: &Option<String>) -> Result<Option<BlockScope>, String> {
    let Some(scope) = scope.as_ref().map(|scope| scope.trim().to_lowercase()) else {
        return Ok(None)
    };
    match scope.as_str() {
        "" | "all" => Ok(Some(BlockScope::All)),
        "rest" => Ok(Some(BlockScope::Rest)),
        "mcp" => Ok(Some(BlockScope::Mcp)),
        other => Err(format!("unknown scope '{}' — use all, rest or mcp", other))
    }
}


/// Adds a blocking rule, or replaces the one matching the same thing.
async fn block_create_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    body: web::Json<NewBlock>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }

    let body = body.into_inner();
    let scope = match parse_scope(&body.scope) {
        Ok(scope) => scope,
        Err(error) => return HttpResponse::BadRequest().json(json!({ "error": error }))
    };
    let note = body.note
        .map(|note| note.trim().to_string())
        .filter(|note| !note.is_empty());

    let rule = match body.kind.trim().to_lowercase().as_str() {
        "ip" => BlockRule::ip(&body.value, note, scope),
        "authcode" => BlockRule::authcode(&body.value, note, scope),
        other => Err(format!("unknown kind '{}' — use ip or authcode", other))
    };
    let rule = match rule {
        Ok(rule) => rule,
        Err(error) => return HttpResponse::BadRequest().json(json!({ "error": error }))
    };

    let id = rule.id();
    // The label, not the value: for an authcode rule the value is a hash and
    // for an IP rule the two are the same thing.
    let described = format!("{} '{}' ({})", rule.kind.as_str(), rule.label, rule.scope().as_str());

    match blocklist::upsert(rule) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: block rule added [{}]", described));
            HttpResponse::Ok().json(json!({ "id": id }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Edits a rule's note, scope or enabled flag.
async fn block_patch_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>,
    body: web::Json<BlockPatch>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }

    let id = path.into_inner();
    let Some(mut rule) = blocklist::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such rule" }))
    };

    let body = body.into_inner();
    if body.scope.is_some() {
        match parse_scope(&body.scope) {
            Ok(scope) => rule.scope = scope,
            Err(error) => return HttpResponse::BadRequest().json(json!({ "error": error }))
        }
    }
    if let Some(note) = body.note {
        let note = note.trim().to_string();
        rule.note = if note.is_empty() { None } else { Some(note) };
    }
    if let Some(enabled) = body.enabled {
        rule.enabled = Some(enabled);
    }

    let described = format!(
        "{} '{}' ({}, {})",
        rule.kind.as_str(),
        rule.label,
        rule.scope().as_str(),
        if rule.is_enabled() { "enforced" } else { "paused" }
    );

    match blocklist::upsert(rule) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: block rule updated [{}]", described));
            HttpResponse::Ok().json(json!({ "id": id }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Removes a rule, letting whoever it matched back in.
async fn block_delete_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }

    let id = path.into_inner();
    let Some(rule) = blocklist::find(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such rule" }))
    };
    let described = format!("{} '{}'", rule.kind.as_str(), rule.label);

    match blocklist::remove(&id) {
        Ok(_) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: block rule removed [{}]", described));
            HttpResponse::Ok().json(json!({ "removed": true }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Body for registering a connector. No secret arrives here — the server mints
/// it, returns it once and keeps only its hash.
#[derive(Debug, Deserialize)]
pub struct NewOauthClient {
    pub name: String,
    pub redirect_uris: Vec<String>
}

/// Body for editing a connector. No `client_id`, and no way to read or reset the
/// secret: a lost secret means a new client.
#[derive(Debug, Deserialize)]
pub struct OauthClientPatch {
    pub name: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub enabled: Option<bool>
}


/// Refuses an OAuth operation on an instance where OAuth is off, for the same
/// reason [`require_mcp`] exists: the state it would report does not exist.
fn require_oauth() -> Option<HttpResponse> {
    if oauth::is_enabled() {
        return None
    }
    Some(HttpResponse::BadRequest().json(json!({
        "error": "OAuth is disabled on this instance ([mcp] oauth_enabled = false)"
    })))
}


/// Registers a connector and returns its credentials.
///
/// **The only response in this service that ever carries a client secret.** It is
/// stored hashed, so it cannot be shown again; losing it means creating another
/// client.
async fn oauth_client_create_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    body: web::Json<NewOauthClient>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }
    if let Some(denied) = require_oauth() {
        return denied
    }

    let body = body.into_inner();
    let name = body.name.trim().to_string();
    if name.is_empty() {
        return HttpResponse::BadRequest().json(json!({ "error": "name is required" }))
    }

    let redirect_uris: Vec<String> = body.redirect_uris.iter()
        .map(|uri| uri.trim().to_string())
        .filter(|uri| !uri.is_empty())
        .collect();
    if redirect_uris.is_empty() {
        return HttpResponse::BadRequest().json(json!({
            "error": "at least one redirect URI is required — for a claude.ai connector these are \
                      https://claude.ai/api/mcp/auth_callback and https://claude.com/api/mcp/auth_callback"
        }))
    }
    // A redirect URI is where a browser is sent with an authorization code. One
    // that is not absolute would be matched against nothing useful.
    if let Some(bad) = redirect_uris.iter().find(|uri| !uri.starts_with("https://") && !uri.starts_with("http://")) {
        return HttpResponse::BadRequest().json(json!({
            "error": format!("'{}' is not an absolute http(s) URI", bad)
        }))
    }

    let secret = oauth::new_secret();
    let client = oauth::OauthClient {
        client_id: uuid::Uuid::new_v4().simple().to_string(),
        name: name.clone(),
        secret_hash: oauth::hash_secret(&secret),
        redirect_uris,
        created_at: Some(chrono::Utc::now()),
        enabled: Some(true)
    };
    let client_id = client.client_id.clone();

    match oauth::store::upsert_client(client) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: OAuth client registered '{}'", name));
            HttpResponse::Ok().json(json!({
                "client_id": client_id,
                "client_secret": secret,
                "note": "This is the only time the secret is shown. Paste both values into the connector's Advanced settings now."
            }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Renames a connector, edits its redirect URIs, or disables it.
async fn oauth_client_patch_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>,
    body: web::Json<OauthClientPatch>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }
    if let Some(denied) = require_oauth() {
        return denied
    }

    let client_id = path.into_inner();
    let Some(mut client) = oauth::store::find_client(&client_id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such client" }))
    };

    let body = body.into_inner();
    if let Some(name) = body.name.filter(|name| !name.trim().is_empty()) {
        client.name = name.trim().to_string();
    }
    if let Some(uris) = body.redirect_uris {
        let uris: Vec<String> = uris.iter().map(|uri| uri.trim().to_string()).filter(|uri| !uri.is_empty()).collect();
        if uris.is_empty() {
            return HttpResponse::BadRequest().json(json!({ "error": "at least one redirect URI is required" }))
        }
        client.redirect_uris = uris;
    }
    if let Some(enabled) = body.enabled {
        client.enabled = Some(enabled);
    }

    let described = format!("{} ({})", client.name, if client.is_enabled() { "enabled" } else { "disabled" });
    match oauth::store::upsert_client(client) {
        Ok(()) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: OAuth client updated [{}]", described));
            HttpResponse::Ok().json(json!({ "client_id": client_id }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Removes a connector, and with it every sign-in it holds.
async fn oauth_client_delete_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }
    if let Some(denied) = require_oauth() {
        return denied
    }

    let client_id = path.into_inner();
    let Some(client) = oauth::store::find_client(&client_id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such client" }))
    };
    let name = client.name.clone();

    match oauth::store::remove_client(&client_id) {
        Ok(_) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: OAuth client removed '{}' and every sign-in it held", name));
            HttpResponse::Ok().json(json!({ "removed": true }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Revokes one sign-in. Its access tokens stop working in the same call.
async fn oauth_session_delete_handler(
    request: HttpRequest,
    state: web::Data<AdminState>,
    path: web::Path<String>
) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
    }
    if let Some(denied) = require_mcp(&state) {
        return denied
    }
    if let Some(denied) = require_oauth() {
        return denied
    }

    let id = path.into_inner();
    let Some(grant) = oauth::store::find_grant(&id) else {
        return HttpResponse::NotFound().json(json!({ "error": "no such sign-in" }))
    };
    let masked = grant.masked();

    match oauth::store::remove_grant(&id) {
        Ok(_) => {
            let ip_address = log_ip(request.clone()).await.to_string();
            log_with_ip(&ip_address, format!("ADMIN: OAuth sign-in revoked [{}]", masked));
            HttpResponse::Ok().json(json!({ "removed": true }))
        }
        Err(error) => HttpResponse::InternalServerError().json(json!({ "error": error }))
    }
}


/// Serves one of the dashboard's own files, behind the same token check as the
/// API — the page itself is part of the protected surface, not public chrome.
async fn asset(request: &HttpRequest, state: &AdminState, name: &str) -> HttpResponse {
    if let Some(denied) = guard(request, state).await {
        return denied
    }
    match NamedFile::open_async(state.static_dir.join(name)).await {
        Ok(file) => file.into_response(request),
        Err(_) => HttpResponse::NotFound().content_type("text/plain").body("Not found")
    }
}

async fn index_handler(request: HttpRequest, state: web::Data<AdminState>) -> impl Responder {
    asset(&request, &state, "index.html").await
}

async fn script_handler(request: HttpRequest, state: web::Data<AdminState>) -> impl Responder {
    asset(&request, &state, "admin.js").await
}

async fn style_handler(request: HttpRequest, state: web::Data<AdminState>) -> impl Responder {
    asset(&request, &state, "admin.css").await
}


/// The `/admin` scope.
///
/// Like `/mcp`, this is mounted as a scope rather than through the repo's
/// `get`/`get_alias` route pattern: it is a small internal application with its
/// own authentication, not a public fetcher endpoint, so there is no plural
/// alias to register.
pub fn scope(state: AdminState) -> Scope {
    web::scope("/admin")
        .app_data(web::Data::new(state))
        .route("", web::get().to(index_handler))
        .route("/", web::get().to(index_handler))
        .route("/admin.js", web::get().to(script_handler))
        .route("/admin.css", web::get().to(style_handler))
        .route("/api/state", web::get().to(state_handler))
        .route("/api/entries", web::post().to(create_handler))
        .route("/api/entries/{id}", web::patch().to(patch_handler))
        .route("/api/entries/{id}", web::delete().to(delete_handler))
        .route("/api/entries/{id}/refresh", web::post().to(refresh_handler))
        .route("/api/entries/{id}/evict", web::post().to(evict_handler))
        // Blocklist. Unlike the routes above these work with MCP disabled — the
        // rules they manage protect the REST endpoints too.
        .route("/api/blocks", web::post().to(block_create_handler))
        .route("/api/blocks/{id}", web::patch().to(block_patch_handler))
        .route("/api/blocks/{id}", web::delete().to(block_delete_handler))
        // OAuth connectors and the sign-ins they hold. Registered whatever the
        // configuration says and refused with a 400 when OAuth is off, like the
        // precache routes on a REST-only instance.
        .route("/api/oauth/clients", web::post().to(oauth_client_create_handler))
        .route("/api/oauth/clients/{id}", web::patch().to(oauth_client_patch_handler))
        .route("/api/oauth/clients/{id}", web::delete().to(oauth_client_delete_handler))
        .route("/api/oauth/sessions/{id}", web::delete().to(oauth_session_delete_handler))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_credentials_decode_to_the_password() {
        // "admin:s3cret-token"
        let decoded = base64_decode("YWRtaW46czNjcmV0LXRva2Vu").expect("decodes");
        let text = String::from_utf8(decoded).expect("utf-8");
        assert_eq!(text, "admin:s3cret-token");
    }

    #[test]
    fn malformed_base64_is_rejected_rather_than_guessed() {
        assert!(base64_decode("not base64!").is_none());
    }
}
