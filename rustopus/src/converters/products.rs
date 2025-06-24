use chrono::{DateTime, Utc};

use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;
use crate::service::log::logger;
use crate::global::errors;

/// `async` Gets the products into string
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update: `&DateTime<Utc>`
/// # Returns
/// `String`
pub async fn get_data(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    match get_products_envelope(&get_products_xml(url, xmlns, authcode, web_update).await) {
        Ok(hu_envelope) => convert_products_envelope_to_xml(hu_envelope),
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_GET_DATA_ERROR)
    }
}


/// Get the string for the request
/// # Parameters
/// * xmlns: `&str`
/// * web_update: `&DateTime<Utc>`
/// * authcode: `&str`
/// # Returns
/// `String`
fn get_products_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetCikkekAuth xmlns="{}">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetCikkekAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


/// `async` Get products request
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update: `&DateTime<Utc>`
/// # Returns
/// `String`
pub async fn get_products_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_products_request_string(xmlns, web_update, authcode);
    soap::get_response(url, soap_request).await
}


/// Get envelope from xml string
/// # Parameters
/// * response_text: `&str`
/// # Returns
/// `Result<o8_xml::products::Envelope, quick_xml::DeError>`
pub fn get_products_envelope(response_text: &str) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


/// Converts products envelope to string
/// # Parameters
/// * hu_envelope: `o8_xml::products::Envelope`
/// # Returns
/// `String`
fn convert_products_envelope_to_xml(hu_envelope: o8_xml::products::Envelope) -> String {
    let en_envelope: partner_xml::products::Envelope = hu_envelope.into();
    match quick_xml::se::to_string(&en_envelope) {
        Ok(eng_xml) => eng_xml,
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_CONVERT_ERROR)
    }
}


/// Logs error and send error struct xml
/// # Parameters
/// * de_error: `quick_xml::DeError`
/// * error: `global::errors:RustopusError`
/// # Returns
/// `String`
fn log_and_send_error_xml(de_error: quick_xml::DeError, error: errors::RustopusError) -> String {
    logger(format!("{}: {} ({})", error.code, error.description, de_error));
    send_error_xml(error.code, error.description)
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
