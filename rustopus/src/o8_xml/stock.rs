use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use std::{error, str::FromStr};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")] // Handle PascalCase names
pub struct Envelope {
    pub Body: Body,
}


impl Envelope {
    pub fn has_error(&self) -> bool {
        self.Body
            .GetCikkekKeszletValtozasAuthResponse
            .GetCikkekKeszletValtozasAuthResult
            .valasz
            .hiba
            .is_some()
    }
}


#[derive(Debug, Deserialize)]
pub struct Body {
    pub GetCikkekKeszletValtozasAuthResponse: GetCikkekKeszletValtozasAuthResponse,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekKeszletValtozasAuthResponse {
    pub GetCikkekKeszletValtozasAuthResult: GetCikkekKeszletValtozasAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekKeszletValtozasAuthResult {
    pub valasz: valasz,
}


#[derive(Debug, Deserialize)]
pub struct valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,
    pub cikkek: cikkek,
    #[serde(rename = "hiba")]
    pub hiba: Option<Hiba>
}


#[derive(Debug, Deserialize)]
pub struct Hiba {
    pub kod: u64,
    pub leiras: String,
}


#[derive(Debug, Deserialize)]
pub struct cikkek {
    pub cikk: Vec<cikk>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct cikk {
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
