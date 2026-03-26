use chrono::NaiveDate;
use serde::Serialize;
use crate::forms::r#in::xml::invoices as o8_invoices;

#[derive(Serialize)]
pub struct Product {
    pub id: i64,
    pub no: Option<String>,
    pub date: Option<NaiveDate>,
    pub completition_date: Option<NaiveDate>,
    pub payment_deadline: Option<NaiveDate>,
    pub currency: String,
    pub pid: i64,
    pub partner_name: String,
    pub foreign_order_no: Option<String>,
    pub delivery_name: Option<String>,
    pub delivery_country: Option<String>,
    pub delivery_zip: Option<String>,
    pub delivery_city: Option<String>,
    pub delivery_street: Option<String>,
    pub lot_no: u64,
    pub item_id: u64,
    pub item_no: String,
    pub item_name: String,
    pub qty: Option<f64>,
    pub unit: String,
    pub net_unit_price: Option<f64>,
    pub unit_price: Option<f64>,
    pub net_price: Option<f64>,
    pub price: Option<f64>,
    pub order_no: Option<String>,
    pub order_foreign_no: Option<String>
}

impl From<(o8_invoices::Fej, o8_invoices::Tetel)> for Product {
    fn from((f, c): (o8_invoices::Fej, o8_invoices::Tetel)) -> Self {
        Self {
            id: f.kiszamlakod,
            no: f.bizonylatszam,
            date: f.bizdatum,
            completition_date: f.teljdatum,
            payment_deadline: f.fizhat,
            currency: f.dnem,
            pid: f.pid,
            partner_name: f.partnernev,
            foreign_order_no: f.idegenmegrszam,
            delivery_name: f.szallcimnev,
            delivery_country: f.szallorszag,
            delivery_zip: f.szallirsz,
            delivery_city: f.szallvaros,
            delivery_street: f.szallutca,
            lot_no: c.tetelszam,
            item_id: c.cikkid,
            item_no: c.cikkszam,
            item_name: c.cikknev,
            qty: c.menny,
            unit: c.me,
            net_unit_price: c.egysegar,
            unit_price: c.bregysegar,
            net_price: c.ertek,
            price: c.brertek,
            order_no: c.rbizonylatszam,
            order_foreign_no: c.ridegenmegrszam
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub products: Vec<Product>
}

impl From<o8_invoices::Envelope> for Products {
    fn from(e: o8_invoices::Envelope) -> Self {
        let mut products: Vec<Product> = Vec::new();
        let szamlak = e.body.get_szamlak_auth_response.get_szamlak_auth_result.valasz.szamlak.szamla;
        for szamla in szamlak {
            let fej = szamla.fej;
            for tetel in szamla.tetelek.tetel {
                products.push((fej.clone(), tetel).into());
            }
        }
        Self {
            products
        }
    }
}
