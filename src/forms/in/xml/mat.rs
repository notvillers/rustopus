/// Structs fro GetMatmodellFogalomAuth's XML
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize, Deserializer};
use std::num::NonZeroU8;
use std::str::FromStr;
use macro_rules_attribute::{attribute_alias, apply};

use crate::forms::r#in::xml::defaults as o8_defaults;


/// Get the string for the request
pub fn get_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
                <soap:Body>
                    <GetMatmodellAuth xmlns="{}">
                    <web_update>{}</web_update>
                    <authcode>{}</authcode>
                    </GetMatmodellAuth>
                </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


attribute_alias! {
    #[apply(O8ModelDeriveOnly)] =
        #[derive(Debug, Deserialize, Serialize)];
    #[apply(O8ModelPascalcase)] =
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "PascalCase")];
    #[apply(O8ModelLowercase)] =
        #[derive(Debug, Deserialize, Serialize)]
        #[serde(rename_all = "lowercase")];
}


#[apply(O8ModelPascalcase)]
pub struct Envelope {
    pub body: Body
}


#[apply(O8ModelPascalcase)]
pub struct Body {
    pub get_matmodell_auth_response: GetMatmodellAuthResponse
}


#[apply(O8ModelPascalcase)]
pub struct GetMatmodellAuthResponse {
    pub get_matmodell_auth_result: GetMatmodellAuthResult
}


#[apply(O8ModelLowercase)]
pub struct GetMatmodellAuthResult {
    pub valasz: Valasz
}


#[apply(O8ModelDeriveOnly)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,

    #[serde(rename = "tulajdonsagok", default)]
    pub tulajdonsagok: Tulajdonsagok,

    #[serde(rename = "hiba")]
    pub hiba: Option<o8_defaults::Hiba>
}


#[apply(O8ModelLowercase)]
#[derive(Default)]
pub struct Tulajdonsagok {
    #[serde(rename = "tulajdonsag", default)]
    pub tulajdonsag: Vec<Tulajdonsag>
}


#[apply(O8ModelLowercase)]
pub struct Tulajdonsag {
    pub azonosito: u64,
    pub tulajdonsagkod: Option<String>,
    pub tulajdonsagnev: Option<String>,

    #[serde(deserialize_with = "empty_as_none", default)]
    pub cikkid: Option<u64>,
    pub cikkszam: Option<String>,
    pub szovegertek: Option<String>,

    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub szamertek: Option<f64>,

    #[serde(deserialize_with = "empty_as_none", default)]
    pub sorrend: Option<i64>,

    #[serde(deserialize_with = "empty_as_none", default)]
    pub delstatus: Option<NonZeroU8>,

    #[serde(deserialize_with = "empty_as_none", default)]
    pub szures: Option<NonZeroU8>,

    #[serde(deserialize_with = "empty_as_none", default)]
    pub adattipus: Option<NonZeroU8>,
    
    #[serde(deserialize_with = "empty_as_none", default)]
    pub ertekkeszlet_id: Option<i64>
}


/// Parses Octopus numeric text (comma decimal separator) into `Option<f64>`,
/// treating an empty element (`<szamertek />`) as `None`.
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


/// Parses any `FromStr` value, treating an empty element (e.g. `<sorrend />`) as `None`
/// instead of failing. Octopus emits empty self-closing tags for absent numeric fields.
fn empty_as_none<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: std::fmt::Display,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(value) if value.is_empty() => Ok(None),
        Some(value) => T::from_str(&value).map(Some).map_err(serde::de::Error::custom),
        None => Ok(None)
    }
}
