use serde::Serialize;
use crate::forms::out::xml::bulk as p_bulk;

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
    pub xsize: Option<f64>,
    pub ysize: Option<f64>,
    pub zsize: Option<f64>,
    pub oem_code: String,
    pub main_category_code: String,
    pub main_category_name: String,
    pub sell_unit: Option<f64>,
    pub origin_country: String,
    pub price: Option<f64>,
    pub currency: String,
    pub image: String,
    pub stock: Option<f64>
}

impl From<p_bulk::Product> for Product {
    fn from(c: p_bulk::Product) -> Self {
        Self {
            id: c.id,
            no: c.no,
            name: c.name,
            unit: c.unit,
            base_unit: c.base_unit,
            base_unit_qty: c.base_unit_qty,
            brand: c.brand,
            category_code: c.category_code,
            category_name: c.category_name,
            description: c.description,
            weight: c.weight,
            xsize: c.size
                .as_ref()
                .and_then(|s| s.x),
            ysize: c.size
                .as_ref()
                .and_then(|s| s.y),
            zsize: c.size
                .as_ref()
                .and_then(|s| s.z),
            oem_code: c.oem_code,
            main_category_code: c.main_category_code,
            main_category_name: c.main_category_name,
            sell_unit: c.sell_unit,
            origin_country: c.origin_country,
            price: c.price,
            currency: c.currency
                .unwrap_or("".to_string()),
            image: c.images.image
                .first()
                .map(|i| i.url.clone())
                .unwrap_or_else(|| "".to_string()),
            stock: c.stock
        }
    }
}


pub struct Products {
    pub products: Vec<Product>
}

impl From<p_bulk::Envelope> for Products {
    fn from(e: p_bulk::Envelope) -> Self {
        let products = e.body.response.result.answer.products.product;
        Self {
            products: products
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}
