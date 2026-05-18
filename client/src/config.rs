use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "client_config.toml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClientConfig {
    pub server_url: String,
    pub octopus_url: String,
    pub authcode: String,
    pub xmlns: String,
    pub pid: String,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            server_url: "http://localhost:1140".to_string(),
            octopus_url: String::new(),
            authcode: String::new(),
            xmlns: String::new(),
            pid: String::new(),
        }
    }
}

impl ClientConfig {
    fn config_path() -> PathBuf {
        PathBuf::from(CONFIG_FILE)
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
