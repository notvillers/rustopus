//! The middleware that makes `/mcp` an OAuth-protected resource.
//!
//! ## Why a middleware and not the transport's hook
//!
//! `rmcp-actix-web` offers an `on_request` hook, and `tools.rs` already uses it
//! to lift the headers into the request extensions. It cannot be used here: its
//! signature is `Fn(&HttpRequest, &mut Extensions)` — it can *observe* a request
//! and add to it, but it cannot **reject** one, and the whole point of this file
//! is the `401`. Wrapping the scope also covers the transport's `GET` (SSE) and
//! `DELETE` alongside `POST`, which per-route plumbing would miss.
//!
//! ## Why it is always wrapped
//!
//! `main.rs` wraps `/mcp` with this unconditionally, because `Scope::wrap`
//! changes the scope's type and a conditional wrap would not typecheck. The cost
//! when OAuth is off is the first line below — the same shape as the blocklist's
//! `ARMED` fast path.

use actix_web::body::{BoxBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::http::{header, StatusCode};
use actix_web::middleware::Next;
use actix_web::{Error, HttpMessage, HttpResponse};

use crate::service::{
    blocklist::{self, Surface},
    log::{elog_with_ip, logger},
    mcp::{
        AUTHCODE_HEADER, McpAuth,
        mask_authcode,
        oauth::{self, store}
    }
};


/// The challenge that starts a connector's sign-in flow.
///
/// Without `resource_metadata` a client has nowhere to begin discovery, assumes
/// the server is simply unauthenticated, and never asks anyone to sign in — the
/// failure mode this whole module exists to fix.
fn challenge(problem: Option<(&str, &str)>) -> String {
    let mut value = format!("Bearer resource_metadata=\"{}\"", oauth::metadata_url());
    if let Some((error, description)) = problem {
        value.push_str(&format!(", error=\"{}\", error_description=\"{}\"", error, description));
    }
    value
}


fn unauthorized(problem: Option<(&str, &str)>) -> HttpResponse {
    HttpResponse::build(StatusCode::UNAUTHORIZED)
        .insert_header((header::WWW_AUTHENTICATE, challenge(problem)))
        .content_type("application/json")
        .body("{\"error\":\"invalid_token\",\"error_description\":\"sign in through the connector's OAuth flow\"}")
}


/// The bearer token a request presents, if any.
fn presented_bearer(request: &ServiceRequest) -> Option<String> {
    let value = request.headers().get(header::AUTHORIZATION)?.to_str().ok()?;
    // Case-insensitive scheme, per RFC 7235; the token itself is not.
    let (scheme, token) = value.split_once(' ')?;
    if !scheme.eq_ignore_ascii_case("Bearer") {
        return None
    }
    let token = token.trim();
    if token.is_empty() {
        return None
    }
    Some(token.to_string())
}


/// Whether the request carries the header identity `/mcp` has always accepted.
fn has_header_identity(request: &ServiceRequest) -> bool {
    request.headers()
        .get(AUTHCODE_HEADER)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| !value.trim().is_empty())
}


/// Guards the `/mcp` scope.
pub async fn guard(
    request: ServiceRequest,
    next: Next<impl MessageBody + 'static>
) -> Result<ServiceResponse<BoxBody>, Error> {
    if !oauth::is_enabled() {
        return next.call(request).await.map(ServiceResponse::map_into_boxed_body)
    }

    if let Some(token) = presented_bearer(&request) {
        let address = request.connection_info().realip_remote_addr().map(str::to_string);
        let address = address.unwrap_or_else(|| "unknown IP address".into());

        let Some(record) = store::resolve_access(&oauth::hash_secret(&token)) else {
            elog_with_ip(&address, "OAUTH: /mcp refused — unknown or expired bearer token");
            return Ok(request.into_response(unauthorized(Some(("invalid_token", "the access token is unknown or has expired")))))
        };
        let Some(grant) = store::find_grant(&record.grant_id) else {
            elog_with_ip(&address, "OAUTH: /mcp refused — the sign-in behind this token has been revoked");
            return Ok(request.into_response(unauthorized(Some(("invalid_token", "this sign-in has been revoked")))))
        };
        // RFC 8707 audience binding: a token minted for another resource must not
        // work here, however valid it is where it came from.
        if grant.resource != oauth::resource_uri() {
            elog_with_ip(&address, format!("OAUTH: /mcp refused — token audience '{}' is not this resource", grant.resource));
            return Ok(request.into_response(unauthorized(Some(("invalid_token", "this token was issued for another resource")))))
        }

        // Re-establish the blocklist check. The app-wide guard reads the code
        // from `X-Authcode` and an OAuth caller sends no such header, so without
        // this every authcode rule would silently stop matching on /mcp. IP rules
        // already matched before this point.
        if let Some(denied) = blocklist::refuse_if_blocked(&grant.authcode, Some(&address), Surface::Mcp) {
            return Ok(request.into_response(denied))
        }

        store::touch(&grant.id);
        logger(format!("MCP request: bearer token accepted ({})", grant.masked()));
        request.extensions_mut().insert(McpAuth { authcode: grant.authcode.clone(), pid: grant.pid });
        return next.call(request).await.map(ServiceResponse::map_into_boxed_body)
    }

    // The transition path: header callers keep working while `oauth_allow_headers`
    // is on. `curl`, `mcp-remote` and Claude Code can all set a header; the
    // claude.ai connector cannot, which is what the branch below is for.
    if oauth::allow_headers() && has_header_identity(&request) {
        return next.call(request).await.map(ServiceResponse::map_into_boxed_body)
    }

    if let Some(authcode) = request.headers()
        .get(AUTHCODE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty()) {
            // Headers present but switched off: say so, rather than letting the
            // caller read the 401 as "my code is wrong".
            let address = request.connection_info().realip_remote_addr().unwrap_or("unknown IP address").to_string();
            elog_with_ip(&address, format!(
                "OAUTH: /mcp refused — header authentication is disabled ({} presented)", mask_authcode(authcode)
            ));
            return Ok(request.into_response(unauthorized(Some((
                "invalid_token",
                "header authentication is disabled on this server — sign in through OAuth"
            )))))
    }

    Ok(request.into_response(unauthorized(None)))
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_challenge_always_points_at_the_metadata_document() {
        let value = challenge(None);
        assert!(value.starts_with("Bearer resource_metadata=\""));
        assert!(value.contains("/.well-known/oauth-protected-resource"));

        let with_problem = challenge(Some(("invalid_token", "expired")));
        assert!(with_problem.contains("error=\"invalid_token\""));
        assert!(with_problem.contains("error_description=\"expired\""));
    }
}
