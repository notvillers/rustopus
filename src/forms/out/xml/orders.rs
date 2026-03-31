use chrono::NaiveDate;
use serde::Serialize;
use quick_xml::escape::escape;
use crate::forms::r#in::xml::orders as p_orders;

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
        xmlns,
        escape(rendelesxml),
        authcode
    )
}


#[derive(Debug, Serialize)]
#[serde(rename = "rendeles")]
pub struct Rendeles {
    #[serde(rename = "@verzio")]
    pub verzio: Option<String>,
    pub fej: Fej,
    pub tetelek: Tetelek
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


#[derive(Debug, Serialize)]
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


#[derive(Debug, Serialize)]
pub struct Cim {
    pub cimnev: Option<String>,
    pub orszag: Option<String>,
    pub irsz: Option<String>,
    pub varos: Option<String>,
    pub utca: Option<String>
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


#[derive(Debug, Serialize)]
#[serde(rename = "tetelek")]
pub struct Tetelek {
    pub tetel: Vec<Tetel>
}

impl From<p_orders::Items> for Tetelek {
    fn from(is: p_orders::Items) -> Self {
        let items = is.items;
        Self {
            tetel: items.into_iter().map(|x| x.into()).collect()
        }
    }
}


#[derive(Debug, Serialize)]
#[serde(rename = "tetel")]
pub struct Tetel {
    pub tetelszam: u64,
    pub cikkszam: String,
    pub mennyiseg: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vegfegysar: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub megj: Option<String>
}

impl From<p_orders::Item> for Tetel {
    fn from(i: p_orders::Item) -> Self {
        Self {
            tetelszam: i.lot_no,
            cikkszam: i.no,
            mennyiseg: i.qty,
            vegfegysar: i.enduser_price,
            megj: i.note
        }
    }
}

pub fn error_struct_xml(_: u64, _: &str) -> String {
    "<Envelope></Envelope>".to_string()
}
