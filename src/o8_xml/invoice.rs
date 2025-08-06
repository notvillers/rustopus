// Structs for GetSzamlakAuth's XML
use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

use crate::o8_xml;
use crate::partner_xml;

/// Get the string for the request
pub fn get_request_string(xmlns: &str, pid: &i64, tipus: &i64, datumtol: &DateTime<Utc>, datumig: &DateTime<Utc>, osszes_fizetetlen: &i64, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
                <soap:Body>
                    <GetSzamlakAuth xmlns="{}">
                        <pid>{}</pid>
                        <tipus>{}</tipus>
                        <datumtol>{}</datumtol>
                        <datumig>{}</datumig>
                        <osszes_fizetetlen>{}</osszes_fizetetlen>
                        <authcode>{}</authcode>
                    </GetSzamlakAuth>
                </soap:Body>
                </soap:Envelope>
        "#,
        xmlns,
        pid,
        tipus,
        datumtol.format("%Y-%m-%dT%H:%M:%S").to_string(),
        datumig.format("%Y-%m-%dT%H:%M:%S").to_string(),
        osszes_fizetetlen,
        authcode
    )
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_szamlak_auth_response: GetSzamlakAuthResponse
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetSzamlakAuthResponse {
    pub get_szamlak_auth_result: GetSzamlakAuthResult
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetSzamlakAuthResult {
    pub valasz: Valasz
}


#[derive(Debug, Deserialize)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,

    // számlák

    #[serde(rename = "hiba")]
    pub hiba: Option<o8_xml::defaults::Hiba>
}