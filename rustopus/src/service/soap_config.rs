use serde::{Serialize, Deserialize};
use std::fs;
use std::path::PathBuf;
use crate::service::path::get_current_or_root_dir;
use crate::service::log::logger;

pub fn get_soap_path() -> PathBuf {
    let mut path = get_current_or_root_dir();
    path.push("soap.json");
    path
}


pub fn check_soap_config() -> bool {
    get_soap_path().is_file()
}


#[derive(Debug, Serialize, Deserialize)]
pub struct SoapConfig {
    pub url: Option<String>
}


impl SoapConfig {
    pub fn load() -> Self {
        let path = get_soap_path();
        if get_soap_path().is_file() {
            match fs::read_to_string(&path) {
                Ok(content) => {
                    match serde_json::from_str::<SoapConfig>(&content) {
                        Ok(json) => {
                            return json
                        }
                        Err(error) => {
                            logger(format!("Can't read dict data from '{:#?}': {}", path, error));
                            SoapConfig {
                                url: None
                            }
                        }
                    }
                }
                Err(error) => {
                    logger(format!("Can't read '{:#?}': {}. (Do not bother this message if you are not willing to work with static 'url'.)", path, error));
                    SoapConfig {
                        url: None
                    }
                }
            }
        } else {
            SoapConfig {
                url: None
            }
        }
    }
}


pub fn get_default_url() -> Option<String> {
    match SoapConfig::load().url {
        Some(url) => Some(url),
        _ => None
    }
}
