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
    pub get_cikkek_auth_response: GetCikkekAuthResponse
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetCikkekAuthResponse {
    pub get_cikkek_auth_result: GetCikkekAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkekAuthResult {
    pub valasz: Valasz,
}


#[derive(Debug, Deserialize)]
pub struct Valasz {
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
    pub gyarto: String,
    pub cikkcsoportkod: String,
    pub cikkcsoportnev: String,
    pub leiras: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub tomeg: Option<f64>,
    pub meret: Option<Meret>,
    pub focsoportkod: String,
    pub focsoportnev: String,
    #[serde(deserialize_with = "parse_comma_f64", default)]
    pub ertmenny: Option<f64>,
    pub szarmorszag: String
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
