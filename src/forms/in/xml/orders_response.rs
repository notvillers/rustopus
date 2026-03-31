use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Envelope {
    pub body: Body,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Body {
    pub rendeles_feladas_auth_response: RendelesFeladasAuthResponse,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct RendelesFeladasAuthResponse {
    pub rendeles_feladas_auth_result: RendelesFeladasAuthResult,
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct RendelesFeladasAuthResult {
    pub valasz: Valasz,
}


#[derive(Debug, Deserialize)]
pub struct Valasz {
    #[serde(rename = "@verzio")]
    pub verzio: Option<String>,
    pub fej: ValaszFej,
    #[serde(default)]
    pub tetelek: ValaszTetelek,
    #[serde(default)]
    pub extratetelek: Option<String>,
    #[serde(default)]
    pub fuvarkoltseg: Option<String>,
    #[serde(default)]
    pub utanvet: Option<String>,
    #[serde(default)]
    pub extraszolg: Option<String>,
    #[serde(default)]
    pub visszavaltasi_dij: Option<String>,
}

/// Response header
#[derive(Debug, Deserialize)]
pub struct ValaszFej {
    pub azonosito: String,
    pub webazon: String,
    pub bizonylatszam: String,
    pub szalldatum: String,
}


#[derive(Debug, Clone, Default, Deserialize)]
pub struct ValaszTetelek {
    #[serde(rename = "tetel", default)]
    pub tetel: Vec<ValaszTetel>,
}


#[derive(Debug, Clone, Deserialize)]
pub struct ValaszTetel {
    pub tetelszam: String,
    pub rogzitett_tetelszam: String,
    pub cikkszam: String,
    pub mennyiseg: ValaszMennyiseg,
    pub egysegar: String,
    pub bregysegar: String,
    pub ertek: String,
    pub brertek: String,
    pub dnem: String,
}


#[derive(Debug, Clone, Deserialize)]
pub struct ValaszMennyiseg {
    #[serde(rename = "$value")]
    pub value: String,
    #[serde(rename = "@tipus")]
    pub tipus: String,
    #[serde(rename = "@kenocs")]
    pub kenocs: String,
    #[serde(rename = "@datum")]
    pub datum: String,
}
