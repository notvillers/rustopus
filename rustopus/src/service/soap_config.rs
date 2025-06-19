use serde::{Serialize, Deserialize};
use std::fs;
use crate::service::path::get_current_or_root_dir;

#[derive(Debug, Serialize, Deserialize)]
pub struct SoapConfig {
    pub url: Option<String>
}

impl SoapConfig {
    pub fn load() -> Self {
        let mut path = get_current_or_root_dir();
        path.push("soap.json");

        fs::read_to_string(&path)
            .ok()
            .and_then(|content| serde_json::from_str::<SoapConfig>(&content).ok())
            .unwrap_or(SoapConfig {
                    url: None
                }
            )
    }
}


pub fn get_default_url() ->Option<String> {
    match SoapConfig::load().url {
        Some(url) => Some(url),
        _ => None
    }
}
