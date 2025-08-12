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
    vec![]
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
                        Ok(error_messages) => return error_messages,
                        Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
                    }
                }
                Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
            }
        }
        Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
    }
    get_dummy_errors()
}


pub fn translate_error(hungarian_error: &str) -> String {
    if let Some(error_message) = &global::errors::ERRORS.iter().find(|error| hungarian_error.starts_with(&error.hu)) {
        return error_message.en.clone()
    }
    hungarian_error.to_string()
}
