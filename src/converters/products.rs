use chrono::{DateTime, Utc};

use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;
use crate::service::log::logger;


/// `async` Get XML data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update: `&DateTime<Utc>`
/// # Returns
/// `String`
pub async fn get_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    soap::get_response(url, o8_xml::products::get_request_string(xmlns, web_update, authcode)).await
}


/// Get envelope from xml string
/// # Parameters
/// * response_text: `&str`
/// # Returns
/// `Result<o8_xml::products::Envelope, quick_xml::DeError>`
pub fn get_envelope(response_text: &str) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


/// Send error struct xml
/// # Parameters
/// * code: `u64`
/// * description: `&str`
/// # Returns
/// `String`
pub fn send_error_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&partner_xml::products::error_struct(code, description)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
