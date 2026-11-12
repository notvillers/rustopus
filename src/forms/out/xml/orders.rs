use chrono::NaiveDate;
use quick_xml::escape::escape;

use crate::{
    macros::out::OutModelDeriveOnly,
    forms::r#in::xml::orders as p_orders
};

pub fn get_request_string(xmlns: &str, rendelesxml: &str, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <RendelesFeladasAuth xmlns="{}">
                    <rendelesxml>{}</rendelesxml>
                    <authcode>{}</authcode>
                </RendelesFeladasAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        escape(xmlns),
        // Escaped exactly once, and not for the same reason as the two beside
        // it: Octopus wants the order document as *text* inside this element,
        // not as nested markup. `to_xml_string` has already escaped the field
        // contents, so a further pass here would triple-encode the order.
        escape(rendelesxml),
        escape(authcode)
    )
}

OutModelDeriveOnly! {
    #[serde(rename = "rendeles")]
    pub struct Rendeles {
        #[serde(rename = "@verzio")]
        pub verzio: String,
        pub fej: Fej,
        pub tetelek: Tetelek
    }

    #[serde(rename = "fej")]
    pub struct Fej {
        pub partnerid: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub idegen_megrendelesszam: Option<String>,
        pub szallitasi_mod: u8,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub szallitasi_megj: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhasznaloazon: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhasznalonev: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelh_ugyintezoid: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelh_ugyintezo: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ugyintezo_telszam: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub ugyintezo_email: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub megj: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub megjraktar: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub belsomegj: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub szamlazasi_cim: Option<Cim>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub szallitasi_cim: Option<Cim>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhasznalo_szallitasi_mod: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelh_dnem: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhfizmod: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none", with = "hungarian_date_format_opt")]
        pub vegfelhfizhat: Option<NaiveDate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhadoszam: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfelhtipus: Option<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub masnevszaml: Option<u8>
    }
    
    pub struct Cim {
        pub cimnev: Option<String>,
        pub orszag: Option<String>,
        pub irsz: Option<String>,
        pub varos: Option<String>,
        pub utca: Option<String>
    }
    
    #[serde(rename = "tetelek")]
    pub struct Tetelek {
        pub tetel: Vec<Tetel>
    }

    #[serde(rename = "tetel")]
    pub struct Tetel {
        pub tetelszam: u64,
        pub cikkszam: String,
        pub mennyiseg: f64,
        // Octopus writes its numbers with a decimal comma, and whether it reads
        // a point is unproven: every quantity sent so far was a whole number.
        #[serde(skip_serializing_if = "Option::is_none", with = "hungarian_decimal_opt")]
        pub egysar: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub vegfegysar: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub megj: Option<String>
    }
}


impl From<p_orders::Order> for Rendeles {
    fn from(o: p_orders::Order) -> Self {
        Self {
            verzio: o.version,
            fej: o.header.into(),
            tetelek: o.items.into()
        }
    }
}


impl From<p_orders::Header> for Fej {
    fn from(h: p_orders::Header) -> Self {
        Self {
            partnerid: h.pid,
            idegen_megrendelesszam: h.foreign_order_number,
            szallitasi_mod: h.delivery_mode,
            szallitasi_megj: h.delivery_note,
            vegfelhasznaloazon: h.enduser_id,
            vegfelhasznalonev: h.enduser_name,
            vegfelh_ugyintezoid: h.enduser_contact_id,
            vegfelh_ugyintezo: h.enduser_contact,
            ugyintezo_telszam: h.contact_phone,
            ugyintezo_email: h.contact_email,
            megj: h.note,
            megjraktar: h.note_warehouse,
            belsomegj: h.note_hidden,
            szamlazasi_cim: h.invoice_address.map(Into::into),
            szallitasi_cim: h.delivery_address.map(Into::into),
            vegfelhasznalo_szallitasi_mod: h.enduser_delivery_mode,
            vegfelh_dnem: h.enduser_currency,
            vegfelhfizmod: h.enduser_payment_method,
            vegfelhfizhat: h.enduser_payment_deadline,
            vegfelhadoszam: h.enduser_vat_no,
            vegfelhtipus: h.enduser_type,
            masnevszaml: h.enduser_invoice
        }
    }
}


impl From<p_orders::Address> for Cim {
    fn from(a: p_orders::Address) -> Self {
        Self {
            cimnev: a.name,
            orszag: a.country,
            irsz: a.zip,
            varos: a.city,
            utca: a.street
        }
    }
}


mod hungarian_date_format_opt {
    use super::*;
    const FORMAT: &str = "%Y.%m.%d";

    pub fn serialize<S>(date: &Option<NaiveDate>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match date {
            Some(d) => serializer.serialize_str(&d.format(FORMAT).to_string()),
            None => serializer.serialize_none(),
        }
    }
}


/// `3506.8` → `3506,8`, the way Octopus writes numbers. Rounded to Octopus's
/// own seven decimals first, so float noise (`0.30000000000000004`) never
/// reaches it; a whole number carries no separator at all.
mod hungarian_decimal_opt {
    pub fn serialize<S>(value: &Option<f64>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match value {
            Some(value) => serializer.serialize_str(&super::hungarian_decimal(*value)),
            None => serializer.serialize_none(),
        }
    }
}

fn hungarian_decimal(value: f64) -> String {
    ((value * 1e7).round() / 1e7).to_string().replace('.', ",")
}


impl From<p_orders::Items> for Tetelek {
    fn from(is: p_orders::Items) -> Self {
        let items = is.items;
        Self {
            tetel: items.into_iter().map(|x| x.into()).collect()
        }
    }
}


impl From<p_orders::Item> for Tetel {
    fn from(i: p_orders::Item) -> Self {
        Self {
            tetelszam: i.lot_no,
            cikkszam: i.no,
            mennyiseg: i.qty,
            egysar: i.unit_price,
            vegfegysar: i.enduser_price,
            megj: i.note
        }
    }
}


pub fn error_struct_xml(_: u64, _: &str) -> String {
    "<Envelope></Envelope>".to_string()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_reach_octopus_with_a_decimal_comma() {
        for (value, sent) in [(3506.8, "3506,8"), (1200.0, "1200"), (0.1 + 0.2, "0,3"), (12.123456789, "12,1234568")] {
            assert_eq!(hungarian_decimal(value), sent);
        }

        // A REST caller may write either separator; Octopus gets a comma.
        for price in ["3506.8", "3506,8"] {
            let body = format!(
                "<order version=\"1.0\"><header><pid>1</pid><delivery_mode>2</delivery_mode></header>\
                 <items><item><lot_no>1</lot_no><no>A</no><qty>2</qty><unit_price>{price}</unit_price></item></items></order>"
            );
            let order: p_orders::Order = quick_xml::de::from_str(&body).expect("the order parses");
            let sent = crate::service::get_data::to_xml_string(&Rendeles::from(order));
            assert!(sent.contains("<egysar>3506,8</egysar>"), "{sent}");
        }
    }
}
