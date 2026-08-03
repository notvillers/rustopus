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

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::{
    HttpRequest, HttpResponse, Responder, Scope, web
};
use serde::Deserialize;
use serde_json::json;

use crate::service::{
    log::{elog_with_ip, log_with_ip},
    ipv4::log_ip,
    mcp::{
        cache::cache,
        precache::{self, PrecacheEntry},
        store
    }
};

/// Header accepted as an alternative to HTTP Basic, for curl and scripts.
const ADMIN_TOKEN_HEADER: &str = "X-Admin-Token";


/// Everything the admin handlers need, shared through actix app data.
#[derive(Clone)]
pub struct AdminState {
    token: String,
    static_dir: PathBuf
}

impl AdminState {
    pub fn new(token: String, static_dir: PathBuf) -> Self {
        Self { token, static_dir }
    }
}


/// Compares two secrets without leaking their common prefix through timing.
fn secrets_match(provided: &str, expected: &str) -> bool {
    let provided = provided.as_bytes();
    let expected = expected.as_bytes();
    // Length is not secret; comparing unequal lengths byte-wise would be, so
    // fold the length check into the same constant-time result.
    let mut difference = (provided.len() ^ expected.len()) as u8;
    for index in 0..provided.len().max(expected.len()) {
        let left = provided.get(index).copied().unwrap_or(0);
        let right = expected.get(index).copied().unwrap_or(0);
        difference |= left ^ right;
    }
    difference == 0
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


/// Cache usage plus one row per configured entry, all authcodes masked.
async fn state_handler(request: HttpRequest, state: web::Data<AdminState>) -> impl Responder {
    if let Some(denied) = guard(&request, &state).await {
        return denied
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
        "entries": entries
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
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn secret_comparison_accepts_only_an_exact_match() {
        assert!(secrets_match("s3cret-token", "s3cret-token"));
        assert!(!secrets_match("s3cret-token", "s3cret-toke"));
        assert!(!secrets_match("s3cret-token", "s3cret-tokenn"));
        assert!(!secrets_match("", "s3cret-token"));
        assert!(!secrets_match("s3cret-token", ""));
    }

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
