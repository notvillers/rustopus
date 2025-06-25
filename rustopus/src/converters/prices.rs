use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;
use crate::service::log::logger;
use crate::global::errors;

/// `async` Get the data into reformatted string
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `String`
pub async fn get_data(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> String {
    let hu_prices_xml = get_xml(url, xmlns, pid, authcode).await;
    match get_envelope(&hu_prices_xml) {
        Ok(hu_envelope) => convert_envelope_to_xml(hu_envelope),
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_GET_DATA_ERROR)
    }
}


/// `async` Get XML data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update: `&DateTime<Utc>`
/// # Returns
/// `String`
pub async fn get_xml(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> String {
    soap::get_response(url, o8_xml::prices::get_request_string(xmlns, authcode, pid)).await
}


/// Get envelope from xml string
/// # Parameters
/// * response_text: `&str`
/// # Returns
/// `Result<o8_xml::prices::Envelope, quick_xml::DeError>`
pub fn get_envelope(response_text: &str) -> Result<o8_xml::prices::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


/// Converts envelope struct to string
/// # Parameters
/// * hu_envelope: `o8_xml::prices::Envelope`
/// # Returns
/// `String`
fn convert_envelope_to_xml(hu_envelope: o8_xml::prices::Envelope) -> String {
    match quick_xml::se::to_string(&hu_envelope.to_en()) {
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
    match quick_xml::se::to_string(&partner_xml::prices::error_struct(code, description)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
