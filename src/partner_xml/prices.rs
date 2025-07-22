/// Prices english struct(s) for XML(s) got from the Octopus call

use serde::Serialize;
use quick_xml;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::prices::Envelope> for Envelope {
    fn from(envelope: o8_xml::prices::Envelope) -> Self {
        Envelope {
            body: envelope.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetPriceAuthResponse
}

impl From<o8_xml::prices::Body> for Body {
    fn from(body: o8_xml::prices::Body) -> Self {
        Body {
            response: body.get_arlista_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResponse {
    pub result: GetPriceAuthResult
}

impl From<o8_xml::prices::GetArlistaAuthResponse> for GetPriceAuthResponse {
    fn from(response: o8_xml::prices::GetArlistaAuthResponse) -> Self {
        GetPriceAuthResponse {
            result: response.get_arlista_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResult {
    pub answer: Answer
}

impl From<o8_xml::prices::GetArlistaAuthResult> for GetPriceAuthResult {
    fn from(result: o8_xml::prices::GetArlistaAuthResult) -> Self {
        GetPriceAuthResult {
            answer: result.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub prices: Prices,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::prices::Valasz> for Answer {
    fn from(valasz: o8_xml::prices::Valasz) -> Self {
        Answer {
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

impl From<o8_xml::prices::Arak> for Prices {
    fn from(arak: o8_xml::prices::Arak) -> Self {
        Prices {
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

impl From<o8_xml::prices::Ar> for Price {
    fn from(ar: o8_xml::prices::Ar) -> Self {
        Price {
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
                        version: "1.0".to_string(),
                        prices: Prices {
                            price: vec![]
                        },
                        error: Some(partner_xml::defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&error_struct(code, description)) {
        Ok(xml) => xml,
        _ => "<Envelope></Envelope>".to_string()
    }
}
