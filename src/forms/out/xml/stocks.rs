/// Stocks english struct(s) for XML(s) got from the Octopus call
use quick_xml;

use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::{
        r#in::xml::stocks as o8_stocks,
        out::xml::defaults as p_defaults
    }
};

OutModelDeriveSerializeOnly! {
    pub struct Envelope {
        pub body: Body
    }

    pub struct Body {
        pub response: GetStockChangeAuthResponse
    }

    pub struct GetStockChangeAuthResponse {
        pub result: GetStockChangeAuthResult
    }

    pub struct GetStockChangeAuthResult {
        pub answer: Answer
    }

    pub struct Answer {
        pub version: String,
        pub products: Products,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }

    pub struct Products {
        pub product: Vec<Product>
    }

    pub struct Product {
        pub id: u64,
        pub no: String,
        pub stock: Option<f64>
    }
}


impl From<o8_stocks::Envelope> for Envelope {
    fn from(e: o8_stocks::Envelope) -> Self {
        Self {
            body: e.body.into()
        }
    }
}


impl From<o8_stocks::Body> for Body {
    fn from(b: o8_stocks::Body) -> Self {
        Self {
            response: b.get_cikkek_keszlet_valtozas_auth_response.into()
        }
    }
}


impl From<o8_stocks::GetCikkekKeszletValtozasAuthResponse> for GetStockChangeAuthResponse {
    fn from(r: o8_stocks::GetCikkekKeszletValtozasAuthResponse) -> Self {
        Self {
            result: r.get_cikkek_keszlet_valtozas_auth_result.into()
        }
    }
}


impl From<o8_stocks::GetCikkekKeszletValtozasAuthResult> for GetStockChangeAuthResult {
    fn from(r: o8_stocks::GetCikkekKeszletValtozasAuthResult) -> Self {
        Self {
            answer: r.valasz.into()
        }
    }
}


impl From<o8_stocks::Valasz> for Answer {
    fn from(v: o8_stocks::Valasz) -> Self {
        Self {
            version: v.verzio,
            products: v.cikkek.into(),
            error: v.hiba.map(|x| x.into())
        }
    }
}


impl From<o8_stocks::Cikkek> for Products {
    fn from(c: o8_stocks::Cikkek) -> Self {
        Self {
            product: c.cikk
                        .into_iter()
                        .map(|x| x.into())
                        .collect()
        }
    }
}


impl From<o8_stocks::Cikk> for Product {
    fn from(c: o8_stocks::Cikk) -> Self {
        Self {
            id: c.cikkid,
            no: c.cikkszam, 
            stock: c.szabad
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetStockChangeAuthResponse {
                result: GetStockChangeAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        products: Products {
                            product: Vec::new()
                        },
                        error: Some(p_defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_struct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
