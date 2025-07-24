/// Structs for GetCikkekKeszletValtozasAuth's XML
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

use crate::o8_xml;
use crate::partner_xml;

/// Get the string for the request
pub fn get_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
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


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}

impl Envelope {
    pub fn to_en(self) -> partner_xml::stocks::Envelope {
        self.into()
    }
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_cikkek_keszlet_valtozas_auth_response: GetCikkekKeszletValtozasAuthResponse,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetCikkekKeszletValtozasAuthResponse {
    pub get_cikkek_keszlet_valtozas_auth_result: GetCikkekKeszletValtozasAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekKeszletValtozasAuthResult {
    pub valasz: Valasz,
}


#[derive(Debug, Deserialize)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,
    pub cikkek: Cikkek,
    #[serde(rename = "hiba")]
    pub hiba: Option<o8_xml::defaults::Hiba>
}


#[derive(Debug, Deserialize)]
pub struct Cikkek {
    pub cikk: Vec<Cikk>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Cikk {
    pub cikkid: u64,
    pub cikkszam: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub szabad: Option<f64>
}


// Octopus sends floats (actually strings) with ',' separator, we need to convert it to '.' separator
fn parse_comma_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => {
            f64::from_str(&value.replace(",", "."))
                .map(Some)
                .map_err(|_| serde::de::Error::custom("invalid float format"))
        }
        None => Ok(None)
    }
}
