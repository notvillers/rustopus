use once_cell::sync::Lazy;
use crate::service::errors;

fn init_errors() -> Vec<errors::ErrorMessage> {
    errors::read_errors()
}

pub const ERRORS: Lazy<Vec<errors::ErrorMessage>> = Lazy::new(init_errors);

pub struct RustopusError {
    pub code: u64,
    pub description: &'static str
}

pub const GLOBAL_AUTH_ERROR: RustopusError = RustopusError {
    code: 101,
    description: "Missing authcode"
};

pub const GLOBAL_URL_ERROR: RustopusError = RustopusError {
    code: 102,
    description: "Missing url (this can be a server side error, if not configured properly.)"
};

pub const GLOBAL_PID_ERROR: RustopusError = RustopusError {
    code: 103,
    description: "Missing PID"
};
