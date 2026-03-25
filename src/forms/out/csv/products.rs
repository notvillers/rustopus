use serde::Serialize;
use crate::forms::r#in::xml::products as o8_products;

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
    pub origin_country: String
}

impl From<o8_products::Cikk> for Product {
    fn from(c: o8_products::Cikk) -> Self {
        Self {
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
            xsize: c.meret
                .clone()
                .and_then(|s| s.xmeret),
            ysize: c.meret
                .clone()
                .and_then(|s| s.ymeret),
            zsize: c.meret
                .and_then(|s| s.zmeret),
            oem_code: c.gycikkszam,
            main_category_code: c.focsoportkod,
            main_category_name: c.focsoportnev,
            sell_unit: c.ertmenny,
            origin_country: c.szarmorszag
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub products: Vec<Product>
}

impl From<o8_products::Envelope> for Products {
    fn from(e: o8_products::Envelope) -> Self {
        let products = e.body.get_cikkek_auth_response.get_cikkek_auth_result.valasz.cikk;
        Self {
            products: products
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}
