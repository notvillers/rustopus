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
        pub server: ServerConfig
    }

    #[derive(Clone)]
    pub struct ServerConfig {
        pub host: String,
        pub port: u16,
        pub timeout: u64,
        pub workers: usize
    }

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
            }
        }
    }
}
