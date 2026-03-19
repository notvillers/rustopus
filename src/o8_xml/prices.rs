use serde::Serialize;
/// Structs for GetArlistaAuth's XML
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

use crate::o8_xml::defaults as o8_defaults;
use crate::partner_xml::prices as p_prices;

/// Get the string for the request
pub fn get_request_string(xmlns: &str, authcode: &str, pid: &i64) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
                <soap:Body>
                    <GetArlistaAuth xmlns="{}">
                        <pid>{}</pid>
                        <partnerkod>{}</partnerkod>
                        <authcode>{}</authcode>
                    </GetArlistaAuth>
                </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        pid,
        "",
        authcode
    )
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}

impl Envelope {
    pub fn to_en(self) -> p_prices::Envelope {
        self.into()
    }
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_arlista_auth_response: GetArlistaAuthResponse 
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetArlistaAuthResponse {
    pub get_arlista_auth_result: GetArlistaAuthResult
}


#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub struct GetArlistaAuthResult {
    pub valasz: Valasz
}


#[derive(Debug, Deserialize, Serialize)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,
    pub arak: Arak,
    #[serde(rename = "hiba")]
    pub hiba: Option<o8_defaults::Hiba>
}


#[derive(Debug, Deserialize, Serialize)]
pub struct Arak {
    pub ar: Vec<Ar>
}


#[derive(Debug, Deserialize, Serialize)]
pub struct Ar {
    pub cikkid: u64,
    pub cikkszam: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub listaar: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub ar: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub akcios_ar: Option<f64>,
    pub devizanem: String
}


// Octopus sends floats with ',', we need to convert it to '.'
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