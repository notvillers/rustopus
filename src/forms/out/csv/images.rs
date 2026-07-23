// Images CSV
use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::r#in::xml::images as o8_images
};

OutModelDeriveSerializeOnly! {
    pub struct Product {
        pub id: u64,
        pub no: String,
        pub url: String
    }

    pub struct Products {
        pub products: Vec<Product>
    }
}


impl From<o8_images::Cikk> for Product {
    fn from(c: o8_images::Cikk) -> Self {
        Self {
            id: c.cikkid,
            no: c.cikkszam,
            url: c.kepek.kep
                .first()
                .map(|k| k.url.clone())
                .unwrap_or_default()
        }
    }
}


impl From<o8_images::Envelope> for Products {
    fn from(e: o8_images::Envelope) -> Self {
        let products = e.body.get_cikk_kepek_auth_response.get_cikk_kepek_auth_result.valasz.cikk;
        Self {
            products: products
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}

/// Hungarian CSV header row for `Product`, in field order. Used when `language=hu`.
pub const HU_HEADERS: &[&str] = &[
    "Cikk azonosító",
    "Cikkszám",
    "Kép url"
];