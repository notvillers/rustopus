/// Structs for GetCikkKepekAuth's XML
use chrono::{DateTime, Utc};
use serde::Deserialize;

use crate::o8_xml;
use crate::partner_xml;

/// Get the string for the request
pub fn get_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
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

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}

impl Envelope {
    pub fn to_en(self) -> partner_xml::images::Envelope {
        self.into()
    }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_cikk_kepek_auth_response: GetCikkKepekAuthResponse
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetCikkKepekAuthResponse {
    pub get_cikk_kepek_auth_result: GetCikkKepekAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkKepekAuthResult {
    pub valasz: Valasz,
}


#[derive(Debug, Deserialize)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,

    #[serde(rename = "cikk")]
    #[serde(default)]
    pub cikk: Vec<Cikk>,

    #[serde(rename = "hiba")]
    pub hiba: Option<o8_xml::defaults::Hiba>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Cikk {
    #[serde(rename = "@cikkid")]
    pub cikkid: u64,
    #[serde(rename = "@cikkszam")]
    pub cikkszam: String,
    pub kepek: Kepek
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Kepek {
    #[serde(default)]
    pub kep: Vec<Kep>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Kep {
    #[serde(rename = "@galeria")]
    pub galeria: String,
    #[serde(rename = "$value")]
    pub url: String
}
