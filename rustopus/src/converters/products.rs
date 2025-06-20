use chrono::{DateTime, Utc};

use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;
use crate::service::log::logger;


/// `async` Gets the products into string
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update: `&DateTime<Utc>`
/// # Returns
/// `String`
/// # Example
/// ```rust
/// let products_string: String = get_products(url, xmlns, authcode, web_update).await;
/// ```
pub async fn get_data(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    match get_products_envelope(&get_products_xml(url, xmlns, authcode, web_update).await) {
        Ok(hu_envelope) => {
            convert_products_envelope_to_xml(hu_envelope)
        }
        Err(e) => {
            logger(format!("Get products error: {}", e));
            "<Envelope></Envelope>".to_string()
        }
    }
}


/// Get the string for the request
/// # Parameters
/// * xmlns: `&str`
/// * web_update: `&DateTime<Utc>`
/// * authcode: `&str`
/// # Returns
/// `String`
/// # Example
/// ```rust
/// let soap_request: String = get_products_request_string(xmlns, web_update, authcode);
/// ```
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
/// # Example
/// ```rust
/// let request_string: Stirng = get_products_xml(url, xmlns, autchode, web_update).await;
/// ```
pub async fn get_products_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_products_request_string(xmlns, web_update, authcode);
    soap::get_response(url, soap_request).await
}


/// Get envelope from xml string
/// # Parameters
/// * response_text: `&str`
/// # Returns
/// `Result<o8_xml::products::Envelope, quick_xml::DeError>`
/// # Example
/// ```rust
///    match get_products_envelope(&hu_products_xml) {
///        Ok(hu_envelope) => {
///            convert_products_envelope_to_xml(hu_envelope)
///        }
///        Err(_) => {
///            "<Envelope></Envelope>".to_string()
///        }
///    }
///}
/// ```
pub fn get_products_envelope(response_text: &str) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


/// Converts products envelope to string
/// # Parameters
/// * hu_envelope: `o8_xml::products::Envelope`
/// # Returns
/// `String`
/// # Example
/// ```rust
/// let xml_string: String = convert_products_envelope_to_xml(hu_envelope);
/// ```
fn convert_products_envelope_to_xml(hu_envelope: o8_xml::products::Envelope) -> String {
    let en_envelope: partner_xml::products::Envelope = hu_envelope.into();
    match quick_xml::se::to_string(&en_envelope) {
        Ok(eng_xml) => {
            eng_xml
        }
        Err(e) => {
            logger(format!("Convert products error: {}", e));
            "<Envelope></Envelope>".to_string()
        }
    }
}


pub fn send_error_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&partner_xml::products::error_struct(code, description)) {
        Ok(e_xml) => {
            e_xml
        }
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
