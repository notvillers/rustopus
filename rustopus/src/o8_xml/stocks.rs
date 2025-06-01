use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}


impl Envelope {
    pub fn has_error(&self) -> bool {
        self.body
            .get_cikkek_keszlet_valtozas_auth_response
            .get_cikkek_keszlet_valtozas_auth_result
            .valasz
            .hiba
            .is_some()
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
    pub hiba: Option<Hiba>
}


#[derive(Debug, Deserialize)]
pub struct Hiba {
    pub kod: u64,
    pub leiras: String,
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


// Parse NaiveDate from String and return Option<NaiveDate>
fn parse_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    let date_str: Option<String> = Option::deserialize(deserializer)?;
    match date_str {
        Some(date) if date.is_empty() => Ok(None),
        Some(date) => {
            NaiveDate::parse_from_str(&date.replace(" ", ""), "%Y. %m. %d.")
                .map(Some)
                .map_err(|_| serde::de::Error::custom("invalid date format"))
        }
        None => Ok(None)
    }
}
