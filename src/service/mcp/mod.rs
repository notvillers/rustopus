//! MCP (Model Context Protocol) endpoint.
//!
//! Everything under this module is reachable only when `[mcp] enabled = true`
//! in `Config.toml`. With the flag off, `main.rs` registers no MCP route, spawns
//! no precache task and builds no cache, so the eight existing REST endpoints
//! run exactly as they did before this module existed.
//!
//! The cache lives here rather than in `service/soap.rs` on purpose: caching at
//! the SOAP layer would silently change all eight existing endpoints, whose
//! consumers expect a live read on every call.

pub mod admin;
pub mod cache;
pub mod index;
pub mod precache;
pub mod store;
pub mod tools;

/// Header carrying the caller's Octopus authentication code.
///
/// Deliberately **not** `Authorization`: `rmcp-actix-web` forwards
/// `Authorization` to MCP services, and its own docs warn that passing those
/// upstream (the proxy pattern) violates the MCP spec.
pub const AUTHCODE_HEADER: &str = "X-Authcode";

/// Header carrying the caller's partner id (`pid`), fixed per user.
pub const PID_HEADER: &str = "X-Pid";

/// Renders an authcode as `FFD3…0E37` — first four and last four characters.
///
/// The full code must never reach a log line, a cache key, a dashboard response
/// or an error message, so every place that wants to identify a caller goes
/// through this. Short codes are masked wholesale rather than leaked.
pub fn mask_authcode(authcode: &str) -> String {
    let chars: Vec<char> = authcode.chars().collect();
    if chars.len() < 12 {
        // Too short to reveal 8 of its characters without giving most of it away.
        return "…".repeat(chars.len().min(4))
    }
    let head: String = chars.iter().take(4).collect();
    let tail: String = chars.iter().skip(chars.len() - 4).collect();
    format!("{}…{}", head, tail)
}


/// Per-request caller identity, lifted out of the HTTP headers by the
/// `on_request` hook and read back inside the tools through
/// `RequestContext::extensions`.
#[derive(Clone, Debug)]
pub struct McpAuth {
    pub authcode: String,
    pub pid: i64
}

impl McpAuth {
    /// The caller's masked identity, safe to log.
    pub fn masked(&self) -> String {
        format!("{} pid={}", mask_authcode(&self.authcode), self.pid)
    }
}


#[cfg(test)]
mod tests {
    use super::mask_authcode;

    #[test]
    fn masks_all_but_first_and_last_four() {
        assert_eq!(mask_authcode("FFD3ABCDEF120E37"), "FFD3…0E37");
    }

    #[test]
    fn short_codes_are_masked_wholesale() {
        // Nothing of a short code is revealed: 8 visible characters out of 10
        // would be a leak, not a mask.
        assert_eq!(mask_authcode("SHORT12345"), "…………");
        assert_eq!(mask_authcode(""), "");
    }
}
