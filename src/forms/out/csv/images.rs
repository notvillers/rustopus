use serde::Serialize;
use crate::forms::r#in::xml::images as o8_images;


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub url: String
}

impl From<o8_images::Cikk> for Product {
    fn from(c: o8_images::Cikk) -> Self {
        Self {
            id: c.cikkid,
            no: c.cikkszam,
            url: c.kepek.kep.first().map(|k| k.url.clone()).unwrap_or_else(|| "".to_string())
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub products: Vec<Product>
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
