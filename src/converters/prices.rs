use crate::partner_xml;
use quick_xml;
use crate::service::log::logger;

/// Send error struct xml
/// # Parameters
/// * code: `u64`
/// * description: `&str`
/// # Returns
/// `String`
pub fn send_error_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&partner_xml::prices::error_struct(code, description)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
