use serde::Serialize;

use crate::o8_xml::products::Hiba;


#[derive(Serialize)]
pub struct Envelope {
    pub body: Body,
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetProductsAuthResponse
}


#[derive(Serialize)]
pub struct GetProductsAuthResponse {
    pub result: GetProductsAuthResult
}



#[derive(Serialize)]
pub struct GetProductsAuthResult {
    pub answer: Answer
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Vec<Product>,
    pub error: Option<Error>
}


#[derive(Serialize)]
pub struct Error {
    pub code: u64,
    pub description: String
}


impl From<Hiba> for Error {
    fn from(hiba: Hiba) -> Self {
        Error {
            code: hiba.kod,
            description: hiba.leiras
        }
    }
}


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub name: String,
    pub unit: String,
    pub base_unit: String,
    pub base_unit_qty: Option<f64>,
    pub brand: String,
    pub category_code: String,
    pub category_name: String,
    pub description: String,
    pub weight: Option<f64>,
    pub size: Size,
    pub main_category_code: String,
    pub main_category_name: String,
    pub sell_unit: Option<f64>,
    pub origin_country: String
}


#[derive(Serialize)]
pub struct Size {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>
}