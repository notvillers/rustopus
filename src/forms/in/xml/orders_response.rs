use crate::{
    macros::r#in::{O8ModelDeriveOnly, O8ModelLowercase, O8ModelPascalcase},
    forms::r#in::xml::defaults as o8_defaults
};
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
        #[serde(rename = "@verzio", default)]
        pub verzio: Option<String>,
        // Optional: a refused order comes back as a well-formed envelope
        // carrying `<hiba>` and no `<fej>`, and a required header would fail
        // the parse and hide the refusal's reason behind a generic error.
        #[serde(default)]
        pub fej: Option<ValaszFej>,
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
        #[serde(default)]
        pub hiba: Option<o8_defaults::Hiba>,
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
    
    // A line Octopus did not record (unknown article, …) comes back with
    // `rogzitett_tetelszam = 0`, `mennyiseg tipus="4"` and no date or prices,
    // so everything past the identifying fields is optional.
    #[derive(Clone)]
    pub struct ValaszTetel {
        pub tetelszam: String,
        pub rogzitett_tetelszam: String,
        pub cikkszam: String,
        pub mennyiseg: ValaszMennyiseg,
        #[serde(default)]
        pub egysegar: Option<String>,
        #[serde(default)]
        pub bregysegar: Option<String>,
        #[serde(default)]
        pub ertek: Option<String>,
        #[serde(default)]
        pub brertek: Option<String>,
        #[serde(default)]
        pub dnem: Option<String>,
    }

    #[derive(Clone)]
    pub struct ValaszMennyiseg {
        #[serde(rename = "$value")]
        pub value: String,
        #[serde(rename = "@tipus", default)]
        pub tipus: Option<String>,
        #[serde(rename = "@kenocs", default)]
        pub kenocs: Option<String>,
        #[serde(rename = "@datum", default)]
        pub datum: Option<String>,
    }
}


#[apply(O8ModelLowercase)]
pub struct RendelesFeladasAuthResult {
    pub valasz: Valasz,
}
