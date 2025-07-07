/// Bulk english struct(s) fro XML(s) got from the Octopus call

use serde::Serialize;
use quick_xml;

use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<(&partner_xml::products::Envelope, Option<&partner_xml::prices::Envelope>, Option<&partner_xml::stocks::Envelope>, Option<&partner_xml::images::Envelope>)> for Envelope {
    fn from((c, p, s, i): (&partner_xml::products::Envelope, Option<&partner_xml::prices::Envelope>, Option<&partner_xml::stocks::Envelope>, Option<&partner_xml::images::Envelope>)) -> Self {
        Envelope {
            body: (
                &c.body,
                p.as_ref().map(|p| &p.body),
                s.as_ref().map(|s| &s.body),
                i.as_ref().map(|i| &i.body)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}

impl From<(&partner_xml::products::Body, Option<&partner_xml::prices::Body>, Option<&partner_xml::stocks::Body>, Option<&partner_xml::images::Body>)> for Body {
    fn from((c, p, s, i): (&partner_xml::products::Body, Option<&partner_xml::prices::Body>, Option<&partner_xml::stocks::Body>, Option<&partner_xml::images::Body>)) -> Self {
        Body {
            response: (
                &c.response,
                p.as_ref().map(|p| &p.response),
                s.as_ref().map(|s| &s.response),
                i.as_ref().map(|i| &i.response)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}

impl From<(&partner_xml::products::GetProductsAuthResponse, Option<&partner_xml::prices::GetPriceAuthResponse>, Option<&partner_xml::stocks::GetStockChangeAuthResponse>, Option<&partner_xml::images::GetProductImagesAuthResponse>)> for Response {
    fn from((c, p , s, i): (&partner_xml::products::GetProductsAuthResponse, Option<&partner_xml::prices::GetPriceAuthResponse>, Option<&partner_xml::stocks::GetStockChangeAuthResponse>, Option<&partner_xml::images::GetProductImagesAuthResponse>)) -> Self {
        Response {
            result: (
                &c.result,
                p.as_ref().map(|p| &p.result),
                s.as_ref().map(|s| &s.result),
                i.as_ref().map(|i| &i.result)
            ).into()
        }
    }
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}

impl From<(&partner_xml::products::GetProductsAuthResult, Option<&partner_xml::prices::GetPriceAuthResult>, Option<&partner_xml::stocks::GetStockChangeAuthResult>, Option<&partner_xml::images::GetProductImagesAuthResult>)> for Result {
    fn from((c, p, s, i): (&partner_xml::products::GetProductsAuthResult, Option<&partner_xml::prices::GetPriceAuthResult>, Option<&partner_xml::stocks::GetStockChangeAuthResult>, Option<&partner_xml::images::GetProductImagesAuthResult>)) -> Self {
        Result {
            answer: (
                &c.answer,
                p.as_ref().map(|p| &p.answer),
                s.as_ref().map(|s| &s.answer),
                i.as_ref().map(|i| &i.answer)
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

impl From<(&partner_xml::products::Answer, Option<&partner_xml::prices::Answer>, Option<&partner_xml::stocks::Answer>, Option<&partner_xml::images::Answer>)> for Answer {
    fn from((c, p, s, i): (&partner_xml::products::Answer, Option<&partner_xml::prices::Answer>, Option<&partner_xml::stocks::Answer>, Option<&partner_xml::images::Answer>)) -> Self {
        let mut errors: Vec<partner_xml::defaults::Error> = vec![];
        [
            c.error.as_ref(),
            p.as_ref().and_then(|v| v.error.as_ref()),
            s.as_ref().and_then(|v| v.error.as_ref()),
            i.as_ref().and_then(|v| v.error.as_ref())
        ]
            .into_iter()
            .flatten()
            .for_each(|e| errors.push(e.clone()));

        Answer {
            version: "1.0".to_string(),
            products: (
                &c.products.product,
                p.as_ref().map_or(&vec![], |p| &p.prices.price),
                s.as_ref().map_or(&vec![], |s| &s.products.product),
                i.as_ref().map_or(&vec![], |i| &i.products.product)
            ).into(),
            error: errors
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl From<(&Vec<partner_xml::products::Product>, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>)> for Products {
    fn from((c, p, s , i): (&Vec<partner_xml::products::Product>, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>)) -> Self {
        Products {
            product: c.iter()
                .map(|c| Product::from((c, p, s, i)))
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
    pub images: Images
}

impl From<(&partner_xml::products::Product, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>)> for Product {
    fn from((c, p, s, i): (&partner_xml::products::Product, &Vec<partner_xml::prices::Price>, &Vec<partner_xml::stocks::Product>, &Vec<partner_xml::images::Product>)) -> Self {
        let price = p.iter().find(|price| price.id == c.id);
        let stock = s.iter().find(|stock| stock.id == c.id);
        let image = i.iter().find(|image| image.id == c.id);
        let product: Product = (c, price, stock, image).into();
        product
    }
}

impl From<(&partner_xml::products::Product, Option<&partner_xml::prices::Price>, Option<&partner_xml::stocks::Product>, Option<&partner_xml::images::Product>)> for Product {
    fn from((c, a, k, i): (&partner_xml::products::Product, Option<&partner_xml::prices::Price>, Option<&partner_xml::stocks::Product>, Option<&partner_xml::images::Product>)) -> Self {

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
            price: a.as_ref().map_or(None, |a| a.sale_price),
            currency: a.map_or(None, |a| Some(a.currency.clone())),
            stock: k.map_or(None, |k| k.stock),
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
