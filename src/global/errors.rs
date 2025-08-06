use once_cell::sync::Lazy;
use crate::service::errors;

/// Initalizes ./errors/errors.json
/// # Returns
/// `Vec<errors::ErrorMessage>`
fn init_errors() -> Vec<errors::ErrorMessage> {
    errors::read_errors()
}

/// Error initalized in ./errors/errors.json
pub const ERRORS: Lazy<Vec<errors::ErrorMessage>> = Lazy::new(init_errors);

pub struct RustopusError {
    pub code: u64,
    pub description: &'static str
}


pub const GLOBAL_AUTH_ERROR: RustopusError = RustopusError {
    code: 201,
    description: "Missing authcode"
};

pub const GLOBAL_URL_ERROR: RustopusError = RustopusError {
    code: 202,
    description: "Missing url (this can be a server side error, if not configured properly.)"
};

pub const GLOBAL_PID_ERROR: RustopusError = RustopusError {
    code: 203,
    description: "Missing PID"
};

pub const GLOBAL_MISSING_ERROR: RustopusError = RustopusError {
    code: 299,
    description: "Missing value"
};

pub const GLOBAL_GET_DATA_ERROR: RustopusError = RustopusError {
    code: 304,
    description: "Get data error"
};

pub const GLOBAL_CONVERT_ERROR: RustopusError = RustopusError {
    code: 401,
    description: "Envelope convert error"
};

pub const BULK_GET_PRODUCTS_ERROR: RustopusError = RustopusError {
    code: 501,
    description: "Bulk products error"
};

pub const BULK_GET_PRICES_ERROR: RustopusError = RustopusError {
    code: 502,
    description: "Bulk prices error"
};

pub const BULK_GET_STOCKS_ERROR: RustopusError = RustopusError {
    code: 503,
    description: "Bulk stocks error"
};

pub const BULK_GET_IMAGES_ERROR: RustopusError = RustopusError {
    code: 504,
    description: "Bulk images error"
};

pub const BULK_GET_BARCODES_ERROR: RustopusError = RustopusError {
    code: 505,
    description: "Bulk barcodes error"
};

pub const UNDEFINED_ERROR: RustopusError = RustopusError {
    code: 999,
    description: "Undefined error"
};
