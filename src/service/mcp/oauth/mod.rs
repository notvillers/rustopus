//! OAuth 2.1 for `/mcp`.
//!
//! ## Why this exists
//!
//! `/mcp` identifies its caller from `X-Authcode` and `X-Pid`. That works for
//! `mcp-remote`, `curl` and Claude Code, all of which let the operator set
//! arbitrary headers. It does not work for the claude.ai **custom connector**
//! dialog, which offers exactly two shapes: no authentication, or OAuth with a
//! client id and secret. Added with headers as the only identity, such a
//! connector looks like it works — `initialize` and `tools/list` need no
//! credentials — and then fails every tool call.
//!
//! So Rustopus becomes both the **authorization server** and the **resource
//! server** for its own MCP endpoint. There is no third party, no new user
//! directory (the sign-in page asks for the Octopus credentials the partner
//! already holds) and no dynamic client registration (RFC 7591): clients are
//! created by hand in `/admin`, which is what the dialog's Client ID / Client
//! Secret fields exist for.
//!
//! ## Layout
//!
//! - [`store`] — the two files and the in-memory tables: clients, grants,
//!   pending sign-ins, authorization codes, access tokens.
//! - [`endpoints`] — the metadata documents, `/oauth/authorize`, `/oauth/login`,
//!   `/oauth/token`, `/oauth/revoke`.
//! - [`guard`] — the middleware that turns `/mcp` into a protected resource.
//!
//! Everything here is inert unless `[mcp] enabled` **and** `[mcp] oauth_enabled`
//! are both on: with either off no route is registered, no file is read, and the
//! guard returns on its first line.
//!
//! ## What is secret and what is not
//!
//! `oauth_sessions.toml` holds live authcodes in plain text, for the same reason
//! `mcp_precache.toml` does — the server presents them to Octopus with no user in
//! the loop. It is secret-grade. `oauth_clients.toml` is not: a client secret is
//! stored hashed and shown exactly once, at creation. Both are written `0600`
//! through a temp file and a rename.

pub mod endpoints;
pub mod guard;
pub mod store;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::service::{
    config::get_mcp_settings,
    mcp::{
        cache::{fingerprint, hash_authcode},
        mask_authcode
    }
};

/// The only scope this server issues. One scope, because there is one thing a
/// token can do: read the catalog the caller's own authcode already reaches.
pub const SCOPE: &str = "catalog.read";

/// How long a validated authorization request waits on its sign-in page.
pub const REQUEST_TTL_SECS: u64 = 600;

/// Authorization-code lifetime. Short by design — it travels through a browser
/// redirect and is exchanged immediately.
pub const CODE_TTL_SECS: u64 = 60;

/// Window the sign-in rate limit counts failures over.
pub const RATE_WINDOW_SECS: u64 = 600;


/// Whether `/mcp` is an OAuth-protected resource on this instance.
///
/// Read on every `/mcp` request, so it is resolved once rather than cloning the
/// whole `[mcp]` table each time. `Config.toml` is parsed once at startup
/// anyway — changing any of this needs a restart.
static ENABLED: Lazy<bool> = Lazy::new(|| {
    let config = get_mcp_settings();
    // OAuth protects `/mcp`; with MCP itself off there is nothing to protect and
    // no route to wrap.
    config.is_enabled() && config.oauth_enabled()
});

static ALLOW_HEADERS: Lazy<bool> = Lazy::new(|| get_mcp_settings().oauth_allow_headers());

/// Issuer identifier. Deliberately the same value as the export links' base URL:
/// two sources of truth for one hostname is how a metadata document ends up
/// pointing at a server nobody can reach.
static ISSUER: Lazy<String> = Lazy::new(|| get_mcp_settings().public_url());

/// Canonical resource URI, the audience every token is bound to (RFC 8707).
static RESOURCE: Lazy<String> = Lazy::new(|| format!("{}/mcp", *ISSUER));


pub fn is_enabled() -> bool {
    *ENABLED
}

/// Whether `X-Authcode` / `X-Pid` callers are still served once OAuth is on.
pub fn allow_headers() -> bool {
    *ALLOW_HEADERS
}

pub fn issuer() -> &'static str {
    ISSUER.as_str()
}

pub fn resource_uri() -> &'static str {
    RESOURCE.as_str()
}

/// Where a client is told to look for the resource metadata, both in the
/// `WWW-Authenticate` challenge and in the documents themselves.
pub fn metadata_url() -> String {
    format!("{}/.well-known/oauth-protected-resource", issuer())
}


/// An unguessable opaque secret: two `uuid` v4 values, ~244 bits from the OS
/// random source, through a dependency this crate already has. The same
/// construction the export download tokens use.
pub fn new_secret() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}


/// SHA-256 hex of a token or secret. What is stored and compared; the value
/// itself is shown once and never again.
pub fn hash_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}


/// A registered OAuth client — one per organization's connector, created in
/// `/admin`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OauthClient {
    pub client_id: String,
    /// Human label for the dashboard.
    pub name: String,
    /// SHA-256 hex of the client secret. The secret itself is returned by the
    /// creating request and then unrecoverable.
    pub secret_hash: String,
    #[serde(default)]
    pub redirect_uris: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>,
    /// Set `false` to keep a client on file but refuse its sign-ins.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>
}

impl OauthClient {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// Whether this client may be redirected to `presented`.
    pub fn allows_redirect(&self, presented: &str) -> bool {
        self.redirect_uris.iter().any(|registered| redirect_matches(registered, presented))
    }
}


/// Exact-match redirect URI comparison, with the one exception RFC 8252 §7.3
/// carves out: a **loopback** URI's port is chosen at run time by the client, so
/// it is compared on scheme, host and path only.
///
/// The exception is loopback-only and `http`-only. Everything else — every URI a
/// browser on the public internet could be sent to — is compared as a string,
/// with no prefix matching and no wildcards, because a redirect URI that is
/// matched loosely is an open redirector.
pub fn redirect_matches(registered: &str, presented: &str) -> bool {
    if registered == presented {
        return true
    }
    match (loopback_parts(registered), loopback_parts(presented)) {
        (Some(left), Some(right)) => left == right,
        _ => false
    }
}


/// Splits a loopback redirect URI into `(host, path-and-query)`, dropping the
/// port. `None` for anything that is not plain `http` to a loopback host.
///
/// `localhost` is accepted alongside the two IP literals RFC 8252 recommends:
/// `mcp-remote`, the client this is most often tested with, builds its callback
/// that way, and the name is still loopback-only.
fn loopback_parts(uri: &str) -> Option<(String, String)> {
    let rest = uri.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(position) => rest.split_at(position),
        None => (rest, "")
    };

    // `[::1]:1234` — the brackets belong to the host, the port does not.
    let host = if let Some(inner) = authority.strip_prefix('[') {
        let (inner, _) = inner.split_once(']')?;
        inner.to_string()
    } else {
        authority.split(':').next()?.to_string()
    };

    match host.as_str() {
        "127.0.0.1" | "::1" | "localhost" => Some((host, path.to_string())),
        _ => None
    }
}


/// One issued grant: a partner's Octopus credentials, tied to the client that
/// signed them in.
///
/// **Secret-grade.** `authcode` is the live code, in plain text, because every
/// tool call has to present it to Octopus with no user present. Nothing renders
/// this struct outside the process without going through [`Grant::masked`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Grant {
    pub id: String,
    pub client_id: String,
    /// How the grant is shown: the masked code and the pid, never more.
    pub label: String,
    /// **SECRET** — the Octopus authentication code.
    pub authcode: String,
    pub pid: i64,
    /// The audience this grant's tokens are bound to (RFC 8707).
    pub resource: String,
    pub scope: String,
    /// SHA-256 hex of the refresh token, never the token.
    pub refresh_hash: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>
}

impl Grant {
    /// Stable, non-reversible identifier.
    ///
    /// Keyed on the client as well as the credentials, so two organizations
    /// signing in with the same authcode hold two grants — and so signing in
    /// again through the same connector **replaces** the previous grant instead
    /// of piling up dead refresh tokens.
    pub fn make_id(client_id: &str, authcode: &str, pid: i64) -> String {
        let seed = format!("{}:{}", client_id, authcode);
        format!("{}-{}", fingerprint(&hash_authcode(&seed)), pid)
    }

    /// The id the precache job would use for the same combination, so `/admin`
    /// can tell the operator whether this partner's catalog is kept warm.
    pub fn precache_id(&self) -> String {
        format!("{}-{}", fingerprint(&hash_authcode(&self.authcode)), self.pid)
    }

    /// How this grant may be shown outside the process.
    pub fn masked(&self) -> String {
        format!("{} pid={}", mask_authcode(&self.authcode), self.pid)
    }

    pub fn is_expired(&self) -> bool {
        self.expires_at <= Utc::now()
    }
}


/// Escapes text interpolated into the sign-in page.
///
/// The page is a template rather than a static file because it has to carry the
/// opaque request id — and the client name beside it is operator input, so it
/// goes through here for the same reason the dashboard builds rows with
/// `textContent`.
pub fn escape_html(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            other => escaped.push(other)
        }
    }
    escaped
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_public_redirect_uri_is_matched_exactly() {
        let uri = "https://claude.ai/api/mcp/auth_callback";
        assert!(redirect_matches(uri, uri));
        // No prefix matching: an open redirector is exactly this check written
        // loosely.
        assert!(!redirect_matches(uri, "https://claude.ai/api/mcp/auth_callback/evil"));
        assert!(!redirect_matches(uri, "https://claude.ai.evil.test/api/mcp/auth_callback"));
        assert!(!redirect_matches(uri, "https://claude.ai/api/mcp/auth_callback?x=1"));
    }

    #[test]
    fn a_loopback_uri_matches_on_any_port() {
        assert!(redirect_matches("http://127.0.0.1:3334/oauth/callback", "http://127.0.0.1:51763/oauth/callback"));
        assert!(redirect_matches("http://[::1]:3334/oauth/callback", "http://[::1]:9/oauth/callback"));
        assert!(redirect_matches("http://localhost:3334/oauth/callback", "http://localhost:1/oauth/callback"));
        // The path still has to match, and the host still has to be loopback.
        assert!(!redirect_matches("http://127.0.0.1:3334/oauth/callback", "http://127.0.0.1:3334/other"));
        assert!(!redirect_matches("http://127.0.0.1:3334/oauth/callback", "http://203.0.113.7:3334/oauth/callback"));
        // https is not the loopback case, so it is matched as a string.
        assert!(!redirect_matches("https://127.0.0.1:3334/cb", "https://127.0.0.1:4/cb"));
    }

    #[test]
    fn a_grant_id_is_derived_from_the_hash_not_the_code() {
        let id = Grant::make_id("client-1", "SUPERSECRETAUTHCODE", 7);
        assert!(!id.contains("SUPERSECRET"));
        assert!(id.ends_with("-7"));
        assert_eq!(id, Grant::make_id("client-1", "SUPERSECRETAUTHCODE", 7));
        assert_ne!(id, Grant::make_id("client-2", "SUPERSECRETAUTHCODE", 7));
        assert_ne!(id, Grant::make_id("client-1", "OTHERSECRETAUTHCODE", 7));
        assert_ne!(id, Grant::make_id("client-1", "SUPERSECRETAUTHCODE", 8));
    }

    #[test]
    fn a_hash_is_hex_and_reveals_nothing() {
        let hash = hash_secret("FFD3ABCDEF120E37");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(hash, hash_secret("FFD3ABCDEF120E38"));
    }

    #[test]
    fn secrets_are_long_and_never_repeat() {
        let one = new_secret();
        assert_eq!(one.len(), 64);
        assert_ne!(one, new_secret());
    }

    #[test]
    fn interpolated_text_cannot_close_a_tag() {
        assert_eq!(escape_html("<script>x</script>"), "&lt;script&gt;x&lt;/script&gt;");
        assert_eq!(escape_html("a\"b'c&d"), "a&quot;b&#39;c&amp;d");
    }
}
