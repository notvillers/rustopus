use chrono::NaiveDate;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Order {
    #[serde(rename = "@version")]
    pub version: Option<String>,
    pub header: Header,
    pub items: Items
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Header {
    pub pid: u64,
    pub foreign_order_numbers: Option<String>,
    pub delivery_mode: u8,
    pub delivery_note: Option<String>,
    pub enduser_id: Option<String>,
    pub enduser_name: Option<String>,
    pub enduser_contact_id: Option<String>,
    pub enduser_contact: Option<String>,
    pub contact_phone: Option<String>,
    pub contact_email: Option<String>,
    pub note: Option<String>,
    pub note_warehouse: Option<String>,
    pub note_hidden: Option<String>,
    #[serde(default)]
    pub invoice_address: Option<Address>,
    #[serde(default)]
    pub delivery_address: Option<Address>,
    pub enduser_delivery_mode: Option<u8>,
    pub enduser_currency: Option<String>,
    pub enduser_payment_method: Option<u8>,
    pub enduser_payment_deadline: Option<NaiveDate>,
    pub enduser_vat_no: Option<String>,
    pub enduser_type: Option<u8>,
    pub enduser_invoice: Option<u8>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Address {
    pub name: Option<String>,
    pub country: Option<String>,
    pub zip: Option<String>,
    pub city: Option<String>,
    pub street: Option<String>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Items {
    #[serde(rename = "item", default)]
    pub items: Vec<Item>
}


#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Item {
    pub lot_no: u64,
    pub no: String,
    pub qty: f64,
    pub enduser_price: Option<f64>,
    pub note: Option<String>
}
