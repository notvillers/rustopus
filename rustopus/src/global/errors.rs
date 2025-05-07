use once_cell::sync::Lazy;
use crate::service::errors;

fn init_errors() -> Vec<errors::ErrorMessage> {
    errors::read_errors()
}

pub const ERRORS: Lazy<Vec<errors::ErrorMessage>> = Lazy::new(init_errors);
