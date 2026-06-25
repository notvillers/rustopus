use std::{
    env,
    fs::File,
    io::BufReader
};
use macro_rules_attribute::apply;

use crate::{
    macros::service::ConfigModelDerive,
    service::log::elogger,
    global::errors::ERRORS
};



#[apply(ConfigModelDerive)]
pub struct ErrorMessage {
    pub hu: String,
    pub en: String
}


/// This function reads errors from `./src/errors/errors.json`
pub fn read_errors() -> Vec<ErrorMessage> {
    match env::current_dir() {
        Ok(current_dir) => {
            match File::open(current_dir.join("src").join("errors").join("errors.json")) {
                Ok(file) => {
                    match serde_json::from_reader::<_, Vec<ErrorMessage>>(BufReader::new(file)) {
                        Ok(error_messages) => return error_messages,
                        Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
                    }
                }
                Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
            }
        }
        Err(error) => elogger(format!("errors.json file error: {}, returning dummy", error))
    }
    vec![]
}


/// This function translates `HU` errors to `EN`
pub fn translate_error(hungarian_error: &str) -> String {
    if let Some(error_message) = &ERRORS.iter().find(|x| hungarian_error.starts_with(&x.hu)) {
        return error_message.en.clone()
    }
    format!("{} (Can not translate error)", hungarian_error)
}
