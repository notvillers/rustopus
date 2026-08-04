use config::Config;
use std::thread::available_parallelism;
use once_cell::sync::Lazy;

use crate::{
    macros::service::ConfigModelDerive,
    service::log::elogger
};

ConfigModelDerive! {
    #[derive(Clone)]
    pub struct Settings {
        pub server: ServerConfig,
        // Optional for the same reason as `soap_concurrency` below: a
        // `Config.toml` written before the MCP endpoint existed has no `[mcp]`
        // table at all, and a missing required field fails the whole parse.
        pub mcp: Option<McpConfig>
    }

    #[derive(Clone)]
    pub struct ServerConfig {
        pub host: String,
        pub port: u16,
        pub timeout: u64,
        pub workers: usize,
        // Optional so existing Config.toml files without the key keep
        // deserializing (a missing required field would fail the whole parse
        // and silently fall back to the hardcoded defaults, port 8080 included).
        pub soap_concurrency: Option<usize>
    }

    /// `[mcp]` table. Every field is `Option` and every default is applied in
    /// code (see the accessors below), so a partial table never fails the parse.
    #[derive(Clone)]
    pub struct McpConfig {
        pub enabled: Option<bool>,
        pub max_bytes: Option<u64>,
        pub ttl_secs: Option<u64>,
        pub precache_interval_secs: Option<u64>,
        pub admin_token: Option<String>,
        pub disk_path: Option<String>,
        pub disk_max_bytes: Option<u64>,
        pub export_path: Option<String>,
        pub export_ttl_secs: Option<u64>,
        pub public_url: Option<String>
    }

}


/// In-**memory** cache budget when `[mcp] max_bytes` is unset: 300 MB.
///
/// Sized for a 1–1.5 GB host holding ~46 MB snapshots, which leaves room for the
/// three things that also need memory at once: the actix server itself, the peak
/// of a snapshot build (the raw ~46 MB XML response plus the parsed structures
/// derived from it), and moka's asynchronous eviction, which lets the cache
/// briefly exceed its cap. Snapshots beyond this budget are not lost — they fall
/// through to the disk store, which is far cheaper than a rebuild.
///
/// Raise it only against measured headroom on the target host.
const DEFAULT_MCP_MAX_BYTES: u64 = 300_000_000;

/// Directory for the on-disk snapshot store when `[mcp] disk_path` is unset.
/// Relative paths resolve against the working directory, like every other
/// runtime path in this service.
const DEFAULT_MCP_DISK_PATH: &str = "mcp_cache";

/// On-**disk** budget when `[mcp] disk_max_bytes` is unset: 5 GB. Stored
/// snapshots are gzipped, so this holds far more combinations than the number
/// suggests.
const DEFAULT_MCP_DISK_MAX_BYTES: u64 = 5_000_000_000;

/// Directory generated exports are written to when `[mcp] export_path` is unset.
const DEFAULT_MCP_EXPORT_PATH: &str = "mcp_exports";

/// How long a download link stays valid when `[mcp] export_ttl_secs` is unset:
/// 1 hour. Long enough for someone to notice the message and click; short enough
/// that a leaked link is not a standing exposure of a partner's prices.
const DEFAULT_MCP_EXPORT_TTL_SECS: u64 = 3_600;

/// Base URL used to build download links when `[mcp] public_url` is unset.
///
/// The local default only works for local testing; a deployment **must** set
/// this to the hostname colleagues' browsers can actually reach, because the MCP
/// transport gives a tool no view of the request's `Host` header.
const DEFAULT_MCP_PUBLIC_URL: &str = "http://localhost:1140";

/// Snapshot lifetime when `[mcp] ttl_secs` is unset: 6 hours.
const DEFAULT_MCP_TTL_SECS: u64 = 21_600;

/// Precache sweep interval when `[mcp] precache_interval_secs` is unset: 1 hour.
const DEFAULT_MCP_PRECACHE_INTERVAL_SECS: u64 = 3_600;

/// Environment variable checked before `[mcp] admin_token`. Preferred, because
/// `Config.toml` is tracked in git and a token written there gets committed.
const ADMIN_TOKEN_ENV: &str = "RUSTOPUS_ADMIN_TOKEN";

impl McpConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(false)
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes.unwrap_or(DEFAULT_MCP_MAX_BYTES)
    }

    pub fn ttl_secs(&self) -> u64 {
        self.ttl_secs.unwrap_or(DEFAULT_MCP_TTL_SECS)
    }

    pub fn precache_interval_secs(&self) -> u64 {
        self.precache_interval_secs.unwrap_or(DEFAULT_MCP_PRECACHE_INTERVAL_SECS)
    }

    pub fn disk_path(&self) -> String {
        self.disk_path.as_ref()
            .filter(|path| !path.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| DEFAULT_MCP_DISK_PATH.to_string())
    }

    pub fn disk_max_bytes(&self) -> u64 {
        self.disk_max_bytes.unwrap_or(DEFAULT_MCP_DISK_MAX_BYTES)
    }

    pub fn export_path(&self) -> String {
        self.export_path.as_ref()
            .filter(|path| !path.trim().is_empty())
            .cloned()
            .unwrap_or_else(|| DEFAULT_MCP_EXPORT_PATH.to_string())
    }

    pub fn export_ttl_secs(&self) -> u64 {
        self.export_ttl_secs.unwrap_or(DEFAULT_MCP_EXPORT_TTL_SECS)
    }

    /// Base URL for generated download links, with any trailing slash removed so
    /// callers can append a path without doubling the separator.
    pub fn public_url(&self) -> String {
        let configured = self.public_url.as_ref()
            .map(|url| url.trim())
            .filter(|url| !url.is_empty())
            .unwrap_or(DEFAULT_MCP_PUBLIC_URL);
        configured.trim_end_matches('/').to_string()
    }

    /// Admin dashboard token: `RUSTOPUS_ADMIN_TOKEN` wins over `Config.toml`.
    /// `None` (or blank in both places) means `/admin` is not registered at all
    /// — the dashboard is never served unauthenticated.
    pub fn admin_token(&self) -> Option<String> {
        if let Ok(token) = std::env::var(ADMIN_TOKEN_ENV)
            && !token.trim().is_empty() {
                return Some(token)
        }
        self.admin_token.as_ref()
            .filter(|token| !token.trim().is_empty())
            .cloned()
    }
}


/// The `[mcp]` table, or an all-defaults (disabled) one when the table is absent.
pub fn get_mcp_settings() -> McpConfig {
    get_settings().mcp.unwrap_or(McpConfig {
        enabled: None,
        max_bytes: None,
        ttl_secs: None,
        precache_interval_secs: None,
        admin_token: None,
        disk_path: None,
        disk_max_bytes: None,
        export_path: None,
        export_ttl_secs: None,
        public_url: None
    })
}


/// `Config.toml` is parsed from disk once; every `get_settings()` call clones
/// from this cached view instead of re-reading the file.
static SETTINGS: Lazy<Settings> = Lazy::new(load_settings);


/// This functions gets `Settings` struct from `Config.toml` based in the root directory.
pub fn get_settings() -> Settings {
    SETTINGS.clone()
}


/// Reads and deserializes `Config.toml`, falling back to defaults on any error.
fn load_settings() -> Settings {
    match Config::builder().add_source(config::File::with_name("Config")).build() {
        Ok(config) => {
            match config.try_deserialize::<Settings>() {
                Ok(settings) => return settings,
                Err(error) => elogger(format!("Config settings error: {}", error))
            }
        }
        Err(e) => elogger(format!("Config config error: {}", e))
    }
    Settings { 
        server: ServerConfig {
            host: "0.0.0.0".into(),
            port: 8080,
            timeout: 1200,
            workers: match available_parallelism() {
                Ok(workers) => workers.into(),
                Err(error) => {
                    elogger(format!("Error getting available_parallelism(): {}", error));
                    1
                }
            },
            soap_concurrency: None
        },
        mcp: None
    }
}
