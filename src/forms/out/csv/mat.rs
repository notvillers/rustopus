// Mat CSV
use std::num::NonZeroU8;
use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::r#in::xml::mat as o8_mat
};

OutModelDeriveSerializeOnly! {
    pub struct Concepts {
        pub concepts: Vec<Concept>
    }

    pub struct Concept {
        pub id: u64,
        pub code: Option<String>,
        pub name: Option<String>,
        pub product_id: Option<u64>,
        pub product_no: Option<String>,
        pub string_value: Option<String>,
        pub num_value: Option<f64>,
        pub order: Option<i64>,
        #[serde(serialize_with = "crate::tools::csv::bool_lang")]
        pub delstatus: bool,
        pub filter: Option<NonZeroU8>,
        pub data_type: Option<NonZeroU8>,
        pub value_set: Option<i64>
    }
}


impl From<o8_mat::Envelope> for Concepts {
    fn from(e: o8_mat::Envelope) -> Self {
        let concepts = e.body.get_matmodell_auth_response.get_matmodell_auth_result.valasz.tulajdonsagok.tulajdonsag;
        Self {
            concepts: concepts
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}


impl From<o8_mat::Tulajdonsag> for Concept {
    fn from(f: o8_mat::Tulajdonsag) -> Self {
        Self {
            id: f.azonosito,
            code: f.tulajdonsagkod,
            name: f.tulajdonsagnev,
            product_id: f.cikkid,
            product_no: f.cikkszam,
            string_value: f.szovegertek,
            num_value: f.szamertek,
            order: f.sorrend,
            delstatus: f.delstatus != NonZeroU8::new(1),
            filter: f.szures,
            data_type: f.adattipus,
            value_set: f.ertekkeszlet_id
        }
    }
}


/// Hungarian CSV header row for `Concept`, in field order. Used when `language=hu`.
pub const HU_HEADERS: &[&str] = &[
    "Azonosító",
    "Tulajdonság kód",
    "Tulajdonság név",
    "Cikk azonosító",
    "Cikkszám",
    "Szöveg érték",
    "Szám érték",
    "Sorrend",
    "Törlési státusz",
    "Szűrés",
    "Adattípus",
    "Értékkészlet azonosító"
];
