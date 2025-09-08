/// Stocks english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::stocks::Envelope> for Envelope {
    fn from(e: o8_xml::stocks::Envelope) -> Self {
        Envelope {
            body: e.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetStockChangeAuthResponse
}

impl From<o8_xml::stocks::Body> for Body {
    fn from(b: o8_xml::stocks::Body) -> Self {
        Body {
            response: b.get_cikkek_keszlet_valtozas_auth_response.into()
        }
    }
}



#[derive(Serialize)]
pub struct GetStockChangeAuthResponse {
    pub result: GetStockChangeAuthResult
}

impl From<o8_xml::stocks::GetCikkekKeszletValtozasAuthResponse> for GetStockChangeAuthResponse {
    fn from(r: o8_xml::stocks::GetCikkekKeszletValtozasAuthResponse) -> Self {
        GetStockChangeAuthResponse {
            result: r.get_cikkek_keszlet_valtozas_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetStockChangeAuthResult {
    pub answer: Answer
}

impl From<o8_xml::stocks::GetCikkekKeszletValtozasAuthResult> for GetStockChangeAuthResult {
    fn from(r: o8_xml::stocks::GetCikkekKeszletValtozasAuthResult) -> Self {
        GetStockChangeAuthResult {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::stocks::Valasz> for Answer {
    fn from(v: o8_xml::stocks::Valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikkek.into(),
            error: v.hiba.map(|x| x.into())
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl From<o8_xml::stocks::Cikkek> for Products {
    fn from(c: o8_xml::stocks::Cikkek) -> Self {
        Products {
            product: c.cikk
                        .into_iter()
                        .map(|x| x.into())
                        .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub stock: Option<f64>
}

impl From<o8_xml::stocks::Cikk> for Product {
    fn from(c: o8_xml::stocks::Cikk) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam, 
            stock: c.szabad
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetStockChangeAuthResponse {
                result: GetStockChangeAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        products: Products {
                            product: Vec::new()
                        },
                        error: Some(partner_xml::defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_struct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
