use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")] // Handle PascalCase names
pub struct Envelope {
    pub Body: Body,
}


impl Envelope {
    pub fn has_error(&self) -> bool {
        self.Body
            .GetCikkekAuthResponse
            .GetCikkekAuthResult
            .valasz
            .hiba
            .is_some()
    }
}


#[derive(Debug, Deserialize)]
pub struct Body {
    pub GetCikkekAuthResponse: GetCikkekAuthResponse,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekAuthResponse {
    pub GetCikkekAuthResult: GetCikkekAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekAuthResult {
    pub valasz: valasz,
}


#[derive(Debug, Deserialize)]
pub struct valasz {
    #[serde(rename = "@verzio")]
    pub verzio: String,

    #[serde(rename = "cikk")]
    #[serde(default)]
    pub cikk: Vec<Cikk>,

    #[serde(rename = "hiba")]
    pub hiba: Option<Hiba>
}


#[derive(Debug, Deserialize)]
pub struct Hiba {
    pub kod: u64,
    pub leiras: String,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Cikk {
    #[serde(rename = "@cikkid")]
    pub cikkid: u64,
    pub cikkszam: String,
    pub cikknev: String,
    pub me: String,
    pub alapme: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub alapmenny: Option<f64>,
    pub kshszam: u64,
    pub gyarto: String,
    pub cikkcsoportkod: String,
    pub cikkcsoportnev: String,
    pub tipus: u64,
    pub beszerzesiallapot: u64,
    #[serde(deserialize_with = "parse_date", default)]
    pub webigendatum: Option<NaiveDate>,
    pub webmegjel: u64,
    pub leiras: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub tomeg: Option<f64>,
    pub meret: Option<Meret>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub afakulcs: Option<f64>,
    pub focsoportkod: String,
    pub focsoportnev: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub ertmenny: Option<f64>,
    pub szarmorszag: String,
    pub cikktipus: u64,
    pub visszavalt_dijas: u64,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Meret {
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub xmeret: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub ymeret: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub zmeret: Option<f64>,
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
