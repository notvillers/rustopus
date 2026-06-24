// Bulk english struct(s) fro XML(s) got from the Octopus call
use quick_xml;

use crate::{
    macros::out::OutModelDeriveOnly,
    forms::out::xml::{
        defaults,
        products,
        prices,
        stocks,
        images,
        barcode
    }
};

OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }
    
    pub struct Body {
        pub response: Response
    }

    pub struct Response {
        pub result: Result
    }

    pub struct Result {
        pub answer: Answer
    }

    pub struct Answer {
        pub version: String,
        pub products: Products,
        pub error: Vec<defaults::Error>
    }

    pub struct Products {
        pub product: Vec<Product>
    }

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
        pub size: Option<products::Size>,
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

    pub struct Images {
        pub image: Vec<Image>
    }

    pub struct Image {
        pub gallery: String,
        pub url: String
    }
}


impl From<(products::Envelope, Option<prices::Envelope>, Option<stocks::Envelope>, Option<images::Envelope>, Option<barcode::Envelope>)> for Envelope {
    fn from((c, p, s, i, b): (products::Envelope, Option<prices::Envelope>, Option<stocks::Envelope>, Option<images::Envelope>, Option<barcode::Envelope>)) -> Self {
        Self {
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


impl From<(products::Body, Option<prices::Body>, Option<stocks::Body>, Option<images::Body>, Option<barcode::Body>)> for Body {
    fn from((c, p, s, i, b): (products::Body, Option<prices::Body>, Option<stocks::Body>, Option<images::Body>, Option<barcode::Body>)) -> Self {
        Self {
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


impl From<(products::GetProductsAuthResponse, Option<prices::GetPriceAuthResponse>, Option<stocks::GetStockChangeAuthResponse>, Option<images::GetProductImagesAuthResponse>, Option<barcode::GetProductBarcodesResponse>)> for Response {
    fn from((c, p , s, i, b): (products::GetProductsAuthResponse, Option<prices::GetPriceAuthResponse>, Option<stocks::GetStockChangeAuthResponse>, Option<images::GetProductImagesAuthResponse>, Option<barcode::GetProductBarcodesResponse>)) -> Self {
        Self {
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


impl From<(products::GetProductsAuthResult, Option<prices::GetPriceAuthResult>, Option<stocks::GetStockChangeAuthResult>, Option<images::GetProductImagesAuthResult>, Option<barcode::GetProductBarcodesResult>)> for Result {
    fn from((c, p, s, i, b): (products::GetProductsAuthResult, Option<prices::GetPriceAuthResult>, Option<stocks::GetStockChangeAuthResult>, Option<images::GetProductImagesAuthResult>, Option<barcode::GetProductBarcodesResult>)) -> Self {
        Self {
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


impl From<(products::Answer, Option<prices::Answer>, Option<stocks::Answer>, Option<images::Answer>, Option<barcode::Answer>)> for Answer {
    fn from((c, p, s, i, b): (products::Answer, Option<prices::Answer>, Option<stocks::Answer>, Option<images::Answer>, Option<barcode::Answer>)) -> Self {
        let mut errors: Vec<defaults::Error> = vec![];
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

        Self {
            version: String::from("1.0"),
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


impl From<(Vec<products::Product>, Vec<prices::Price>, Vec<stocks::Product>, Vec<images::Product>, Vec<barcode::Barcode>)> for Products {
    fn from((c, p, s, i, b): (Vec<products::Product>, Vec<prices::Price>, Vec<stocks::Product>, Vec<images::Product>, Vec<barcode::Barcode>)) -> Self {
        use std::collections::HashMap;
        let price_map: HashMap<u64, &prices::Price> = p.iter().map(|x| (x.id, x)).collect();
        let stock_map: HashMap<u64, &stocks::Product> = s.iter().map(|x| (x.id, x)).collect();
        let image_map: HashMap<u64, &images::Product> = i.iter().map(|x| (x.id, x)).collect();
        let mut barcode_map: HashMap<u64, &barcode::Barcode> = HashMap::new();
        for bc in b.iter() {
            let entry = barcode_map.entry(bc.id).or_insert(bc);
            if bc.main_ean && !entry.main_ean {
                *entry = bc;
            }
        }
        Self {
            product: c.into_iter()
                .map(|x| {
                    let id = x.id;
                    Product::from((
                        x,
                        price_map.get(&id).copied(),
                        stock_map.get(&id).copied(),
                        image_map.get(&id).copied(),
                        barcode_map.get(&id).copied(),
                    ))
                })
                .collect()
        }
    }
}


impl From<(products::Product, Option<&prices::Price>, Option<&stocks::Product>, Option<&images::Product>, Option<&barcode::Barcode>)> for Product {
    fn from((c, a, k, i, b): (products::Product, Option<&prices::Price>, Option<&stocks::Product>, Option<&images::Product>, Option<&barcode::Barcode>)) -> Self {
        Self {
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


impl From<Option<&images::Product>> for Images {
    fn from(i: Option<&images::Product>) -> Self {
        Self {
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


pub fn error_struct(errors: Vec<defaults::Error>) -> Envelope {
    Envelope {
        body: Body {
            response: Response {
                result: Result {
                    answer: Answer {
                        version: "1.0".into(),
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
    let error = defaults::Error {
        code: code,
        description: description.to_string()
    };
    quick_xml::se::to_string(&error_struct(vec![error])).unwrap_or("<Envelope></Envelope>".into())
}
