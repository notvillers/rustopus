use chrono::NaiveDate;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")] // Handle PascalCase names
pub struct Envelope {
    pub Body: Body,
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
    pub cikk: Vec<Cikk>,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Cikk {
    #[serde(rename = "@cikkid")]
    pub cikkid: u32,
    pub cikkszam: String,
    pub cikknev: String,
    pub me: String,
    pub alapme: String,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub alapmenny: Option<f64>,
    pub kshszam: u32,
    pub gyarto: String,
    pub cikkcsoportkod: String,
    pub cikkcsoportnev: String,
    pub tipus: u8,
    pub beszerzesiallapot: u8,
    #[serde(deserialize_with = "parse_date")]
    pub webigendatum: Option<NaiveDate>,
    pub webmegjel: u8,
    pub leiras: String,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub tomeg: Option<f64>,
    pub meret: Option<Meret>,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub afakulcs: Option<f64>,
    pub focsoportkod: String,
    pub focsoportnev: String,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub ertmenny: Option<f64>,
    pub szarmorszag: String,
    pub cikktipus: u8,
    pub visszavalt_dijas: u8,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Meret {
    #[serde(deserialize_with = "parse_comma_f64")]
    pub xmeret: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub ymeret: Option<f64>,
    #[serde(deserialize_with = "parse_comma_f64")]
    pub zmeret: Option<f64>,
}


// Octopus sends floats with ',', we need to convert it to '.'
fn parse_comma_f64<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: Deserializer<'de>,
{
    let s: Option<String> = Option::deserialize(deserializer)?;
    match s {
        Some(value) if value.is_empty() => Ok(None), // Return None if the value is empty
        Some(value) => {
            // Replace the comma with a dot and try to parse it as f64
            f64::from_str(&value.replace(",", "."))
                .map(Some) // Wrap the parsed value in Some if it's valid
                .map_err(|_| serde::de::Error::custom("invalid float format")) // Return None on error
        }
        None => Ok(None), // Return None if the value is missing
    }
}


// Parse NaiveDate from String and return Option<NaiveDate>
fn parse_date<'de, D>(deserializer: D) -> Result<Option<NaiveDate>, D::Error>
where
    D: Deserializer<'de>,
{
    let date_str: Option<String> = Option::deserialize(deserializer)?;
    match date_str {
        Some(date) if date.is_empty() => Ok(None), // Return None if the date is an empty string
        Some(date) => {
            // Try to parse the date, and return Some(parsed_date) if successful
            NaiveDate::parse_from_str(&date.replace(" ", ""), "%Y. %m. %d.")
                .map(Some) // Wrap the parsed date in Some if it's valid
                .map_err(|_| serde::de::Error::custom("invalid date format")) // Return None on error
        }
        None => Ok(None), // Return None if the date string is missing
    }
}
