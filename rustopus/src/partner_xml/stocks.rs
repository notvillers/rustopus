use serde::Serialize;
use crate::o8_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetStockChangeAuthResponse
}


#[derive(Serialize)]
pub struct GetStockChangeAuthResponse {
    pub result: GetStockChangeAuthResult
}


#[derive(Serialize)]
pub struct GetStockChangeAuthResult {
    pub answer: Answer
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Option<Error>
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
            description: hiba.leiras
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub stock: Option<f64>
}
