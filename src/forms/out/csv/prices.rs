use serde::Serialize;
use crate::forms::r#in::xml::prices as o8_prices;

#[derive(Serialize)]
pub struct Price {
    pub id: u64,
    pub no: String,
    pub list_price: Option<f64>,
    pub price: Option<f64>,
    pub sale_price: Option<f64>,
    pub currencry: String
}

impl From<o8_prices::Ar> for Price {
    fn from(a: o8_prices::Ar) -> Self {
        Self {
            id: a.cikkid,
            no: a.cikkszam,
            list_price: a.listaar,
            price: a.ar,
            sale_price: a.akcios_ar,
            currencry: a.devizanem
        }
    }
}


#[derive(Serialize)]
pub struct Prices {
    pub prices: Vec<Price>
}

impl From<o8_prices::Envelope> for Prices {
    fn from(e: o8_prices::Envelope) -> Self {
        let prices = e.body.get_arlista_auth_response.get_arlista_auth_result.valasz.arak.ar;
        Self {
            prices: prices
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}