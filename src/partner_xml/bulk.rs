/// Bulk english struct(s) fro XML(s) got from the Octopus call

use serde::Serialize;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Vec<partner_xml::defaults::Error>
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
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
    pub oem_code: String,
    pub category_code: String,
    pub category_name: String,
    pub description: String,
    pub weight: Option<f64>,
    pub size: Option<Size>,
    pub main_category_code: String,
    pub main_category_name: String,
    pub sell_unit: Option<f64>,
    pub origin_country: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub stock: Option<f64>,
    pub images: Vec<Image>
}


#[derive(Serialize)]
pub struct Image {
    pub url: String
}


impl From<(&o8_xml::products::Cikk, Option<&o8_xml::prices::Ar>, Option<&o8_xml::stocks::Cikk>, Option<&o8_xml::images::Cikk>)> for Product {
    fn from((c, a, k, i): (&o8_xml::products::Cikk, Option<&o8_xml::prices::Ar>, Option<&o8_xml::stocks::Cikk>, Option<&o8_xml::images::Cikk>)) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam.clone(),
            name: c.cikknev.clone(),
            unit: c.me.clone(),
            base_unit: c.alapme.clone(),
            base_unit_qty: c.alapmenny,
            brand: c.gyarto.clone(),
            category_code: c.cikkcsoportkod.clone(),
            category_name: c.cikkcsoportnev.clone(),
            description: c.leiras.clone(),
            weight: c.tomeg,
            size: c.meret.as_ref().map(|s| s.into()),
            oem_code: c.gycikkszam.clone(),
            main_category_code: c.focsoportkod.clone(),
            main_category_name: c.focsoportnev.clone(),
            sell_unit: c.ertmenny,
            origin_country: c.szarmorszag.clone(),
            price: a.as_ref().map_or(None, |a| a.akcios_ar),
            currency: a.map_or(None, |a| Some(a.devizanem.clone())),
            stock: k.map_or(None, |k| k.szabad),
            images: i.map(
                |i_c| {
                    i_c.kepek.kep.iter().map(
                        |k| Image {
                            url: k.url.clone()
                        }
                    ).collect::<Vec<Image>>()
                }
            ).unwrap_or(vec![])
        }
    }
}


#[derive(Serialize)]
pub struct Size {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>
}

impl From<&o8_xml::products::Meret> for Size {
    fn from(m: &o8_xml::products::Meret) -> Self {
        Size {
            x: m.xmeret,
            y: m.ymeret,
            z: m.zmeret
        }
    }
}


pub fn error_struct(errors: Vec<partner_xml::defaults::Error>) -> Envelope {
    Envelope {
        body: Body {
            response: Response {
                result: Result {
                    answer: Answer {
                        version: "1.0".to_string(),
                        products: Products {
                            product: Vec::new()
                        },
                        error: errors
                    }
                }
            }
        }
    }
}
