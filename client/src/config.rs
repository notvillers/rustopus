use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "client_config.toml";

/// Resolves a data file next to the working directory if it exists there
/// (dev runs from the repo root), otherwise next to the executable —
/// Finder-launched .app bundles get `/` as their working directory.
pub fn data_path(file_name: &str) -> PathBuf {
    let local = PathBuf::from(file_name);
    if local.exists() {
        return local;
    }
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(file_name)))
        .unwrap_or(local)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_url: String,
    pub octopus_url: String,
    pub authcode: String,
    pub xmlns: String,
    pub pid: String,
    /// Start hidden with only a menu-bar icon (macOS) / tray icon (Windows);
    /// also makes the close button hide instead of quit on those platforms.
    #[serde(default)]
    pub start_minimized: bool,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:1140".to_string(),
            octopus_url: String::new(),
            authcode: String::new(),
            xmlns: String::new(),
            pid: String::new(),
            start_minimized: false,
        }
    }
}

impl ClientConfig {
    fn config_path() -> PathBuf {
        data_path(CONFIG_FILE)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Ok(content) = toml::to_string_pretty(self) {
            let _ = fs::write(Self::config_path(), content);
        }
    }
}
