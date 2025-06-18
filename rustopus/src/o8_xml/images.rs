use serde::{Deserialize};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub get_cikk_kepek_auth_response: GetCikkKepekAuthResponse
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct GetCikkKepekAuthResponse {
    pub get_cikk_kepek_auth_result: GetCikkKepekAuthResult,
}


#[derive(Debug, Deserialize)]
pub struct GetCikkKepekAuthResult {
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
    #[serde(rename = "@cikkszam")]
    pub cikkszam: String,
    pub kepek: Kepek
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Kepek {
    #[serde(default)]
    pub kep: Vec<Kep>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Kep {
    #[serde(rename = "@galeria")]
    pub galeria: String,
    #[serde(rename = "$value")]
    pub url: String
}
