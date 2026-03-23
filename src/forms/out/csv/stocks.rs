use serde::Serialize;
use crate::forms::r#in::xml::stocks as o8_stocks;

#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub stock: Option<f64>
}

impl From<o8_stocks::Cikk> for Product {
    fn from(c: o8_stocks::Cikk) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam,
            stock: c.szabad
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub products: Vec<Product>
}

impl From<o8_stocks::Envelope> for Products {
    fn from(e: o8_stocks::Envelope) -> Self {
        let products = e.body.get_cikkek_keszlet_valtozas_auth_response.get_cikkek_keszlet_valtozas_auth_result.valasz.cikkek.cikk;
        Self {
            products: products
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}
