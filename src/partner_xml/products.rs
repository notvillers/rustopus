/// Products english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::products::Envelope> for Envelope {
    fn from(e: o8_xml::products::Envelope) -> Self {
        Envelope {
            body: e.body.into()
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
            response: b.get_cikkek_auth_response.into()
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
            result: r.get_cikkek_auth_result.into()
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
    pub products: Products,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::products::Valasz> for Answer {
    fn from(v: o8_xml::products::Valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikk.into_iter().collect::<Products>(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl FromIterator<o8_xml::products::Cikk> for Products {
    fn from_iter<I: IntoIterator<Item = o8_xml::products::Cikk>>(iter: I) -> Self {
        Products {
            product: iter.into_iter().map(|x| x.into()).collect()
        }
    }
}


#[derive(Serialize, Clone)]
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
    pub oem_code: String,
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
            oem_code: c.gycikkszam,
            main_category_code: c.focsoportkod,
            main_category_name: c.focsoportnev,
            sell_unit: c.ertmenny,
            origin_country: c.szarmorszag
        }
    }
}


#[derive(Serialize, Clone, Copy)]
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


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetProductsAuthResponse {
                result: GetProductsAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        products: Products {
                            product: vec![]
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
