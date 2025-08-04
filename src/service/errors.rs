use serde::Deserialize;
use std::env;
use std::fs::File;
use std::io::BufReader;

use crate::service::log::elogger;

use crate::global;

#[derive(Debug, Deserialize)]
pub struct ErrorMessage {
    pub hu: String,
    pub en: String
}


fn get_dummy_errors() -> Vec<ErrorMessage> {
    Vec::new()
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
                        Ok(errors) => return errors,
                        Err(e) => elogger(format!("errors.json file error: {}, returning dummy", e))
                    }
                }
                Err(e) => elogger(format!("errors.json file error: {}, returning dummy", e))
            }
        }
        Err(e) => elogger(format!("errors.json file error: {}, returning dummy", e))
    }
    get_dummy_errors()
}


pub fn translate_error(hungarian_error: &str) -> String {
    if let Some(error) = &global::errors::ERRORS.iter().find(|e| hungarian_error.starts_with(&e.hu)) {
        return error.en.clone()
    }
    hungarian_error.to_string()
}
