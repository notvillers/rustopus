use chrono::format::StrftimeItems;
use quick_xml::se::to_string;
use serde::Serialize;

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
