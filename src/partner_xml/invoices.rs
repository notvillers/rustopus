// Barcodes english struct(s) for XML(s) got from the Octopus call
use chrono::NaiveDate;
use serde::Serialize;
use quick_xml;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::invoices::Envelope> for Envelope {
    fn from(e: o8_xml::invoices::Envelope) -> Self {
        Envelope {
            body: e.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}

impl From<o8_xml::invoices::Body> for Body {
    fn from(b: o8_xml::invoices::Body) -> Self {
        Body {
            response: b.get_szamlak_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}

impl From<o8_xml::invoices::GetSzamlakAuthResponse> for Response {
    fn from(r: o8_xml::invoices::GetSzamlakAuthResponse) -> Self {
        Response {
            result: r.get_szamlak_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}

impl From<o8_xml::invoices::GetSzamlakAuthResult> for Result {
    fn from(r: o8_xml::invoices::GetSzamlakAuthResult) -> Self {
        Result {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub invoices: Invoices,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::invoices::Valasz> for Answer {
    fn from(v: o8_xml::invoices::Valasz) -> Self {
        Answer {
            version: v.verzio,
            invoices: v.szamlak.into(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Invoices {
    pub invoice: Vec<Invoice>,
}

impl From<o8_xml::invoices::Szamlak> for Invoices {
    fn from(sz: o8_xml::invoices::Szamlak) -> Self {
        Invoices {
            invoice: sz.szamla.into_iter().map(Invoice::from).collect()
        }
    }
}


#[derive(Serialize)]
pub struct Invoice {
    pub head: Head,
    pub products: Products
}

impl From<o8_xml::invoices::Szamla> for Invoice {
    fn from(sz: o8_xml::invoices::Szamla) -> Self {
        Invoice {
            head: sz.fej.into(),
            products: sz.tetelek.tetel.into()
        }
    }
}


#[derive(Serialize)]
pub struct Head {
    pub id: i64,
    pub no: Option<String>,
    pub date: Option<NaiveDate>,
    pub completition_date: Option<NaiveDate>,
    pub payment_deadline: Option<NaiveDate>,
    pub net_price: Option<f64>,
    pub price: Option<f64>,
    pub remaining: Option<f64>,
    pub cancellation_no: Option<String>,
    pub currency: String,
    pub pid: i64,
    pub partner_name: String,
    pub status: i64,
    pub foreign_order_no: Option<String>,
    pub delivery_name: Option<String>,
    pub delivery_country: Option<String>,
    pub delivery_zip: Option<String>,
    pub delivery_city: Option<String>,
    pub delivery_street: Option<String>
}

impl From<o8_xml::invoices::Fej> for Head {
    fn from(f: o8_xml::invoices::Fej) -> Self {
        Head {
            id: f.kiszamlakod,
            no: f.bizonylatszam,
            date: f.bizdatum,
            completition_date: f.teljdatum,
            payment_deadline: f.fizhat,
            net_price: f.devnetto,
            price: f.devbrutto,
            remaining: f.devtartozas,
            cancellation_no: f.stornobizszam,
            currency: f.dnem,
            pid: f.pid,
            partner_name: f.partnernev,
            status: f.bizstatus,
            foreign_order_no: f.idegenmegrszam,
            delivery_name: f.szallcimnev,
            delivery_country: f.szallorszag,
            delivery_zip: f.szallirsz,
            delivery_city: f.szallvaros,
            delivery_street: f.szallutca
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl From<Vec<o8_xml::invoices::Tetel>> for Products {
    fn from(t: Vec<o8_xml::invoices::Tetel>) -> Self {
        Products {
            product: t.into_iter().map(|x| x.into()).collect()
        }
    }
}


#[derive(Serialize)]
pub struct Product {
    pub lot_no: u64,
    pub id: u64,
    pub no: String,
    pub name: String,
    pub qty: Option<f64>,
    pub unit: String,
    pub net_unit_price: Option<f64>,
    pub unit_price: Option<f64>,
    pub net_price: Option<f64>,
    pub price: Option<f64>,
    pub order_no: Option<String>,
    pub order_foreign_no: Option<String>
}

impl From<o8_xml::invoices::Tetel> for Product {
    fn from(t: o8_xml::invoices::Tetel) -> Self {
        Product {
            lot_no: t.tetelszam,
            id: t.cikkid,
            no: t.cikkszam,
            name: t.cikknev,
            qty: t.menny,
            unit: t.me,
            net_unit_price: t.egysegar,
            unit_price: t.bregysegar,
            net_price: t.ertek,
            price: t.brertek,
            order_no: t.rbizonylatszam,
            order_foreign_no: t.ridegenmegrszam
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: Response {
                result: Result {
                    answer: Answer {
                        version: "1.0".into(),
                        invoices: Invoices {
                            invoice: vec![]
                        },
                        error: Some(partner_xml::defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_struct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
