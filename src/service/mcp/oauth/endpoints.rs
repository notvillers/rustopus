//! The OAuth endpoints: two metadata documents at the origin root, and the
//! `/oauth` scope that signs a partner in and issues tokens.
//!
//! ## Why `/oauth` is a scope
//!
//! Like `/mcp`, `/admin` and `/export/{token}`, this is mounted as a scope rather
//! than through the repo's `get`/`get_alias` route pair. It is a small internal
//! application with its own authentication model, not a fetcher endpoint, so
//! there is no plural alias to register. It is the fourth deliberate exception to
//! that convention.
//!
//! ## Two headers these handlers set for themselves
//!
//! - **`Content-Security-Policy`.** The app-wide policy in `main.rs` ends with
//!   `form-action 'self'`, and Chrome applies `form-action` to the *redirect
//!   target* of a form submission — so the `303` from `POST /oauth/login` back to
//!   `https://claude.ai/…` would be refused in the browser, with a console error
//!   and no server-side symptom at all. `DefaultHeaders` only adds a header when
//!   it is absent, so the value set here wins.
//! - **CORS on the metadata documents.** They are public documents fetched by
//!   clients from other origins; the app-wide
//!   `Cross-Origin-Resource-Policy: same-origin` has to be relaxed there.

use std::path::PathBuf;

use actix_files::NamedFile;
use actix_web::http::{Method, StatusCode, header};
use actix_web::{HttpRequest, HttpResponse, Responder, Scope, web};
use base64::Engine;
use chrono::{Duration as ChronoDuration, Utc};
use serde::Deserialize;
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::service::{
    config::get_mcp_settings,
    ipv4::log_ip,
    log::{elog_with_ip, elogger, log_with_ip, logger},
    mcp::{
        cache::cache,
        index::verify_authcode,
        mask_authcode,
        oauth::{
            self, Grant, OauthClient, SCOPE,
            store::{self, IssuedCode, PendingRequest}
        },
        secrets_match
    },
    path::get_current_or_root_dir,
    soap_config::get_default_url
};

/// Policy for the sign-in pages.
///
/// The only difference from the app-wide policy is `form-action`: the sign-in
/// form's response is a redirect to the client, which Chrome checks against this
/// directive. Scripts stay `'self'` — the page's assets are external files, like
/// the admin dashboard's.
const OAUTH_CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self'; img-src 'self' data:; \
                         font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; \
                         form-action 'self' https://claude.ai https://claude.com";


/// Directory the sign-in page and its assets are served from, resolved against
/// the working directory like the docs and the dashboard.
fn static_dir() -> PathBuf {
    let mut path = get_current_or_root_dir();
    path.push("src");
    path.push("static");
    path.push("oauth");
    path
}


/// Percent-encodes a value for a query string.
///
/// Written here rather than pulled in as a dependency: three call sites need it,
/// all of them building a redirect back to a client.
fn query_escape(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for byte in value.as_bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => escaped.push(*byte as char),
            other => escaped.push_str(&format!("%{:02X}", other))
        }
    }
    escaped
}


/// Appends a query string to a URI that may already carry one.
fn with_query(uri: &str, query: &str) -> String {
    let separator = if uri.contains('?') { '&' } else { '?' };
    format!("{}{}{}", uri, separator, query)
}


// ------------------------------------------------------ metadata documents ---

/// Adds the headers a cross-origin client needs to read a public document.
fn metadata_response(body: serde_json::Value) -> HttpResponse {
    HttpResponse::Ok()
        .insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
        // Overrides the app-wide `same-origin`, which would stop a client on
        // another origin from reading a document written to be public.
        .insert_header(("Cross-Origin-Resource-Policy", "cross-origin"))
        .insert_header((header::CACHE_CONTROL, "public, max-age=3600"))
        .json(body)
}


/// RFC 9728 protected-resource metadata.
pub fn protected_resource_document() -> serde_json::Value {
    json!({
        "resource": oauth::resource_uri(),
        "authorization_servers": [oauth::issuer()],
        "bearer_methods_supported": ["header"],
        "scopes_supported": [SCOPE],
        "resource_documentation": format!("{}/docs/", oauth::issuer())
    })
}


/// RFC 8414 authorization-server metadata.
///
/// No `registration_endpoint` is published: clients are created by hand in
/// `/admin`. A client that can only register dynamically therefore fails at
/// discovery rather than half-working, which is the intended outcome.
pub fn authorization_server_document() -> serde_json::Value {
    let issuer = oauth::issuer();
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{}/oauth/authorize", issuer),
        "token_endpoint": format!("{}/oauth/token", issuer),
        "revocation_endpoint": format!("{}/oauth/revoke", issuer),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "code_challenge_methods_supported": ["S256"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
        "scopes_supported": [SCOPE],
        "service_documentation": format!("{}/docs/", issuer)
    })
}


/// Served at both `/.well-known/oauth-protected-resource` and
/// `…/oauth-protected-resource/mcp`, because clients disagree about where to look
/// and two route registrations are cheaper than a support call.
async fn protected_resource() -> impl Responder {
    metadata_response(protected_resource_document())
}

async fn authorization_server() -> impl Responder {
    metadata_response(authorization_server_document())
}


/// Preflight for the two documents above.
async fn metadata_preflight() -> impl Responder {
    HttpResponse::NoContent()
        .insert_header((header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"))
        .insert_header((header::ACCESS_CONTROL_ALLOW_METHODS, "GET, OPTIONS"))
        .insert_header((header::ACCESS_CONTROL_ALLOW_HEADERS, "*"))
        .finish()
}


// -------------------------------------------------------------- authorize ---

#[derive(Debug, Deserialize)]
pub struct AuthorizeQuery {
    client_id: Option<String>,
    redirect_uri: Option<String>,
    response_type: Option<String>,
    code_challenge: Option<String>,
    code_challenge_method: Option<String>,
    scope: Option<String>,
    state: Option<String>,
    resource: Option<String>
}


/// A plain page for a failure that must **not** be redirected anywhere: an
/// unknown client, or a redirect URI this client has not registered. Redirecting
/// to an unverified URI is how open redirectors are built.
fn plain_error(status: StatusCode, title: &str, message: &str) -> HttpResponse {
    HttpResponse::build(status)
        .insert_header((header::CONTENT_SECURITY_POLICY, OAUTH_CSP))
        .content_type("text/html; charset=utf-8")
        .body(format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <title>{title}</title><link rel=\"stylesheet\" href=\"/oauth/login.css\"></head>\
             <body class=\"error-page\"><main class=\"card\"><h1>{title}</h1><p>{message}</p></main></body></html>",
            title = oauth::escape_html(title),
            message = oauth::escape_html(message)
        ))
}


/// RFC 6749 §4.1.2.1: once the client and its redirect URI are known to be
/// genuine, an invalid request is reported *to the client*, not to the user.
fn redirect_error(redirect_uri: &str, state: Option<&str>, error: &str, description: &str) -> HttpResponse {
    let mut query = format!("error={}&error_description={}", query_escape(error), query_escape(description));
    if let Some(state) = state {
        query.push_str(&format!("&state={}", query_escape(state)));
    }
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, with_query(redirect_uri, &query)))
        .insert_header((header::CONTENT_SECURITY_POLICY, OAUTH_CSP))
        .finish()
}


/// Renders the sign-in page.
///
/// A template rather than a static file: the page has to carry the opaque
/// request id, and the client's name beside it is operator input, so both go
/// through [`oauth::escape_html`].
fn render_login(request_id: &str, client_name: &str, error: Option<&str>, status: StatusCode) -> HttpResponse {
    let path = static_dir().join("login.html");
    let template = match std::fs::read_to_string(&path) {
        Ok(template) => template,
        Err(error) => {
            elogger(format!("OAuth: cannot read the sign-in page '{:?}': {}", path, error));
            return plain_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Sign-in unavailable",
                "The sign-in page could not be loaded. Tell whoever runs this server."
            )
        }
    };

    let banner = error
        .map(|message| format!("<p class=\"error\" role=\"alert\">{}</p>", oauth::escape_html(message)))
        .unwrap_or_default();

    let body = template
        .replace("{{REQUEST_ID}}", &oauth::escape_html(request_id))
        .replace("{{CLIENT_NAME}}", &oauth::escape_html(client_name))
        .replace("{{ERROR}}", &banner);

    HttpResponse::build(status)
        .insert_header((header::CONTENT_SECURITY_POLICY, OAUTH_CSP))
        // A sign-in page is per-request state; a cached copy would carry a stale
        // request id.
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .content_type("text/html; charset=utf-8")
        .body(body)
}


/// Validates an authorization request and renders the sign-in page.
///
/// Order matters: the client and the redirect URI are checked first, because
/// every later failure is reported by redirecting to that URI.
async fn authorize(query: web::Query<AuthorizeQuery>, request: HttpRequest) -> impl Responder {
    let query = query.into_inner();
    let ip_address = log_ip(request.clone()).await.to_string();

    let Some(client_id) = query.client_id.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
        return plain_error(StatusCode::BAD_REQUEST, "Unknown connector", "This sign-in link carries no client id.")
    };
    let Some(client) = store::find_client(client_id).filter(OauthClient::is_enabled) else {
        elog_with_ip(&ip_address, format!("OAUTH: authorize refused — unknown or disabled client '{}'", client_id));
        return plain_error(
            StatusCode::BAD_REQUEST,
            "Unknown connector",
            "This connector is not registered here, or has been disabled. Ask whoever set it up."
        )
    };

    let Some(redirect_uri) = query.redirect_uri.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
        return plain_error(StatusCode::BAD_REQUEST, "Invalid sign-in link", "This sign-in link carries no redirect URI.")
    };
    if !client.allows_redirect(redirect_uri) {
        elog_with_ip(&ip_address, format!(
            "OAUTH: authorize refused — redirect URI not registered for client '{}'", client.name
        ));
        return plain_error(
            StatusCode::BAD_REQUEST,
            "Invalid sign-in link",
            "This connector is not allowed to send you back to that address. \
             The redirect URI has to be registered here first."
        )
    }

    let state = query.state.as_deref();

    if query.response_type.as_deref().map(str::trim) != Some("code") {
        return redirect_error(redirect_uri, state, "unsupported_response_type", "only response_type=code is supported")
    }

    let Some(code_challenge) = query.code_challenge.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
        return redirect_error(redirect_uri, state, "invalid_request", "code_challenge is required (PKCE)")
    };
    if query.code_challenge_method.as_deref().map(str::trim) != Some("S256") {
        return redirect_error(redirect_uri, state, "invalid_request", "only code_challenge_method=S256 is supported")
    }

    // An empty `scope` means "whatever you issue", which here is one scope.
    let requested_scope = query.scope.as_deref().map(str::trim).unwrap_or_default();
    if !requested_scope.is_empty() && requested_scope != SCOPE {
        return redirect_error(redirect_uri, state, "invalid_scope", "the only scope this server issues is catalog.read")
    }

    // RFC 8707: a token is bound to one resource. A client asking for a different
    // audience is asking the wrong server.
    if let Some(resource) = query.resource.as_deref().map(str::trim).filter(|value| !value.is_empty())
        && resource.trim_end_matches('/') != oauth::resource_uri() {
            return redirect_error(redirect_uri, state, "invalid_target", "this server only issues tokens for its own /mcp endpoint")
    }

    let request_id = store::stash_request(PendingRequest::new(
        client.client_id.clone(),
        client.name.clone(),
        redirect_uri.to_string(),
        state.map(str::to_string),
        code_challenge.to_string(),
        SCOPE.to_string(),
        oauth::resource_uri().to_string()
    ));

    log_with_ip(&ip_address, format!("OAUTH: sign-in page served for client '{}'", client.name));
    render_login(&request_id, &client.name, None, StatusCode::OK)
}


// ------------------------------------------------------------------ login ---

#[derive(Debug, Deserialize)]
pub struct LoginForm {
    request_id: String,
    authcode: String,
    /// Taken as text and parsed here, so a typo comes back as a message on the
    /// form rather than a bare `400` from the extractor.
    pid: String
}


/// Accepts the partner's Octopus credentials and hands the client an
/// authorization code.
///
/// `POST` only, form-encoded: an authcode must never reach a query string, where
/// it would land in every access log between the browser and here — the same rule
/// that keeps authcodes out of `/export` links.
async fn login(form: web::Form<LoginForm>, request: HttpRequest) -> impl Responder {
    let form = form.into_inner();
    let ip_address = log_ip(request.clone()).await.to_string();

    let Some(pending) = store::peek_request(form.request_id.trim()) else {
        return plain_error(
            StatusCode::BAD_REQUEST,
            "Sign-in timed out",
            "This sign-in page is no longer valid. Start again from the connector."
        )
    };

    let limit = get_mcp_settings().oauth_login_rate_limit();
    if store::is_rate_limited(&ip_address, limit) {
        elog_with_ip(&ip_address, "OAUTH: sign-in refused — too many failed attempts");
        return render_login(
            &form.request_id,
            &pending.client_name,
            Some("Too many failed attempts. Wait ten minutes and try again."),
            StatusCode::TOO_MANY_REQUESTS
        )
    }

    let authcode = form.authcode.trim();
    if authcode.is_empty() {
        return render_login(&form.request_id, &pending.client_name, Some("Enter your authcode."), StatusCode::OK)
    }
    let Ok(pid) = form.pid.trim().parse::<i64>() else {
        return render_login(&form.request_id, &pending.client_name, Some("The partner ID has to be a number."), StatusCode::OK)
    };

    let Some(url) = get_default_url() else {
        elogger("OAuth: sign-in cannot proceed — no Octopus url in soap.json");
        return plain_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Sign-in unavailable",
            "This server has no Octopus endpoint configured. Tell whoever runs it."
        )
    };

    // One cheap incremental call proves the code. A wrong one fails here, before
    // any token exists.
    if let Err(error) = verify_authcode(authcode, pid, &url).await {
        store::note_failure(&ip_address);
        elog_with_ip(&ip_address, format!(
            "OAUTH: sign-in refused for {} pid={} — {}", mask_authcode(authcode), pid, error
        ));
        return render_login(
            &form.request_id,
            &pending.client_name,
            Some("Octopus did not accept that authcode. Check it and try again."),
            StatusCode::OK
        )
    }
    store::clear_failures(&ip_address);
    store::sweep_grants();

    // The refresh token is minted here rather than at the token exchange, so the
    // credential file is written once per sign-in instead of twice.
    let refresh_token = oauth::new_secret();
    let ttl = get_mcp_settings().oauth_refresh_ttl_secs() as i64;
    let grant = Grant {
        id: Grant::make_id(&pending.client_id, authcode, pid),
        client_id: pending.client_id.clone(),
        label: format!("{} pid={}", mask_authcode(authcode), pid),
        authcode: authcode.to_string(),
        pid,
        resource: pending.resource.clone(),
        scope: pending.scope.clone(),
        refresh_hash: oauth::hash_secret(&refresh_token),
        created_at: Utc::now(),
        expires_at: Utc::now() + ChronoDuration::seconds(ttl)
    };
    let grant_id = grant.id.clone();
    let masked = grant.masked();

    if let Err(error) = store::upsert_grant(grant) {
        elogger(format!("OAuth: cannot record the grant for {}: {}", masked, error));
        return plain_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Sign-in failed",
            "Your credentials were accepted but the sign-in could not be recorded. Tell whoever runs this server."
        )
    }

    store::drop_request(form.request_id.trim());
    let code = store::issue_code(IssuedCode::new(
        pending.client_id.clone(),
        pending.redirect_uri.clone(),
        pending.code_challenge.clone(),
        pending.resource.clone(),
        pending.scope.clone(),
        grant_id,
        refresh_token
    ));

    log_with_ip(&ip_address, format!("OAUTH: signed in [{}] for client '{}'", masked, pending.client_name));

    // Warm the catalog in the background. This is what proves the pid — only
    // prices vary by it, so a synchronous check would mean a full price pull —
    // and it means the partner's first question does not wait out a cold build.
    let warm_authcode = authcode.to_string();
    tokio::spawn(async move {
        match cache().get_or_build(&warm_authcode, pid, &url).await {
            Ok(snapshot) => logger(format!(
                "OAuth: catalog warmed after sign-in for {} pid={} — {} products",
                mask_authcode(&warm_authcode), pid, snapshot.products.len()
            )),
            Err(error) => elogger(format!(
                "OAuth: first catalog build failed for {} pid={} — {}. A wrong partner ID looks like this.",
                mask_authcode(&warm_authcode), pid, error
            ))
        }
    });

    let mut query = format!("code={}", query_escape(&code));
    if let Some(state) = &pending.state {
        query.push_str(&format!("&state={}", query_escape(state)));
    }
    HttpResponse::SeeOther()
        .insert_header((header::LOCATION, with_query(&pending.redirect_uri, &query)))
        .insert_header((header::CONTENT_SECURITY_POLICY, OAUTH_CSP))
        .finish()
}


// ------------------------------------------------------------------ token ---

#[derive(Debug, Deserialize)]
pub struct TokenForm {
    grant_type: Option<String>,
    code: Option<String>,
    redirect_uri: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    resource: Option<String>
}


/// RFC 6749 §5.2 error. JSON, deliberately **not** the house XML error shape:
/// what parses this is an OAuth client, not a partner reading an API response.
fn token_error(status: StatusCode, error: &str, description: &str) -> HttpResponse {
    let mut response = HttpResponse::build(status);
    if status == StatusCode::UNAUTHORIZED {
        response.insert_header((header::WWW_AUTHENTICATE, "Basic realm=\"Rustopus OAuth\""));
    }
    response
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .json(json!({ "error": error, "error_description": description }))
}


/// The client credentials a request presents, from HTTP Basic or the form body.
///
/// Both ids and secrets are minted by this server as hex, so no
/// form-urlencoding ever appears inside the Basic payload and none is undone
/// here.
fn presented_client(request: &HttpRequest, body_id: Option<&str>, body_secret: Option<&str>) -> Option<(String, String)> {
    if let Some(value) = request.headers().get(header::AUTHORIZATION)
        && let Ok(value) = value.to_str()
        && let Some(encoded) = value.strip_prefix("Basic ")
        && let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(encoded.trim())
        && let Ok(decoded) = String::from_utf8(decoded)
        && let Some((id, secret)) = decoded.split_once(':') {
            return Some((id.to_string(), secret.to_string()))
    }

    match (body_id, body_secret) {
        (Some(id), Some(secret)) => Some((id.to_string(), secret.to_string())),
        _ => None
    }
}


/// Authenticates the client, or returns the response to send instead.
fn authenticate_client(
    request: &HttpRequest,
    body_id: Option<&str>,
    body_secret: Option<&str>
) -> Result<OauthClient, HttpResponse> {
    let Some((client_id, client_secret)) = presented_client(request, body_id, body_secret) else {
        return Err(token_error(StatusCode::UNAUTHORIZED, "invalid_client", "client authentication is required"))
    };
    let Some(client) = store::find_client(&client_id).filter(OauthClient::is_enabled) else {
        return Err(token_error(StatusCode::UNAUTHORIZED, "invalid_client", "unknown or disabled client"))
    };
    if !secrets_match(&oauth::hash_secret(&client_secret), &client.secret_hash) {
        return Err(token_error(StatusCode::UNAUTHORIZED, "invalid_client", "unknown or disabled client"))
    }
    Ok(client)
}


/// `BASE64URL-ENCODE(SHA256(ASCII(code_verifier))) == code_challenge`, unpadded.
fn pkce_matches(verifier: &str, challenge: &str) -> bool {
    let digest = Sha256::digest(verifier.as_bytes());
    let computed = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(digest);
    secrets_match(&computed, challenge)
}


/// The `authorization_code` and `refresh_token` grants.
async fn token(form: web::Form<TokenForm>, request: HttpRequest) -> impl Responder {
    let form = form.into_inner();
    let ip_address = log_ip(request.clone()).await.to_string();

    let client = match authenticate_client(&request, form.client_id.as_deref(), form.client_secret.as_deref()) {
        Ok(client) => client,
        Err(response) => {
            elog_with_ip(&ip_address, "OAUTH: token request refused — client authentication failed");
            return response
        }
    };

    let access_ttl = get_mcp_settings().oauth_access_ttl_secs();
    store::sweep_grants();

    match form.grant_type.as_deref().map(str::trim) {
        Some("authorization_code") => {
            let Some(value) = form.code.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_request", "code is required")
            };
            // Consumed on the first lookup, whether or not what follows succeeds.
            let Some(code) = store::take_code(value) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "the authorization code is unknown, used or expired")
            };
            if code.client_id != client.client_id {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "the authorization code was issued to another client")
            }
            // RFC 6749 §4.1.3: identical to the URI the code was issued for.
            if form.redirect_uri.as_deref().map(str::trim) != Some(code.redirect_uri.as_str()) {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "redirect_uri does not match the authorization request")
            }
            let Some(verifier) = form.code_verifier.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_request", "code_verifier is required (PKCE)")
            };
            if !pkce_matches(verifier, &code.code_challenge) {
                elog_with_ip(&ip_address, "OAUTH: token request refused — PKCE verification failed");
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "code_verifier does not match the code_challenge")
            }
            if let Some(resource) = form.resource.as_deref().map(str::trim).filter(|value| !value.is_empty())
                && resource.trim_end_matches('/') != code.resource {
                    return token_error(StatusCode::BAD_REQUEST, "invalid_target", "this token is for another resource")
            }
            let Some(grant) = store::find_grant(&code.grant_id) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "the sign-in behind this code has been revoked")
            };

            let access = store::issue_access(&grant.id, access_ttl);
            log_with_ip(&ip_address, format!("OAUTH: access token issued [{}] for client '{}'", grant.masked(), client.name));
            token_response(&access, access_ttl, Some(&code.refresh_token), &code.scope)
        }

        Some("refresh_token") => {
            let Some(value) = form.refresh_token.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_request", "refresh_token is required")
            };
            let Some(grant) = store::grant_by_refresh(&oauth::hash_secret(value)) else {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "the refresh token is unknown, revoked or expired")
            };
            if grant.client_id != client.client_id {
                return token_error(StatusCode::BAD_REQUEST, "invalid_grant", "the refresh token was issued to another client")
            }
            if let Some(resource) = form.resource.as_deref().map(str::trim).filter(|value| !value.is_empty())
                && resource.trim_end_matches('/') != grant.resource {
                    return token_error(StatusCode::BAD_REQUEST, "invalid_target", "this token is for another resource")
            }

            let access = store::issue_access(&grant.id, access_ttl);
            // The same refresh token comes back: it is not rotated, which is what
            // keeps the credential file untouched between sign-ins (see
            // `oauth::store`). Permitted because the client is confidential.
            log_with_ip(&ip_address, format!("OAUTH: access token refreshed [{}] for client '{}'", grant.masked(), client.name));
            token_response(&access, access_ttl, Some(value), &grant.scope)
        }

        other => token_error(
            StatusCode::BAD_REQUEST,
            "unsupported_grant_type",
            &format!(
                "'{}' is not supported — use authorization_code or refresh_token",
                other.unwrap_or("(none)")
            )
        )
    }
}


fn token_response(access: &str, expires_in: u64, refresh: Option<&str>, scope: &str) -> HttpResponse {
    let mut body = json!({
        "access_token": access,
        "token_type": "Bearer",
        "expires_in": expires_in,
        "scope": scope
    });
    if let Some(refresh) = refresh
        && let Some(object) = body.as_object_mut() {
            object.insert("refresh_token".into(), json!(refresh));
    }
    HttpResponse::Ok()
        .insert_header((header::CACHE_CONTROL, "no-store"))
        .insert_header((header::PRAGMA, "no-cache"))
        .json(body)
}


// ----------------------------------------------------------------- revoke ---

#[derive(Debug, Deserialize)]
pub struct RevokeForm {
    token: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>
}


/// RFC 7009 revocation.
///
/// Answers `200` whatever happened — a client must not learn from this endpoint
/// whether a token it presents exists.
async fn revoke(form: web::Form<RevokeForm>, request: HttpRequest) -> impl Responder {
    let form = form.into_inner();
    let ip_address = log_ip(request.clone()).await.to_string();

    let client = match authenticate_client(&request, form.client_id.as_deref(), form.client_secret.as_deref()) {
        Ok(client) => client,
        Err(response) => return response
    };

    let Some(token) = form.token.as_deref().map(str::trim).filter(|value| !value.is_empty()) else {
        return HttpResponse::Ok().finish()
    };
    let hash = oauth::hash_secret(token);

    if store::drop_access(&hash) {
        log_with_ip(&ip_address, format!("OAUTH: access token revoked by client '{}'", client.name));
        return HttpResponse::Ok().finish()
    }

    if let Some(grant) = store::grant_by_refresh(&hash)
        && grant.client_id == client.client_id {
            let masked = grant.masked();
            match store::remove_grant(&grant.id) {
                Ok(_) => log_with_ip(&ip_address, format!("OAUTH: sign-in revoked [{}] by client '{}'", masked, client.name)),
                Err(error) => elogger(format!("OAuth: cannot remove the revoked grant {}: {}", masked, error))
            }
    }

    HttpResponse::Ok().finish()
}


// ----------------------------------------------------------------- assets ---

/// Serves one of the sign-in page's own files. Public by nature — the page they
/// style is the one an unauthenticated partner is looking at.
async fn asset(request: &HttpRequest, name: &str) -> HttpResponse {
    match NamedFile::open_async(static_dir().join(name)).await {
        Ok(file) => file.into_response(request),
        Err(_) => HttpResponse::NotFound().content_type("text/plain").body("Not found")
    }
}

async fn style(request: HttpRequest) -> impl Responder {
    asset(&request, "login.css").await
}

async fn script(request: HttpRequest) -> impl Responder {
    asset(&request, "login.js").await
}


/// The `/oauth` scope. Registered only when MCP **and** OAuth are both enabled.
pub fn scope() -> Scope {
    web::scope("/oauth")
        .route("/authorize", web::get().to(authorize))
        .route("/login", web::post().to(login))
        .route("/token", web::post().to(token))
        .route("/revoke", web::post().to(revoke))
        .route("/login.css", web::get().to(style))
        .route("/login.js", web::get().to(script))
}


/// The metadata documents. They must sit at the **origin root**, not under
/// `/mcp`, which is why they are a scope of their own rather than part of the one
/// above. Both spellings of each path are served.
pub fn well_known_scope() -> Scope {
    web::scope("/.well-known")
        .route("/oauth-protected-resource", web::get().to(protected_resource))
        .route("/oauth-protected-resource/mcp", web::get().to(protected_resource))
        .route("/oauth-authorization-server", web::get().to(authorization_server))
        .route("/oauth-authorization-server/mcp", web::get().to(authorization_server))
        .route("/{document:.*}", web::method(Method::OPTIONS).to(metadata_preflight))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkce_accepts_only_the_matching_verifier() {
        // RFC 7636 appendix B's worked example.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        let challenge = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
        assert!(pkce_matches(verifier, challenge));
        assert!(!pkce_matches("some-other-verifier", challenge));
        assert!(!pkce_matches(verifier, "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cN"));
    }

    #[test]
    fn query_values_are_escaped_not_interpolated() {
        assert_eq!(query_escape("a b&c=d"), "a%20b%26c%3Dd");
        assert_eq!(query_escape("safe-._~"), "safe-._~");
        // A state carrying a separator cannot smuggle a second parameter in.
        assert_eq!(query_escape("x&error=nope"), "x%26error%3Dnope");
    }

    #[test]
    fn a_redirect_uri_that_already_has_a_query_keeps_it() {
        assert_eq!(with_query("https://x.test/cb", "code=1"), "https://x.test/cb?code=1");
        assert_eq!(with_query("https://x.test/cb?a=1", "code=1"), "https://x.test/cb?a=1&code=1");
    }

    #[test]
    fn the_protected_resource_document_names_this_server_and_its_scope() {
        let document = protected_resource_document();
        assert_eq!(document["resource"], oauth::resource_uri());
        assert_eq!(document["authorization_servers"][0], oauth::issuer());
        assert_eq!(document["bearer_methods_supported"][0], "header");
        assert_eq!(document["scopes_supported"][0], SCOPE);
    }

    #[test]
    fn the_authorization_server_document_advertises_pkce_and_no_registration() {
        let document = authorization_server_document();
        assert_eq!(document["issuer"], oauth::issuer());
        assert_eq!(document["response_types_supported"][0], "code");
        assert_eq!(document["code_challenge_methods_supported"][0], "S256");
        assert_eq!(document["grant_types_supported"][0], "authorization_code");
        assert_eq!(document["grant_types_supported"][1], "refresh_token");
        // Deliberately absent: clients are registered by hand in /admin, and a
        // client that can only register dynamically should fail at discovery
        // rather than half-work.
        assert!(document.get("registration_endpoint").is_none());
        assert!(document["authorization_endpoint"].as_str().is_some_and(|url| url.ends_with("/oauth/authorize")));
        assert!(document["token_endpoint"].as_str().is_some_and(|url| url.ends_with("/oauth/token")));
        assert!(document["revocation_endpoint"].as_str().is_some_and(|url| url.ends_with("/oauth/revoke")));
    }
}
