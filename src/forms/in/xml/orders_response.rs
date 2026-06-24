use crate::macros::r#in::{O8ModelDeriveOnly, O8ModelLowercase, O8ModelPascalcase};
use macro_rules_attribute::apply;

O8ModelPascalcase! {
    pub struct Envelope {
        pub body: Body,
    }
    
    pub struct Body {
        pub rendeles_feladas_auth_response: RendelesFeladasAuthResponse,
    }
    
    pub struct RendelesFeladasAuthResponse {
        pub rendeles_feladas_auth_result: RendelesFeladasAuthResult,
    }
}


O8ModelDeriveOnly! {
    #[derive(Default, Clone)]
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
    #[derive(Default, Clone)]
    pub struct ValaszFej {
        pub azonosito: String,
        pub webazon: String,
        pub bizonylatszam: String,
        pub szalldatum: String,
    }
    
    #[derive(Clone, Default)]
    pub struct ValaszTetelek {
        #[serde(rename = "tetel", default)]
        pub tetel: Vec<ValaszTetel>,
    }
    
    #[derive(Clone)]
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
    
    #[derive(Clone)]
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
}


#[apply(O8ModelLowercase)]
pub struct RendelesFeladasAuthResult {
    pub valasz: Valasz,
}
