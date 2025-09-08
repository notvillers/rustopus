use config::Config;
use serde::Deserialize;
use std::thread::available_parallelism;
use crate::service::log::elogger;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub server: ServerConfig
}


#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub timeout: u64,
    pub workers: usize
}


pub fn get_settings() -> Settings {
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
            }
        }
    }
}
