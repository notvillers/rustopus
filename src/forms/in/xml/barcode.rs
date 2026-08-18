// Structs for GetVonalkodokAuth's XML
use chrono::{DateTime, Utc};

use quick_xml::escape::escape;

use crate::{
    macros::r#in::{O8ModelLowercase, O8ModelPascalcase},
    forms::r#in::xml::defaults as o8_defaults
};

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
        escape(xmlns),
        web_update.format("%Y-%m-%dT%H:%M:%S"),
        escape(authcode)
    )
}


O8ModelPascalcase! {
    pub struct Envelope {
        pub body: Body
    }
    
    pub struct Body {
        pub get_vonalkodok_auth_response: GetVonalkodokAuthResponse
    }
    
    pub struct GetVonalkodokAuthResponse {
        pub get_vonalkodok_auth_result: GetVonalkodokAuthResult
    }
    
    pub struct GetVonalkodokAuthResult {
        #[serde(rename = "valasz")]
        pub valasz: Valasz
    }
}


O8ModelLowercase! {
    pub struct Valasz {
        #[serde(rename = "@verzio")]
        pub verzio: String,
        /// Absent when Octopus answers with `<hiba>` instead of data. See the
        /// note on `Valasz::arak` in `prices.rs` — without the default, the
        /// error response fails to parse and its reason never reaches the caller.
        #[serde(rename = "vonalkodok", default)]
        pub vonalkodok: Vonalkodok,
        pub hiba: Option<o8_defaults::Hiba>
    }

    #[derive(Default)]
    pub struct Vonalkodok {
        #[serde(rename = "vonalkod")]
        pub vonalkod: Vec<Vonalkod>
    }

    pub struct Vonalkod {
        pub cikkid: u64,
        pub cikkszam: String,
        pub vonalkod: String,
        pub me: String,
        pub elsean: u64
    }
}
