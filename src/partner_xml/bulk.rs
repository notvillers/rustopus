/// Bulk english struct(s) fro XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<(partner_xml::products::Envelope, Option<partner_xml::prices::Envelope>, Option<partner_xml::stocks::Envelope>, Option<partner_xml::images::Envelope>, Option<partner_xml::barcode::Envelope>)> for Envelope {
    fn from((c, p, s, i, b): (partner_xml::products::Envelope, Option<partner_xml::prices::Envelope>, Option<partner_xml::stocks::Envelope>, Option<partner_xml::images::Envelope>, Option<partner_xml::barcode::Envelope>)) -> Self {
        Envelope {
            body: (
                c.body,
                p.map(|x| x.body),
                s.map(|x| x.body),
                i.map(|x| x.body),
                b.map(|x| x.body)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}

impl From<(partner_xml::products::Body, Option<partner_xml::prices::Body>, Option<partner_xml::stocks::Body>, Option<partner_xml::images::Body>, Option<partner_xml::barcode::Body>)> for Body {
    fn from((c, p, s, i, b): (partner_xml::products::Body, Option<partner_xml::prices::Body>, Option<partner_xml::stocks::Body>, Option<partner_xml::images::Body>, Option<partner_xml::barcode::Body>)) -> Self {
        Body {
            response: (
                c.response,
                p.map(|x| x.response),
                s.map(|x| x.response),
                i.map(|x| x.response),
                b.map(|x| x.response)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}

impl From<(partner_xml::products::GetProductsAuthResponse, Option<partner_xml::prices::GetPriceAuthResponse>, Option<partner_xml::stocks::GetStockChangeAuthResponse>, Option<partner_xml::images::GetProductImagesAuthResponse>, Option<partner_xml::barcode::GetProductBarcodesResponse>)> for Response {
    fn from((c, p , s, i, b): (partner_xml::products::GetProductsAuthResponse, Option<partner_xml::prices::GetPriceAuthResponse>, Option<partner_xml::stocks::GetStockChangeAuthResponse>, Option<partner_xml::images::GetProductImagesAuthResponse>, Option<partner_xml::barcode::GetProductBarcodesResponse>)) -> Self {
        Response {
            result: (
                c.result,
                p.map(|x| x.result),
                s.map(|x| x.result),
                i.map(|x| x.result),
                b.map(|x| x.result),
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}

impl From<(partner_xml::products::GetProductsAuthResult, Option<partner_xml::prices::GetPriceAuthResult>, Option<partner_xml::stocks::GetStockChangeAuthResult>, Option<partner_xml::images::GetProductImagesAuthResult>, Option<partner_xml::barcode::GetProductBarcodesResult>)> for Result {
    fn from((c, p, s, i, b): (partner_xml::products::GetProductsAuthResult, Option<partner_xml::prices::GetPriceAuthResult>, Option<partner_xml::stocks::GetStockChangeAuthResult>, Option<partner_xml::images::GetProductImagesAuthResult>, Option<partner_xml::barcode::GetProductBarcodesResult>)) -> Self {
        Result {
            answer: (
                c.answer,
                p.map(|x| x.answer),
                s.map(|x| x.answer),
                i.map(|x| x.answer),
                b.map(|x| x.answer)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Vec<partner_xml::defaults::Error>
}

impl From<(partner_xml::products::Answer, Option<partner_xml::prices::Answer>, Option<partner_xml::stocks::Answer>, Option<partner_xml::images::Answer>, Option<partner_xml::barcode::Answer>)> for Answer {
    fn from((c, p, s, i, b): (partner_xml::products::Answer, Option<partner_xml::prices::Answer>, Option<partner_xml::stocks::Answer>, Option<partner_xml::images::Answer>, Option<partner_xml::barcode::Answer>)) -> Self {
        let mut errors: Vec<partner_xml::defaults::Error> = vec![];
        [
            c.error.as_ref(),
            p.as_ref().and_then(|x| x.error.as_ref()),
            s.as_ref().and_then(|x| x.error.as_ref()),
            i.as_ref().and_then(|x| x.error.as_ref()),
            b.as_ref().and_then(|x| x.error.as_ref())
        ]
            .into_iter()
            .flatten()
            .for_each(|x| errors.push(x.clone()));

        Answer {
            version: "1.0".to_string(),
            products: (
                c.products.product,
                p.map_or(vec![], |x| x.prices.price),
                s.map_or(vec![], |x| x.products.product),
                i.map_or(vec![], |x| x.products.product),
                b.map_or(vec![], |x| x.barcodes.barcode)
            ).into(),
            error: errors
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl From<(Vec<partner_xml::products::Product>, Vec<partner_xml::prices::Price>, Vec<partner_xml::stocks::Product>, Vec<partner_xml::images::Product>, Vec<partner_xml::barcode::Barcode>)> for Products {
    fn from((c, p, s , i, b): (Vec<partner_xml::products::Product>, Vec<partner_xml::prices::Price>, Vec<partner_xml::stocks::Product>, Vec<partner_xml::images::Product>, Vec<partner_xml::barcode::Barcode>)) -> Self {
        Products {
            product: c.into_iter()
                .map(|x| Product::from((x, &p, &s, &i, &b)))
                .collect()
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
    pub oem_code: String,
    pub category_code: String,
    pub category_name: String,
    pub description: String,
    pub weight: Option<f64>,
    pub size: Option<partner_xml::products::Size>,
    pub main_category_code: String,
    pub main_category_name: String,
    pub sell_unit: Option<f64>,
    pub origin_country: String,
    pub price: Option<f64>,
    pub currency: Option<String>,
    pub stock: Option<f64>,
    pub ean: Option<String>,
    pub images: Images
}

impl From<(partner_xml::products::Product, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>, &Vec<partner_xml::barcode::Barcode>)> for Product {
    fn from((c, p, s, i, b): (partner_xml::products::Product, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>, &Vec<partner_xml::barcode::Barcode>)) -> Self {
        let price = p.iter().find(|x| x.id == c.id);
        let stock = s.iter().find(|x| x.id == c.id);
        let ean = match b.iter().find(|x| x.id == c.id && x.main_ean) {
            Some(barcode) => Some(barcode),
            _ => b.iter().find(|x| x.id == c.id)
        };
        let image = i.iter().find(|x| x.id == c.id);
        let product: Product = (c, price, stock, image, ean).into();
        product
    }
}

impl From<(partner_xml::products::Product, Option<&partner_xml::prices::Price>, Option<&partner_xml::stocks::Product>, Option<&partner_xml::images::Product>, Option<&partner_xml::barcode::Barcode>)> for Product {
    fn from((c, a, k, i, b): (partner_xml::products::Product, Option<&partner_xml::prices::Price>, Option<&partner_xml::stocks::Product>, Option<&partner_xml::images::Product>, Option<&partner_xml::barcode::Barcode>)) -> Self {
        Product {
            id: c.id,
            no: c.no.clone(),
            name: c.name.clone(),
            unit: c.unit.clone(),
            base_unit: c.base_unit.clone(),
            base_unit_qty: c.base_unit_qty,
            brand: c.brand.clone(),
            category_code: c.category_code.clone(),
            category_name: c.category_name.clone(),
            description: c.description.clone(),
            weight: c.weight,
            size: c.size.as_ref().map(|s| (*s).clone()),
            oem_code: c.oem_code.clone(),
            main_category_code: c.main_category_code.clone(),
            main_category_name: c.main_category_name.clone(),
            sell_unit: c.sell_unit,
            origin_country: c.origin_country.clone(),
            price: a.as_ref().map_or(None, |x| x.sale_price),
            currency: a.map_or(None, |x| Some(x.currency.clone())),
            stock: k.map_or(None, |x| x.stock),
            ean: b.map_or(None, |x| Some(x.ean.clone())),
            images: i.into()
        }
    }
}


#[derive(Serialize)]
pub struct Images {
    pub image: Vec<Image>
}


impl From<Option<&partner_xml::images::Product>> for Images {
    fn from(i: Option<&partner_xml::images::Product>) -> Self {
        Images {
            image: i
                .map(|i_c| {
                    i_c.images.image.iter().map(|img| Image {
                        gallery: img.gallery.clone(),
                        url: img.url.clone()
                    }).collect()
                })
                .unwrap_or_else(Vec::new)
        }
    }
}


#[derive(Serialize)]
pub struct Image {
    pub gallery: String,
    pub url: String
}


#[derive(Serialize)]
pub struct Size {
    pub x: Option<f64>,
    pub y: Option<f64>,
    pub z: Option<f64>
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


pub fn error_struct_xml(code: u64, description: &str) -> String {
    let error = partner_xml::defaults::Error {
        code: code,
        description: description.to_string()
    };
    match quick_xml::se::to_string(&error_struct(vec![error])) {
        Ok(xml) => xml,
        _ => "<Envelope></Envelope>".to_string()
    }
}
