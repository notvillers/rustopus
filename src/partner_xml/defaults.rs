/// Default english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;

use crate::o8_xml;
use crate::service::errors;
use crate::global;

#[derive(Serialize, Clone)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl Error {
    pub fn load<S: AsRef<str>>(code: u64, description: S) -> Self {
        Error {
            code: code,
            description: description.as_ref().into()
        }
    }
}

impl From<o8_xml::defaults::Hiba> for Error {
    fn from(e: o8_xml::defaults::Hiba) -> Self {
        Error {
            code: e.kod,
            description: errors::translate_error(&e.leiras)
        }
    }
}

impl From<global::errors::RustopusError> for Error {
    fn from(e: global::errors::RustopusError) -> Self {
        Error {
            code: e.code,
            description: e.description.into()
        }
    }
}

impl From<&o8_xml::defaults::Hiba> for Error {
    fn from(e: &o8_xml::defaults::Hiba) -> Self {
        Error {
            code: e.kod,
            description: e.leiras.clone()
        }
    }
}
