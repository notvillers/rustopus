use serde::{Deserialize, Deserializer};
use std::str::FromStr;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
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
