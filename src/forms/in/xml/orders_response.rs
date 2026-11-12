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
        // The five charge blocks Octopus adds on its own — a delivery order
        // gets a `fuvarkoltseg` line, a deposit-return (DRS) product a
        // `visszavaltasi_dij` line — each as `<tetel>` elements, and an empty
        // `<x />` when there is none.
        #[serde(default)]
        pub extratetelek: ValaszDijak,
        #[serde(default)]
        pub fuvarkoltseg: ValaszDijak,
        #[serde(default)]
        pub utanvet: ValaszDijak,
        #[serde(default)]
        pub extraszolg: ValaszDijak,
        #[serde(default)]
        pub visszavaltasi_dij: ValaszDijak,
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
    //
    // `mennyiseg` repeats when a line ships in parts: 11 from stock
    // (`tipus="1"`) and 9 arriving later (`tipus="3"`, `kenocs="Érkezik"`),
    // each with its own date. A single field fails the whole answer with
    // "duplicate field `mennyiseg`" — for an order that was recorded fine.
    #[derive(Clone)]
    pub struct ValaszTetel {
        pub tetelszam: String,
        pub rogzitett_tetelszam: String,
        pub cikkszam: String,
        #[serde(default)]
        pub mennyiseg: Vec<ValaszMennyiseg>,
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

    #[derive(Clone, Default)]
    pub struct ValaszDijak {
        #[serde(rename = "tetel", default)]
        pub tetel: Vec<ValaszDij>,
    }

    /// A charge line Octopus added to the order. Shaped like an order line but
    /// not the same: it carries the article's name and internal id, has no
    /// requested line number, and a deposit line names the line it belongs to.
    #[derive(Clone)]
    pub struct ValaszDij {
        #[serde(default)]
        pub rogzitett_tetelszam: Option<String>,
        #[serde(default)]
        pub cikkid: Option<String>,
        #[serde(default)]
        pub cikkszam: Option<String>,
        #[serde(default)]
        pub cikknev: Option<String>,
        /// A list for the same reason as on `ValaszTetel`.
        #[serde(default)]
        pub mennyiseg: Vec<ValaszMennyiseg>,
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
        /// On a deposit line: the recorded line number of the product the
        /// deposit is charged for.
        #[serde(default)]
        pub vcapatszam: Option<String>,
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
