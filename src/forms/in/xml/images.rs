/// Structs for GetCikkKepekAuth's XML
use chrono::{DateTime, Utc};
use macro_rules_attribute::apply;

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
                    <GetCikkKepekAuth xmlns="{}">
                        <web_update>{}</web_update>
                        <authcode>{}</authcode>
                    </GetCikkKepekAuth>
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
        pub get_cikk_kepek_auth_response: GetCikkKepekAuthResponse
    }
    
    pub struct GetCikkKepekAuthResponse {
        pub get_cikk_kepek_auth_result: GetCikkKepekAuthResult,
    }
}


O8ModelLowercase! {
    pub struct GetCikkKepekAuthResult {
        pub valasz: Valasz,
    }
    
    pub struct Cikk {
        #[serde(rename = "@cikkid")]
        pub cikkid: u64,
        #[serde(rename = "@cikkszam")]
        pub cikkszam: String,
        pub kepek: Kepek
    }
    
    pub struct Kepek {
        #[serde(default)]
        pub kep: Vec<Kep>
    }

    pub struct Kep {
        #[serde(rename = "@galeria")]
        pub galeria: String,
        #[serde(rename = "$value")]
        pub url: String
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
