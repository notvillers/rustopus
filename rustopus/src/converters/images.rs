use chrono::{DateTime, Utc};

use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;
use crate::service::log::logger;

pub async fn get_data(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    match get_images_envelope(&get_images_xml(url, xmlns, authcode, web_update).await) {
        Ok(hu_envelope) => {
            convert_images_envelope_to_xml(hu_envelope)
        }
        Err(e) => {
            let error_code = 102;
            let error_description = "Server error: Get images envelope error";
            logger(format!("{}: Get images envelope error: {}", error_description, e));
            send_error_xml(error_code, error_description)
        }
    }
}


fn get_images_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
                <soap:Body>
                    <GetCikkKepekAuth xmlns="{}">
                        <web_update>{}</web_update>
                        <authcode>{}</authcode>
                    </GetCikkKepekAuth>
                </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    ) 
}


pub async fn get_images_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    soap::get_response(url, get_images_request_string(xmlns, web_update, authcode)).await
}


pub fn get_images_envelope(response_text: &str) -> Result<o8_xml::images::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


fn convert_images_envelope_to_xml(hu_envelope: o8_xml::images::Envelope) -> String {
    let en_envelope: partner_xml::images::Envelope = hu_envelope.into();
    match quick_xml::se::to_string(&en_envelope) {
        Ok(eng_xml) => {
            eng_xml
        }
        Err(e) => {
            let error_code = 101;
            let error_description = "Server error: Convert error";
            logger(format!("{}: {}", error_description, e));
            send_error_xml(error_code, error_description)
        }
    }
}


pub fn send_error_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&partner_xml::images::error_struct(code, description)) {
        Ok(e_xml) => {
            e_xml
        }
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
