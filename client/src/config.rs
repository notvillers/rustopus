use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "client_config.toml";

/// Folder name inside the user's platform config directory.
#[cfg(target_os = "linux")]
const CONFIG_DIR_NAME: &str = "rustopus-client";
#[cfg(not(target_os = "linux"))]
const CONFIG_DIR_NAME: &str = "Rustopus Client";

/// `%APPDATA%\Rustopus Client` (Windows),
/// `~/Library/Application Support/Rustopus Client` (macOS),
/// `~/.config/rustopus-client` (Linux).
fn config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join(CONFIG_DIR_NAME))
}

/// Resolves a data file in the working directory if it exists there (dev
/// runs from the repo root), otherwise in the platform config directory.
/// Legacy files next to the executable (the old location — inside the .app
/// bundle on macOS, next to the exe on Windows) are copied into the config
/// directory the first time they are looked up.
pub fn data_path(file_name: &str) -> PathBuf {
    let local = PathBuf::from(file_name);
    if local.exists() {
        return local;
    }

    let legacy = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join(file_name)));

    let Some(dir) = config_dir() else {
        // No resolvable home directory; stay next to the executable.
        return legacy.unwrap_or(local);
    };

    let standard = dir.join(file_name);
    if !standard.exists()
        && let Some(legacy) = legacy.filter(|path| path.exists())
    {
        let _ = fs::create_dir_all(&dir);
        let _ = fs::copy(&legacy, &standard);
    }
    standard
}

/// Creates the parent directory of `path` so a following write succeeds.
pub fn ensure_parent_dir(path: &std::path::Path) {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        let _ = fs::create_dir_all(parent);
    }
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
            let path = Self::config_path();
            ensure_parent_dir(&path);
            let _ = fs::write(path, content);
        }
    }
}
