// Sturcts for GetVonalkodokAuth's XML
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::o8_xml;
use crate::partner_xml;

pub fn get_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetVonalkodokAuth xmlns="{}">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetVonalkodokAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body
}

impl Envelope {
    pub fn to_en(self) -> partner_xml::barcode::Envelope {
        self.into()
    }
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_vonalkodok_auth_response: GetVonalkodokAuthResponse
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetVonalkodokAuthResponse {
    pub get_vonalkodok_auth_result: GetVonalkodokAuthResult
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetVonalkodokAuthResult {
    #[serde(rename = "valasz")]
    pub valasz: Valasz
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,
    #[serde(rename = "vonalkodok")]
    pub vonalkodok: Vonalkodok,
    pub hiba: Option<o8_xml::defaults::Hiba>
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Vonalkodok {
    #[serde(rename = "vonalkod")]
    pub vonalkod: Vec<Vonalkod>
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub struct Vonalkod {
    pub cikkid: u64,
    pub cikkszam: String,
    pub vonalkod: String,
    pub me: String,
    pub elsean: u64
}
