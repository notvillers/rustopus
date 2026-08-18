/// Structs for GetCikkekAuth's XML
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Deserializer};
use macro_rules_attribute::apply;
use std::{
    num::NonZeroU8,
    str::FromStr
};

use crate::{
    macros::r#in::{O8ModelDeriveOnly, O8ModelLowercase, O8ModelPascalcase},
    forms::r#in::xml::defaults as o8_defaults
};

/// Get the string for the request
pub fn get_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
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
        web_update.format("%Y-%m-%dT%H:%M:%S"),
        authcode
    )
}


O8ModelPascalcase! {
    pub struct Envelope {
        pub body: Body,
    }
    
    pub struct Body {
        pub get_cikkek_auth_response: GetCikkekAuthResponse
    }
    
    pub struct GetCikkekAuthResponse {
        pub get_cikkek_auth_result: GetCikkekAuthResult,
    }
}


O8ModelLowercase! {
    pub struct GetCikkekAuthResult {
        pub valasz: Valasz,
    }

    pub struct Cikk {
        #[serde(rename = "@cikkid")]
        pub cikkid: u64,
        pub cikkszam: String,
        pub cikknev: String,
        pub me: String,
        pub alapme: String,
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub alapmenny: Option<f64>,
        pub gyarto: String,
        pub cikkcsoportkod: String,
        pub cikkcsoportnev: String,
        pub tipus: NonZeroU8,
        pub beszerzesiallapot: NonZeroU8,
        pub webmegjel: NonZeroU8,
        #[serde(with = "hungarian_date_format_opt", default)]
        pub webigendatum: Option<NaiveDate>,
        pub leiras: String,
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub tomeg: Option<f64>,
        pub meret: Option<Meret>,
        pub gycikkszam: String,
        pub focsoportkod: String,
        pub focsoportnev: String,
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub ertmenny: Option<f64>,
        pub szarmorszag: String
    }

    #[derive(Clone)]
    pub struct Meret {
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub xmeret: Option<f64>,
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub ymeret: Option<f64>,
        #[serde(deserialize_with = "parse_comma_f64", default)]
        pub zmeret: Option<f64>,
    }
}


#[apply(O8ModelDeriveOnly)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,

    #[serde(rename = "cikk")]
    #[serde(default)]
    pub cikk: Vec<Cikk>,

    #[serde(rename = "hiba")]
    pub hiba: Option<o8_defaults::Hiba>
}


// Format Octopus date (`2012.11.29.`) to NaiveDate
mod hungarian_date_format_opt {
    use super::*;
    const FORMAT: &str = "%Y.%m.%d.";

    pub fn serialize<S>(date: &Option<NaiveDate>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match date {
            Some(d) => serializer.serialize_str(&d.format(FORMAT).to_string()),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Octopus sends the date with a trailing '.', but not always
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(text) => Ok(NaiveDate::parse_from_str(text.trim().trim_end_matches('.'), "%Y.%m.%d").ok()),
            None => Ok(None),
        }
    }
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
