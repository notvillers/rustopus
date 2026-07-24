use std::{
    fs,
    path::PathBuf,
    sync::OnceLock
};
use serde::{Serialize, Deserialize};

use crate::service::{
    path::get_current_or_root_dir,
    log::elogger
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
