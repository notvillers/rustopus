/// Default english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;

use crate::o8_xml::defaults::Hiba as o8_error;
use crate::service::errors;
use crate::global::errors::RustopusError;

#[derive(Serialize, Clone)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl Error {
    pub fn load<S: AsRef<str>>(code: u64, description: S) -> Self {
        Self {
            code: code,
            description: description.as_ref().into()
        }
    }
}

impl From<o8_error> for Error {
    fn from(e: o8_error) -> Self {
        Self {
            code: e.kod,
            description: errors::translate_error(&e.leiras)
        }
    }
}

impl From<RustopusError> for Error {
    fn from(e: RustopusError) -> Self {
        Self {
            code: e.code,
            description: e.description.into()
        }
    }
}

impl From<&o8_error> for Error {
    fn from(e: &o8_error) -> Self {
        Self {
            code: e.kod,
            description: e.leiras.clone()
        }
    }
}
