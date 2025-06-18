use serde::Serialize;

use crate::o8_xml;
use crate::service::errors;

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
    pub error: Option<Error>
}

impl From<o8_xml::stocks::Valasz> for Answer {
    fn from(v: o8_xml::stocks::Valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikkek.into(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl From<o8_xml::stocks::Hiba> for Error {
    fn from(hiba: o8_xml::stocks::Hiba) -> Self {
        Error {
            code: hiba.kod,
            description: errors::translate_error(&hiba.leiras)
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
                        .map(|p| p.into())
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
                        version: "1.0".to_string(),
                        products: Products {
                            product: Vec::new()
                        },
                        error: Some(
                            Error {
                                code: code,
                                description: description.to_string()
                            }
                        )
                    }
                }
            }
        }
    }
}
