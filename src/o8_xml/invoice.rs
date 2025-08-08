// Structs for GetSzamlakAuth's XML
use chrono::{NaiveDate, DateTime, Utc};
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

use crate::o8_xml;
use crate::partner_xml;
use crate::service;

pub fn get_request_string_opt(xmlns: &str, pid: &Option<i64>, tipus: &Option<i64>, datumtol: &Option<DateTime<Utc>>, datumig: &Option<DateTime<Utc>>, osszes_fizetetlen: &Option<i64>, authcode: &str) -> String {
    get_request_string(
        xmlns,
        &pid.unwrap_or(0),
        &tipus.unwrap_or(1),
        &datumtol.unwrap_or(service::get_data::get_first_date()),
        &datumig.unwrap_or(service::get_data::get_first_date()),
        &osszes_fizetetlen.unwrap_or(0),
        authcode
    )
}

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

impl Envelope {
    pub fn to_en(self) -> partner_xml::invoice::Envelope {
        self.into()
    }
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
#[serde(rename_all = "lowercase")]
pub struct GetSzamlakAuthResult {
    pub valasz: Valasz
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,
    pub szamlak: Szamlak,
    #[serde(rename = "hiba")]
    pub hiba: Option<o8_xml::defaults::Hiba>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Szamlak {
    pub szamla: Vec<Szamla>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Szamla {
    pub fej: Fej,
    pub tetelek: Tetelek
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Fej {
    pub kiszamlakod: i64,
    pub bizonylatszam: Option<String>,
    #[serde(with = "hungarian_date_format_opt")]
    pub bizdatum: Option<NaiveDate>,
    #[serde(with = "hungarian_date_format_opt")]
    pub teljdatum: Option<NaiveDate>,
    #[serde(with = "hungarian_date_format_opt")]
    pub fizhat: Option<NaiveDate>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub devnetto: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub devbrutto: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub devtartozas: Option<f64>,
    pub stornobizszam: Option<String>,
    pub dnem: String,
    pub pid: i64,
    pub partnernev: String,
    pub bizstatus: i64,
    pub idegenmegrszam: Option<String>,
    pub szallcimnev: Option<String>,
    pub szallorszag: Option<String>,
    pub szallirsz: Option<String>,
    pub szallvaros: Option<String>,
    pub szallutca: Option<String>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Tetelek {
    pub tetel: Vec<Tetel>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Tetel {
    pub tetelszam: u64,
    pub cikkid: u64,
    pub cikkszam: String,
    pub cikknev: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub menny: Option<f64>,
    pub me: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub egysegar: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub bregysegar: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub ertek: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub brertek: Option<f64>,
    pub rbizonylatszam: Option<String>,
    pub ridegenmegrszam: Option<String>
}


// Format Octopus date to NaiveDate
mod hungarian_date_format_opt {
    use super::*;
    const FORMAT: &str = "%Y.%m.%d";

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<&str> = Option::deserialize(deserializer)?;
        match s {
            Some(text) => Ok(NaiveDate::parse_from_str(text, FORMAT).ok()),
            None => Ok(None),
        }
    }
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
