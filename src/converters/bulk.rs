use quick_xml;
use crate::partner_xml;
use crate::service::log::logger;

/// Send error struct xml
/// # Parameters
/// * code: `u64`
/// * description: `&str`
/// # Returns
/// `String`
pub fn send_error_xml(code: u64, description: &str) -> String {
    let errors: Vec<partner_xml::defaults::Error> = vec![
        partner_xml::defaults::Error {
            code: code,
            description: description.to_string()
        }
    ];
    match quick_xml::se::to_string(&partner_xml::bulk::error_struct(errors)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
