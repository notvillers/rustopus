use std::{
    fs,
    path::PathBuf,
    sync::OnceLock
};
use once_cell::sync::Lazy;
use serde::{Serialize, Deserialize};
use url::Url;

use crate::service::{
    config::get_settings,
    path::get_current_or_root_dir,
    log::{elogger, logger}
};

/// Cached default SOAP url, loaded once at startup from `soap.json`.
pub static SOAP_URL: OnceLock<Option<String>> = OnceLock::new();

/// This function get paths to `soap.json`
pub fn get_soap_path() -> PathBuf {
    let mut path = get_current_or_root_dir();
    path.push("soap.json");
    path
}


/// This function checks, if the soap is a file. 
pub fn check_soap_config() -> bool {
    get_soap_path().is_file()
}


/// `SoapConfig` struct
#[derive(Debug, Serialize, Deserialize)]
pub struct SoapConfig {
    pub url: Option<String>
}

impl Default for SoapConfig {
    /// Default for `SoapConfig`
    fn default() -> Self {
        Self {
            url: None
        }
    }
}


impl SoapConfig {
    /// Load for `SoapConfig` from soap file, or `default`
    pub fn load() -> Self {
        if get_soap_path().is_file() {
            match fs::read_to_string(get_soap_path()) {
                Ok(content) => {
                    match serde_json::from_str::<Self>(&content) {
                        Ok(config) => return config,
                        Err(error) => elogger(format!("Can't read dict data from '{:#?}': {}", get_soap_path(), error))
                    }
                }
                Err(error) => elogger(format!("Can't read '{:#?}': {}. (Do not bother this message, if you are not willing to work with static 'url'.)", get_soap_path(), error))
            }
        }
        Self {
            ..Default::default()
        }
    }
}


/// This function return default url if found (reads from cached `SOAP_URL`)
pub fn get_default_url() -> Option<String> {
    SOAP_URL.get().and_then(|v| v.clone())
}


/// One entry of the outbound allowlist: a host, and optionally the one port it
/// may be reached on.
///
/// An entry without a port allows any port on that host, which is what an
/// operator writing `orink.hu` means. Writing `orink.hu:443` pins it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AllowedHost {
    host: String,
    port: Option<u16>
}

impl AllowedHost {
    /// Parses one configured entry. Accepts a bare host, `host:port`, or a full
    /// url — an operator who pastes the whole wsdl address should not have to
    /// discover that only the host was wanted.
    fn parse(entry: &str) -> Option<Self> {
        let entry = entry.trim();
        if entry.is_empty() {
            return None
        }
        if let Ok(parsed) = Url::parse(entry)
            && let Some(host) = parsed.host_str() {
                // `port()` is `None` for a default port, which is the "any port"
                // shape — right, since a url written without one means the host.
                return Some(Self { host: host.to_lowercase(), port: parsed.port() })
        }
        // Not a url: `host` or `host:port`. Parsed through `Url` all the same, so
        // the host rules (including IPv6 brackets and IDNA) stay in one place.
        let parsed = Url::parse(&format!("soap://{}", entry)).ok()?;
        Some(Self {
            host: parsed.host_str()?.to_lowercase(),
            port: parsed.port()
        })
    }

    /// Whether a request url's host (and port) is this entry.
    fn covers(&self, host: &str, port: Option<u16>) -> bool {
        if self.host != host {
            return false
        }
        match self.port {
            // Pinned: the request has to be on that port.
            Some(allowed) => port == Some(allowed),
            None => true
        }
    }
}


/// Hosts an outbound SOAP call may be sent to, resolved once at startup.
///
/// `[server] allowed_soap_hosts` when it is set, otherwise the single host of
/// `soap.json`'s url. Empty means **nothing is allowed**: this list is what
/// stops a caller-supplied `?url=` from turning the service into a request proxy
/// for whatever the host can reach, so an unconfigured instance has to fail
/// closed rather than open.
static ALLOWED_HOSTS: Lazy<Vec<AllowedHost>> = Lazy::new(|| {
    let configured: Vec<String> = get_settings().server.allowed_soap_hosts.clone().unwrap_or_default();
    let configured: Vec<String> = configured.into_iter()
        .map(|entry| entry.trim().to_string())
        .filter(|entry| !entry.is_empty())
        .collect();

    let source = if configured.is_empty() {
        // Nothing configured: the one Octopus this instance talks to anyway.
        get_default_url().into_iter().collect()
    } else {
        configured
    };

    let hosts: Vec<AllowedHost> = source.iter()
        .filter_map(|entry| match AllowedHost::parse(entry) {
            Some(host) => Some(host),
            None => {
                elogger(format!("SOAP allowlist: '{}' is not a host, host:port or url — ignored", entry));
                None
            }
        })
        .collect();

    if hosts.is_empty() {
        elogger(
            "SOAP allowlist: no allowed hosts — every request carrying a 'url' parameter will be refused. \
             Set [server] allowed_soap_hosts in Config.toml, or configure soap.json."
        );
    } else {
        logger(format!(
            "SOAP allowlist: {} host(s) allowed — {}",
            hosts.len(),
            hosts.iter()
                .map(|host| match host.port {
                    Some(port) => format!("{}:{}", host.host, port),
                    None => host.host.clone()
                })
                .collect::<Vec<String>>()
                .join(", ")
        ));
    }

    hosts
});


/// Whether an outbound SOAP call may be sent to this url.
///
/// The `url` request parameter is caller-controlled on every fetcher and on
/// `/post-order`, and it decides where this server opens a connection — without
/// this check any unauthenticated caller can make the process POST an arbitrary
/// body to anything it can route to (internal services, cloud metadata, a third
/// party attacked from this machine's address). The parameter stays, because a
/// deployment may legitimately front more than one Octopus; where it may point
/// is now configuration rather than the caller's choice.
///
/// Scheme is checked as well as host: only `http` and `https` ever reach an
/// Octopus, and a scheme this service does not speak has no business being
/// attempted.
pub fn is_allowed_soap_url(url: &str) -> bool {
    let Ok(parsed) = Url::parse(url.trim()) else {
        return false
    };
    if !matches!(parsed.scheme(), "http" | "https") {
        return false
    }
    let Some(host) = parsed.host_str() else {
        return false
    };
    let host = host.to_lowercase();
    // `port_or_known_default` resolves 80/443 so `orink.hu:443` pins what the
    // caller wrote as plain `https://orink.hu/...`.
    let port = parsed.port_or_known_default();

    ALLOWED_HOSTS.iter().any(|allowed| allowed.covers(&host, port))
}


/// Loads the allowlist so it is reported at startup rather than on the first
/// request. Called from `main.rs` after `SOAP_URL` is set — the default url is
/// the fallback source, so the order matters.
pub fn init_allowlist() {
    Lazy::force(&ALLOWED_HOSTS);
}


#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(entry: &str) -> AllowedHost {
        AllowedHost::parse(entry).expect("entry parses")
    }

    /// The check `is_allowed_soap_url` performs, against an explicit list —
    /// `ALLOWED_HOSTS` reads `Config.toml`, which a unit test has no business
    /// depending on.
    fn permits(hosts: &[AllowedHost], url: &str) -> bool {
        let Ok(parsed) = Url::parse(url.trim()) else {
            return false
        };
        if !matches!(parsed.scheme(), "http" | "https") {
            return false
        }
        let Some(host) = parsed.host_str() else {
            return false
        };
        let host = host.to_lowercase();
        let port = parsed.port_or_known_default();
        hosts.iter().any(|allowed| allowed.covers(&host, port))
    }

    #[test]
    fn an_entry_may_be_a_host_a_host_port_or_a_whole_url() {
        assert_eq!(allowed("orink.hu"), AllowedHost { host: "orink.hu".into(), port: None });
        assert_eq!(allowed("orink.hu:8443"), AllowedHost { host: "orink.hu".into(), port: Some(8443) });
        // A pasted wsdl address means its host, not its path.
        assert_eq!(allowed("https://orink.hu/services/vision.asmx"), AllowedHost { host: "orink.hu".into(), port: None });
        assert_eq!(allowed("https://orink.hu:8443/services/"), AllowedHost { host: "orink.hu".into(), port: Some(8443) });
        // Case is not part of a host name.
        assert_eq!(allowed("ORINK.HU").host, "orink.hu");
        assert!(AllowedHost::parse("   ").is_none());
    }

    #[test]
    fn only_the_allowed_host_is_reachable() {
        let hosts = vec![allowed("orink.hu")];
        assert!(permits(&hosts, "https://orink.hu/services/vision.asmx"));
        assert!(permits(&hosts, "http://orink.hu/services/vision.asmx"));
        assert!(permits(&hosts, "https://ORINK.hu/services/vision.asmx"));

        // The SSRF targets this exists to refuse.
        assert!(!permits(&hosts, "http://169.254.169.254/latest/meta-data/"));
        assert!(!permits(&hosts, "http://127.0.0.1:1140/get-product"));
        assert!(!permits(&hosts, "http://10.0.0.5:8080/"));
        assert!(!permits(&hosts, "https://attacker.test/collect"));
    }

    #[test]
    fn a_lookalike_host_is_not_the_allowed_host() {
        let hosts = vec![allowed("orink.hu")];
        // Suffix, prefix and userinfo tricks all have to miss — this is a host
        // comparison, not a string comparison.
        assert!(!permits(&hosts, "https://orink.hu.attacker.test/services/"));
        assert!(!permits(&hosts, "https://notorink.hu/services/"));
        assert!(!permits(&hosts, "https://orink.hu@attacker.test/services/"));
        assert!(!permits(&hosts, "https://attacker.test/?x=orink.hu"));
        // A subdomain is a different host; list it if it is wanted.
        assert!(!permits(&hosts, "https://api.orink.hu/services/"));
    }

    #[test]
    fn a_pinned_port_is_enforced_and_a_bare_host_allows_any() {
        let pinned = vec![allowed("orink.hu:8443")];
        assert!(permits(&pinned, "https://orink.hu:8443/services/"));
        assert!(!permits(&pinned, "https://orink.hu/services/"));
        assert!(!permits(&pinned, "https://orink.hu:22/"));

        // Written without a port, the entry's own default resolves the same way
        // the request's does, so `https://orink.hu` still matches `orink.hu:443`.
        let default_port = vec![allowed("https://orink.hu:443/services/")];
        assert!(permits(&default_port, "https://orink.hu/services/"));

        let any = vec![allowed("orink.hu")];
        assert!(permits(&any, "https://orink.hu:8443/services/"));
        assert!(permits(&any, "https://orink.hu:22/"));
    }

    #[test]
    fn a_scheme_this_service_does_not_speak_is_refused() {
        let hosts = vec![allowed("orink.hu")];
        assert!(!permits(&hosts, "file://orink.hu/etc/passwd"));
        assert!(!permits(&hosts, "gopher://orink.hu:70/"));
        assert!(!permits(&hosts, "not a url at all"));
        assert!(!permits(&hosts, ""));
    }

    #[test]
    fn an_empty_allowlist_permits_nothing() {
        // The fail-closed case: no `allowed_soap_hosts` and no `soap.json`.
        assert!(!permits(&[], "https://orink.hu/services/vision.asmx"));
    }
}
