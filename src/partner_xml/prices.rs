/// Prices english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::forms::r#in::xml::prices as o8_prices;
use crate::partner_xml::defaults as p_defaults;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_prices::Envelope> for Envelope {
    fn from(envelope: o8_prices::Envelope) -> Self {
        Self {
            body: envelope.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetPriceAuthResponse
}

impl From<o8_prices::Body> for Body {
    fn from(body: o8_prices::Body) -> Self {
        Self {
            response: body.get_arlista_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResponse {
    pub result: GetPriceAuthResult
}

impl From<o8_prices::GetArlistaAuthResponse> for GetPriceAuthResponse {
    fn from(response: o8_prices::GetArlistaAuthResponse) -> Self {
        Self {
            result: response.get_arlista_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResult {
    pub answer: Answer
}

impl From<o8_prices::GetArlistaAuthResult> for GetPriceAuthResult {
    fn from(result: o8_prices::GetArlistaAuthResult) -> Self {
        Self {
            answer: result.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub prices: Prices,
    pub error: Option<p_defaults::Error>
}

impl From<o8_prices::Valasz> for Answer {
    fn from(valasz: o8_prices::Valasz) -> Self {
        Self {
            version: valasz.verzio,
            prices: valasz.arak.into(),
            error: valasz.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Prices {
    pub price: Vec<Price>
}

impl From<o8_prices::Arak> for Prices {
    fn from(arak: o8_prices::Arak) -> Self {
        Self {
            price: arak.ar
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Price {
    pub id: u64,
    pub no: String,
    pub list_price: Option<f64>,
    pub price: Option<f64>,
    pub sale_price: Option<f64>,
    pub currency: String
}

impl From<o8_prices::Ar> for Price {
    fn from(ar: o8_prices::Ar) -> Self {
        Self {
            id: ar.cikkid,
            no: ar.cikkszam,
            list_price: ar.listaar,
            price: ar.ar,
            sale_price: ar.akcios_ar,
            currency: ar.devizanem
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetPriceAuthResponse {
                result: GetPriceAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        prices: Prices {
                            price: vec![]
                        },
                        error: Some(p_defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_struct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
