use serde::Serialize;

use crate::o8_xml;
use crate::service::errors;


#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::products::Envelope> for Envelope {
    fn from(e: o8_xml::products::Envelope) -> Self {
        Envelope {
            body: e.Body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetProductsAuthResponse
}

impl From<o8_xml::products::Body> for Body {
    fn from(b: o8_xml::products::Body) -> Self {
        Body {
            response: b.GetCikkekAuthResponse.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductsAuthResponse {
    pub result: GetProductsAuthResult
}

impl From<o8_xml::products::GetCikkekAuthResponse> for GetProductsAuthResponse {
    fn from(r: o8_xml::products::GetCikkekAuthResponse) -> Self {
        GetProductsAuthResponse {
            result: r.GetCikkekAuthResult.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductsAuthResult {
    pub answer: Answer
}

impl From<o8_xml::products::GetCikkekAuthResult> for GetProductsAuthResult {
    fn from(r: o8_xml::products::GetCikkekAuthResult) -> Self {
        GetProductsAuthResult {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Vec<Product>,
    pub error: Option<Error>
}

impl From<o8_xml::products::valasz> for Answer {
    fn from(v: o8_xml::products::valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikk
                        .into_iter()
                        .map(|p| p.into())
                        .collect(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl From<o8_xml::products::Hiba> for Error {
    fn from(hiba: o8_xml::products::Hiba) -> Self {
        Error {
            code: hiba.kod,
            description: errors::translate_error(&hiba.leiras)
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
    pub size: Option<Size>,
    pub main_category_code: String,
    pub main_category_name: String,
    pub sell_unit: Option<f64>,
    pub origin_country: String
}

impl From<o8_xml::products::Cikk> for Product {
    fn from(c: o8_xml::products::Cikk) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam,
            name: c.cikknev,
            unit: c.me,
            base_unit: c.alapme,
            base_unit_qty: c.alapmenny,
            brand: c.gyarto,
            category_code: c.cikkcsoportkod,
            category_name: c.cikkcsoportnev,
            description: c.leiras,
            weight: c.tomeg,
            size: c.meret.map(|s| s.into()),
            main_category_code: c.focsoportkod,
            main_category_name: c.focsoportnev,
            sell_unit: c.ertmenny,
            origin_country: c.szarmorszag
        }
    }
}


#[derive(Serialize)]
pub struct Size {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>
}

impl From<o8_xml::products::Meret> for Size {
    fn from(meret: o8_xml::products::Meret) -> Self {
        Size {
            x: meret.xmeret,
            y: meret.ymeret,
            z: meret.zmeret
        }
    }
}