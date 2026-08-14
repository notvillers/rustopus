//! Where OAuth state lives: two files on disk, four tables in memory.
//!
//! ## On disk
//!
//! - `oauth_clients.toml` — registered connectors. Secrets are stored hashed, so
//!   this is not a credential file; it is still `0600`, since nothing but this
//!   server needs to read it.
//! - `oauth_sessions.toml` — issued grants. **Secret-grade**: each grant holds a
//!   live Octopus authcode in plain text, for the same reason `mcp_precache.toml`
//!   does — the tools present it to Octopus with no user in the loop.
//!
//! Both are written through a temporary file and a rename, so a crash mid-write
//! cannot leave half a file behind, and both treat a missing file as "nothing
//! configured" rather than an error. The structure is `service/blocklist.rs`'s.
//!
//! ## Why the session file is not rewritten hourly
//!
//! `service/mcp/precache.rs` already argues that a file holding live credentials
//! should not be rewritten on a timer. Two choices here follow from it:
//!
//! - **Access tokens are never persisted.** They live in [`ACCESS`], so an hourly
//!   refresh touches no file. A restart drops them; a client sees a `401`,
//!   refreshes, and carries on with nobody involved.
//! - **Refresh tokens are not rotated.** Rotation would mean writing the
//!   credential file every time an access token expired. OAuth 2.1 requires
//!   rotation *or* a confidential client, and claude.ai supplies a client secret.
//!
//! So the file is written on sign-in, on revocation and on the expiry sweep —
//! when a person actually did something.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, RwLock};
use std::time::Instant;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::service::{
    config::get_mcp_settings,
    log::{elogger, logger},
    mcp::oauth::{CODE_TTL_SECS, Grant, OauthClient, RATE_WINDOW_SECS, REQUEST_TTL_SECS, hash_secret, new_secret},
    path::get_current_or_root_dir
};


/// On-disk shape of `oauth_clients.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ClientConfig {
    #[serde(default, rename = "client")]
    pub clients: Vec<OauthClient>
}

/// On-disk shape of `oauth_sessions.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default, rename = "grant")]
    pub grants: Vec<Grant>
}


static CLIENTS: Lazy<RwLock<Vec<OauthClient>>> = Lazy::new(|| RwLock::new(load_clients().clients));
static GRANTS: Lazy<RwLock<Vec<Grant>>> = Lazy::new(|| RwLock::new(load_sessions().grants));


/// A validated authorization request, waiting on its sign-in page.
///
/// Held here rather than round-tripped through the browser as hidden fields: the
/// page carries only an opaque id, so nothing the user's page can be tricked into
/// echoing back changes what the resulting code is bound to.
#[derive(Debug, Clone)]
pub struct PendingRequest {
    pub client_id: String,
    pub client_name: String,
    pub redirect_uri: String,
    pub state: Option<String>,
    pub code_challenge: String,
    pub scope: String,
    pub resource: String,
    created: Instant
}

/// An issued authorization code. Single use, 60 seconds, and bound to everything
/// the exchange has to prove.
#[derive(Debug, Clone)]
pub struct IssuedCode {
    pub client_id: String,
    pub redirect_uri: String,
    pub code_challenge: String,
    pub resource: String,
    pub scope: String,
    pub grant_id: String,
    /// The refresh token minted with the grant, handed on at the exchange. It is
    /// created at sign-in so the credential file is written once per sign-in
    /// rather than once per token call.
    pub refresh_token: String,
    created: Instant
}

/// A live access token. Memory only — see the module note.
#[derive(Debug, Clone)]
pub struct AccessRecord {
    pub grant_id: String,
    expires: Instant
}

static REQUESTS: Lazy<Mutex<HashMap<String, PendingRequest>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static CODES: Lazy<Mutex<HashMap<String, IssuedCode>>> = Lazy::new(|| Mutex::new(HashMap::new()));
static ACCESS: Lazy<Mutex<HashMap<String, AccessRecord>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// When each grant was last used, for the dashboard. In memory like the precache
/// run log: a timestamp is not worth rewriting a credential file for.
static LAST_USED: Lazy<Mutex<HashMap<String, DateTime<Utc>>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Failed sign-ins per address, so the login form is not an oracle for guessing
/// authcodes against the ERP.
static FAILURES: Lazy<Mutex<HashMap<String, Vec<Instant>>>> = Lazy::new(|| Mutex::new(HashMap::new()));


/// Resolves a configured path against the working directory, like every other
/// runtime path in this service.
fn resolve(configured: String) -> PathBuf {
    let path = PathBuf::from(&configured);
    if path.is_absolute() {
        return path
    }
    let mut base = get_current_or_root_dir();
    base.push(configured);
    base
}

pub fn clients_path() -> PathBuf {
    resolve(get_mcp_settings().oauth_clients_path())
}

pub fn sessions_path() -> PathBuf {
    resolve(get_mcp_settings().oauth_sessions_path())
}


/// Reads a TOML file, or its empty shape when it is absent or unreadable. A
/// missing file is the normal case: nobody has signed in yet.
fn read_file<T: Default + serde::de::DeserializeOwned>(path: &Path, what: &str) -> T {
    if !path.is_file() {
        return T::default()
    }
    match std::fs::read_to_string(path) {
        Ok(content) => match toml::from_str::<T>(&content) {
            Ok(parsed) => parsed,
            Err(error) => {
                elogger(format!("OAuth: cannot parse {} '{:?}': {}", what, path, error));
                T::default()
            }
        },
        Err(error) => {
            elogger(format!("OAuth: cannot read {} '{:?}': {}", what, path, error));
            T::default()
        }
    }
}


/// Writes a file owner-only, through a temporary path and a rename.
fn write_file<T: Serialize>(path: &Path, banner: &str, value: &T) -> Result<(), String> {
    let body = toml::to_string_pretty(value).map_err(|error| error.to_string())?;
    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, format!("{}\n{}", banner, body)).map_err(|error| error.to_string())?;
    restrict_permissions(&temporary);
    std::fs::rename(&temporary, path).map_err(|error| error.to_string())?;
    restrict_permissions(path);
    Ok(())
}


/// Narrows a file to owner read/write. Logged rather than fatal — the
/// alternative is refusing to record a sign-in at all.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            elogger(format!("OAuth: cannot restrict permissions on '{:?}': {}", path, error));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}


pub fn load_clients() -> ClientConfig {
    read_file(&clients_path(), "oauth_clients.toml")
}

pub fn load_sessions() -> SessionConfig {
    read_file(&sessions_path(), "oauth_sessions.toml")
}


fn save_clients(config: &ClientConfig) -> Result<(), String> {
    write_file(
        &clients_path(),
        "# Rustopus OAuth clients.\n\
         #\n\
         # One entry per connector. Secrets are stored as a SHA-256 hash and shown\n\
         # exactly once, at creation, so this file is not a credential store — but\n\
         # nothing except this server needs to read it, so it is written 0600.\n\
         #\n\
         # Managed by the /admin dashboard; hand edits are picked up on restart.\n",
        config
    )
}


fn save_sessions(config: &SessionConfig) -> Result<(), String> {
    write_file(
        &sessions_path(),
        "# Rustopus OAuth grants.\n\
         #\n\
         # SECRET FILE: every grant holds a live Octopus authcode in plain text,\n\
         # because the MCP tools present it to Octopus with no user in the loop.\n\
         # Keep it gitignored, keep it 0600, and mount it like mcp_precache.toml\n\
         # rather than baking it into an image.\n\
         #\n\
         # Written on sign-in, on revocation and on the expiry sweep — never on a\n\
         # token refresh. Hand edits are picked up on restart.\n",
        config
    )
}


/// Loads both files and reports them. Called from `main.rs` at startup so a
/// broken file is visible in the log before the first connector tries to sign in.
pub fn init() {
    let clients = clients();
    let grants = grants();
    logger(format!(
        "OAuth: {} client{} registered, {} active grant{}",
        clients.len(),
        if clients.len() == 1 { "" } else { "s" },
        grants.len(),
        if grants.len() == 1 { "" } else { "s" }
    ));
    if clients.is_empty() {
        logger("OAuth: no clients registered yet — add one in /admin, then paste its id and secret into the connector");
    }
}


// ---------------------------------------------------------------- clients ---

pub fn clients() -> Vec<OauthClient> {
    CLIENTS.read().map(|clients| clients.clone()).unwrap_or_default()
}

pub fn find_client(client_id: &str) -> Option<OauthClient> {
    clients().into_iter().find(|client| client.client_id == client_id)
}

/// Adds a client, or replaces the one with the same id, and persists the result.
pub fn upsert_client(client: OauthClient) -> Result<(), String> {
    let mut current = clients();
    match current.iter().position(|existing| existing.client_id == client.client_id) {
        Some(position) => current[position] = client,
        None => current.push(client)
    }
    save_clients(&ClientConfig { clients: current.clone() })?;
    match CLIENTS.write() {
        Ok(mut held) => *held = current,
        Err(_) => return Err("oauth client lock poisoned".into())
    }
    Ok(())
}

/// Removes a client and every grant it issued: a connector that is gone should
/// not leave working tokens behind.
pub fn remove_client(client_id: &str) -> Result<bool, String> {
    let mut current = clients();
    let before = current.len();
    current.retain(|client| client.client_id != client_id);
    let removed = current.len() != before;

    save_clients(&ClientConfig { clients: current.clone() })?;
    match CLIENTS.write() {
        Ok(mut held) => *held = current,
        Err(_) => return Err("oauth client lock poisoned".into())
    }

    if removed {
        for grant in grants().into_iter().filter(|grant| grant.client_id == client_id) {
            remove_grant(&grant.id)?;
        }
    }
    Ok(removed)
}


// ----------------------------------------------------------------- grants ---

pub fn grants() -> Vec<Grant> {
    GRANTS.read().map(|grants| grants.clone()).unwrap_or_default()
}

pub fn find_grant(id: &str) -> Option<Grant> {
    grants().into_iter().find(|grant| grant.id == id && !grant.is_expired())
}

/// The grant a refresh token belongs to, by the token's hash.
pub fn grant_by_refresh(refresh_hash: &str) -> Option<Grant> {
    grants().into_iter().find(|grant| grant.refresh_hash == refresh_hash && !grant.is_expired())
}

/// Records a grant, replacing any previous one with the same id.
///
/// Signing in again through the same connector lands here: the old refresh token
/// stops working and its access tokens are dropped, which is what "sign in again"
/// should mean.
pub fn upsert_grant(grant: Grant) -> Result<(), String> {
    let id = grant.id.clone();
    let mut current = grants();
    match current.iter().position(|existing| existing.id == id) {
        Some(position) => current[position] = grant,
        None => current.push(grant)
    }
    commit_grants(current)?;
    drop_access_for_grant(&id);
    Ok(())
}

/// Revokes a grant: it leaves the file, and its live access tokens stop working
/// in the same call.
pub fn remove_grant(id: &str) -> Result<bool, String> {
    let mut current = grants();
    let before = current.len();
    current.retain(|grant| grant.id != id);
    let removed = current.len() != before;
    commit_grants(current)?;
    drop_access_for_grant(id);
    if let Ok(mut used) = LAST_USED.lock() {
        used.remove(id);
    }
    Ok(removed)
}


fn commit_grants(grants: Vec<Grant>) -> Result<(), String> {
    save_sessions(&SessionConfig { grants: grants.clone() })?;
    match GRANTS.write() {
        Ok(mut held) => *held = grants,
        Err(_) => return Err("oauth grant lock poisoned".into())
    }
    Ok(())
}


/// Drops grants whose refresh token has expired. Run at sign-in and at token
/// exchange rather than on a timer, for the reason in the module note.
pub fn sweep_grants() {
    let current = grants();
    let live: Vec<Grant> = current.iter().filter(|grant| !grant.is_expired()).cloned().collect();
    if live.len() == current.len() {
        return
    }
    for grant in current.iter().filter(|grant| grant.is_expired()) {
        logger(format!("OAuth: grant expired and was dropped [{}]", grant.masked()));
        drop_access_for_grant(&grant.id);
    }
    if let Err(error) = commit_grants(live) {
        elogger(format!("OAuth: cannot write the session file during the expiry sweep: {}", error));
    }
}


/// Notes that a grant was used, for the dashboard.
pub fn touch(grant_id: &str) {
    if let Ok(mut used) = LAST_USED.lock() {
        used.insert(grant_id.to_string(), Utc::now());
    }
}

pub fn last_used() -> HashMap<String, DateTime<Utc>> {
    LAST_USED.lock().map(|used| used.clone()).unwrap_or_default()
}


// ------------------------------------------------- pending sign-in requests ---

/// Stores a validated authorization request and returns the opaque id its
/// sign-in page carries.
pub fn stash_request(request: PendingRequest) -> String {
    let id = new_secret();
    sweep_requests();
    if let Ok(mut requests) = REQUESTS.lock() {
        requests.insert(id.clone(), PendingRequest { created: Instant::now(), ..request });
    }
    id
}

/// The request behind an id, or `None` when it is unknown or has timed out.
pub fn peek_request(id: &str) -> Option<PendingRequest> {
    sweep_requests();
    REQUESTS.lock().ok()?.get(id).cloned()
}

/// Forgets a request once its sign-in has succeeded.
pub fn drop_request(id: &str) {
    if let Ok(mut requests) = REQUESTS.lock() {
        requests.remove(id);
    }
}

fn sweep_requests() {
    if let Ok(mut requests) = REQUESTS.lock() {
        requests.retain(|_, request| request.created.elapsed().as_secs() < REQUEST_TTL_SECS);
    }
}

impl PendingRequest {
    /// Builds a pending request. `created` is set when it is stashed.
    pub fn new(
        client_id: String,
        client_name: String,
        redirect_uri: String,
        state: Option<String>,
        code_challenge: String,
        scope: String,
        resource: String
    ) -> Self {
        Self {
            client_id,
            client_name,
            redirect_uri,
            state,
            code_challenge,
            scope,
            resource,
            created: Instant::now()
        }
    }
}


// ---------------------------------------------------- authorization codes ---

/// Registers an authorization code and returns it.
pub fn issue_code(code: IssuedCode) -> String {
    let value = new_secret();
    sweep_codes();
    if let Ok(mut codes) = CODES.lock() {
        codes.insert(value.clone(), IssuedCode { created: Instant::now(), ..code });
    }
    value
}

/// Consumes a code. Removed on the **first** lookup whether or not the exchange
/// that follows succeeds, so a replayed code is never a second chance.
pub fn take_code(value: &str) -> Option<IssuedCode> {
    sweep_codes();
    let mut codes = CODES.lock().ok()?;
    let code = codes.remove(value)?;
    if code.created.elapsed().as_secs() >= CODE_TTL_SECS {
        return None
    }
    Some(code)
}

fn sweep_codes() {
    if let Ok(mut codes) = CODES.lock() {
        codes.retain(|_, code| code.created.elapsed().as_secs() < CODE_TTL_SECS);
    }
}

impl IssuedCode {
    pub fn new(
        client_id: String,
        redirect_uri: String,
        code_challenge: String,
        resource: String,
        scope: String,
        grant_id: String,
        refresh_token: String
    ) -> Self {
        Self {
            client_id,
            redirect_uri,
            code_challenge,
            resource,
            scope,
            grant_id,
            refresh_token,
            created: Instant::now()
        }
    }
}


// ----------------------------------------------------------- access tokens ---

/// Mints an access token for a grant and returns it. Only the hash is kept.
pub fn issue_access(grant_id: &str, ttl_secs: u64) -> String {
    let token = new_secret();
    sweep_access();
    if let Ok(mut access) = ACCESS.lock() {
        access.insert(hash_secret(&token), AccessRecord {
            grant_id: grant_id.to_string(),
            expires: Instant::now() + std::time::Duration::from_secs(ttl_secs)
        });
    }
    token
}

/// The grant a presented access token belongs to, or `None` when the token is
/// unknown or has expired.
pub fn resolve_access(token_hash: &str) -> Option<AccessRecord> {
    let access = ACCESS.lock().ok()?;
    let record = access.get(token_hash)?;
    if record.expires <= Instant::now() {
        return None
    }
    Some(record.clone())
}

/// Drops one access token (RFC 7009 revocation of an access token).
pub fn drop_access(token_hash: &str) -> bool {
    ACCESS.lock().map(|mut access| access.remove(token_hash).is_some()).unwrap_or(false)
}

/// Drops every access token issued against a grant.
pub fn drop_access_for_grant(grant_id: &str) {
    if let Ok(mut access) = ACCESS.lock() {
        access.retain(|_, record| record.grant_id != grant_id);
    }
}

fn sweep_access() {
    if let Ok(mut access) = ACCESS.lock() {
        let now = Instant::now();
        access.retain(|_, record| record.expires > now);
    }
}


// ------------------------------------------------------------- rate limit ---

/// Records a failed sign-in from an address.
pub fn note_failure(address: &str) {
    if let Ok(mut failures) = FAILURES.lock() {
        let window = failures.entry(address.to_string()).or_default();
        window.retain(|at| at.elapsed().as_secs() < RATE_WINDOW_SECS);
        window.push(Instant::now());
    }
}

/// Whether an address has spent its failed-sign-in budget for the window.
pub fn is_rate_limited(address: &str, limit: u32) -> bool {
    let Ok(mut failures) = FAILURES.lock() else {
        // A poisoned lock must not turn into an open door *or* a lockout; the
        // authcode check behind this still has to pass.
        return false
    };
    let Some(window) = failures.get_mut(address) else {
        return false
    };
    window.retain(|at| at.elapsed().as_secs() < RATE_WINDOW_SECS);
    window.len() as u32 >= limit
}

/// Forgets an address's failures after a successful sign-in.
pub fn clear_failures(address: &str) {
    if let Ok(mut failures) = FAILURES.lock() {
        failures.remove(address);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mcp::oauth::hash_secret;

    fn grant(id: &str, days: i64) -> Grant {
        Grant {
            id: id.into(),
            client_id: "client-1".into(),
            label: "FFD3…0E37 pid=1".into(),
            authcode: "FFD3ABCDEF120E37".into(),
            pid: 1,
            resource: "https://example.test/mcp".into(),
            scope: "catalog.read".into(),
            refresh_hash: hash_secret("refresh"),
            created_at: Utc::now(),
            expires_at: Utc::now() + chrono::Duration::days(days)
        }
    }

    #[test]
    fn an_expired_grant_is_recognised() {
        assert!(!grant("a", 30).is_expired());
        assert!(grant("a", -1).is_expired());
    }

    #[test]
    fn serialized_clients_round_trip_without_a_secret() {
        let config = ClientConfig {
            clients: vec![OauthClient {
                client_id: "abc".into(),
                name: "Orink Hungary".into(),
                secret_hash: hash_secret("s3cret"),
                redirect_uris: vec!["https://claude.ai/api/mcp/auth_callback".into()],
                created_at: Some(Utc::now()),
                enabled: Some(true)
            }]
        };
        let text = toml::to_string_pretty(&config).expect("serializes");
        assert!(!text.contains("s3cret"));

        let parsed: ClientConfig = toml::from_str(&text).expect("parses");
        assert_eq!(parsed.clients.len(), 1);
        assert_eq!(parsed.clients[0].redirect_uris.len(), 1);
        assert!(parsed.clients[0].is_enabled());
    }

    #[test]
    fn serialized_grants_round_trip() {
        let config = SessionConfig { grants: vec![grant("a", 30)] };
        let text = toml::to_string_pretty(&config).expect("serializes");
        // The refresh token is hashed; only the authcode is plain, and that is
        // what makes this file secret-grade.
        assert!(!text.contains("refresh\""));

        let parsed: SessionConfig = toml::from_str(&text).expect("parses");
        assert_eq!(parsed.grants.len(), 1);
        assert_eq!(parsed.grants[0].pid, 1);
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        assert!(toml::from_str::<ClientConfig>("").expect("empty parses").clients.is_empty());
        assert!(toml::from_str::<SessionConfig>("").expect("empty parses").grants.is_empty());
    }

    #[test]
    fn an_authorization_code_works_once() {
        let value = issue_code(IssuedCode::new(
            "client-1".into(),
            "https://claude.ai/api/mcp/auth_callback".into(),
            "challenge".into(),
            "https://example.test/mcp".into(),
            "catalog.read".into(),
            "grant-1".into(),
            "refresh".into()
        ));
        assert!(take_code(&value).is_some());
        assert!(take_code(&value).is_none());
    }

    #[test]
    fn an_access_token_resolves_to_its_grant_and_can_be_dropped() {
        let token = issue_access("grant-2", 60);
        let hash = hash_secret(&token);
        assert_eq!(resolve_access(&hash).map(|record| record.grant_id), Some("grant-2".into()));
        assert!(drop_access(&hash));
        assert!(resolve_access(&hash).is_none());
    }

    #[test]
    fn an_expired_access_token_stops_resolving() {
        let token = issue_access("grant-3", 0);
        assert!(resolve_access(&hash_secret(&token)).is_none());
    }

    #[test]
    fn revoking_a_grant_drops_its_access_tokens() {
        let token = issue_access("grant-4", 60);
        drop_access_for_grant("grant-4");
        assert!(resolve_access(&hash_secret(&token)).is_none());
    }

    #[test]
    fn failed_sign_ins_are_counted_per_address() {
        let address = "203.0.113.77";
        clear_failures(address);
        assert!(!is_rate_limited(address, 2));
        note_failure(address);
        assert!(!is_rate_limited(address, 2));
        note_failure(address);
        assert!(is_rate_limited(address, 2));
        clear_failures(address);
        assert!(!is_rate_limited(address, 2));
    }
}
