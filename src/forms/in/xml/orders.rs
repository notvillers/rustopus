use chrono::NaiveDate;
use crate::macros::r#in::O8ModelLowercase;

O8ModelLowercase! {
    pub struct Order {
        #[serde(rename = "@version", default)]
        pub version: String,
        pub header: Header,
        pub items: Items
    }
    
    pub struct Header {
        pub pid: u64,
        pub foreign_order_number: Option<String>,
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

    pub struct Address {
        pub name: Option<String>,
        pub country: Option<String>,
        pub zip: Option<String>,
        pub city: Option<String>,
        pub street: Option<String>
    }
    
    pub struct Items {
        #[serde(rename = "item", default)]
        pub items: Vec<Item>
    }
    
    pub struct Item {
        pub lot_no: u64,
        pub no: String,
        pub qty: f64,
        /// Net unit price to record the line at (Octopus `egysar`). Octopus
        /// honours it only where Orink has enabled it for the partner, and
        /// otherwise records its own current price. Accepts `3506.8` and
        /// `3506,8` alike.
        #[serde(default, deserialize_with = "decimal_point_or_comma")]
        pub unit_price: Option<f64>,
        pub enduser_price: Option<f64>,
        pub note: Option<String>
    }
}


/// A number written with a decimal point or a decimal comma; blank is absent.
fn decimal_point_or_comma<'de, D>(deserializer: D) -> Result<Option<f64>, D::Error>
where
    D: serde::Deserializer<'de>
{
    use serde::Deserialize;

    let Some(text) = Option::<String>::deserialize(deserializer)? else {
        return Ok(None)
    };
    let text = text.trim();
    if text.is_empty() {
        return Ok(None)
    }
    text.replace(',', ".").parse::<f64>()
        .map(Some)
        .map_err(|_| serde::de::Error::custom(format!("'{}' is not a number", text)))
}
