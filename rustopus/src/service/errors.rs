use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::BufReader;

use crate::service::log::logger;

use crate::global;

#[derive(Debug, Deserialize)]
pub struct ErrorMessage {
    pub code: u64,
    pub hu: String,
    pub en: String
}


fn get_dummy_errors() -> Vec<ErrorMessage> {
    let dummy_errors: Vec<ErrorMessage> = Vec::new();
    dummy_errors
}


pub fn read_errors() -> Vec<ErrorMessage> {
    match env::current_dir() {
        Ok(current_dir) => {
            let errors_path = current_dir.join("src").join("errors").join("errors.json");
            match File::open(errors_path) {
                Ok(file) => {
                    let reader = BufReader::new(file);
                    let json: Result<Vec<ErrorMessage>, serde_json::Error> = serde_json::from_reader(reader);
                    match json {
                        Ok(errors) => {
                            errors
                        },
                        Err(e) => {
                            logger(format!("errors.json file error: {}, returning dummy", e));
                            get_dummy_errors()
                        }
                    }
                }
                Err(e) => {
                    logger(format!("errors.json file error: {}, returning dummy", e));
                    get_dummy_errors()
                }
            }
        }
        Err(e) => {
            logger(format!("errors.json file error: {}, returning dummy", e));
            get_dummy_errors()
        }
    }
}


pub fn translate_error(hungarian_error: &str) -> String {
    let errors = &global::errors::ERRORS;
    if let Some(error) = errors.iter().find(|e | hungarian_error.starts_with(&e.hu)) {
        return error.en.clone()
    }
    hungarian_error.to_string()
}