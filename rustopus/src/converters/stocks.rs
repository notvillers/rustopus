use crate::o8_xml;
use crate::partner_xml;
use quick_xml;
use chrono::{DateTime, Utc};
use crate::service::soap;
use crate::service::log::logger;
use crate::global::errors;

pub async fn get_data(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let hu_stocks_xml = get_stocks_xml(url, xmlns, authcode, web_update).await;
    match get_stocks_envelope(&hu_stocks_xml) {
        Ok(hu_envelope) => convert_stocks_envelope_to_xml(hu_envelope),
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_GET_DATA_ERROR)
    }
}


fn get_stocks_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetCikkekKeszletValtozasAuth xmlns="{}">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetCikkekKeszletValtozasAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


pub async fn get_stocks_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_stocks_request_string(xmlns, web_update, authcode);
    soap::get_response(url, soap_request).await
}


pub fn get_stocks_envelope(response_text: &str) -> Result<o8_xml::stocks::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


fn convert_stocks_envelope_to_xml(hu_envelope: o8_xml::stocks::Envelope) -> String {
    let en_envelope: partner_xml::stocks::Envelope = hu_envelope.into();
    match quick_xml::se::to_string(&en_envelope) {
        Ok(eng_xml) => eng_xml,
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_CONVERT_ERROR)
    }
}


fn log_and_send_error_xml(de_error: quick_xml::DeError, error: errors::RustopusError) -> String {
    logger(format!("{}: {} ({})", error.code, error.description, de_error));
    send_error_xml(error.code, error.description)
}


pub fn send_error_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&partner_xml::stocks::error_struct(code, description)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
