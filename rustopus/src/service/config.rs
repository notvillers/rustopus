use config::Config;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Settings {
    pub server: ServerConfig
}


#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub port: u16,
    pub timeout: u64
}


pub fn get_settings() -> Settings {
    let config = Config::builder()
        .add_source(config::File::with_name("Config"))
        .build();

    match config {
        Ok(config) => {
            let settings: Result<Settings, config::ConfigError> = config.try_deserialize();
            match settings {
                Ok(settings) => {
                    return settings
                }
                Err(e) => {
                    println!("Config settings error: {}", e);
                }
            }
        }
        Err(e) => {
            println!("Config config error: {}", e);
        }
    }
    Settings { 
        server: ServerConfig { 
            port: 8080,
            timeout: 1200
        }
    }
}