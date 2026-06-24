use serde::{Deserialize, Deserializer};
use std::str::FromStr;
use macro_rules_attribute::apply;

use crate::{
    macros::r#in::{O8ModelDeriveOnly, O8ModelLowercase, O8ModelPascalcase},
    forms::r#in::xml::defaults as o8_defaults
};

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


O8ModelPascalcase! {
    pub struct Envelope {
        pub body: Body,
    }
    
    pub struct Body {
        pub get_arlista_auth_response: GetArlistaAuthResponse 
    }
    
    pub struct GetArlistaAuthResponse {
        pub get_arlista_auth_result: GetArlistaAuthResult
    }
}


O8ModelDeriveOnly! {
    pub struct Valasz {
        #[serde(rename = "@verzio")]
        pub verzio: String,
        pub arak: Arak,
        #[serde(rename = "hiba")]
        pub hiba: Option<o8_defaults::Hiba>
    }
    
    pub struct Arak {
        pub ar: Vec<Ar>
    }
    
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
}


#[apply(O8ModelLowercase)]
pub struct GetArlistaAuthResult {
    pub valasz: Valasz
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
