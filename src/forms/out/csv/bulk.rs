use serde::Serialize;

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
    pub list_price: Option<f64>,
    pub price: Option<f64>,
    pub sale_price: Option<f64>,
    pub currency: String,
    pub image: String,
    pub stock: Option<f64>
}
