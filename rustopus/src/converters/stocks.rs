use crate::o8_xml;
use crate::partner_xml;
use quick_xml;
use chrono::{DateTime, Utc};
use crate::service::soap;
use crate::service::log::logger;

pub async fn get_stocks(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let hu_stocks_xml = get_stocks_xml(url, xmlns, authcode, web_update).await;
    let hu_envelope = get_stocks_envelope(&hu_stocks_xml);
    match hu_envelope {
        Ok(hu_envelope) => {
            convert_stocks_envelope_to_xml(hu_envelope)
        }
        Err(e) => {
            logger(format!("Get stocks error: {}", e));
            "<Envelope></Envelope>".to_string()
        }
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
    let eng_xml = quick_xml::se::to_string(&en_envelope);

    match eng_xml {
        Ok(eng_xml) => {
            eng_xml
        }
        Err(e) => {
            logger(format!("Convert stocks error: {}", e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
