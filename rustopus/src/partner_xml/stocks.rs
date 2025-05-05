use serde::Serialize;
use crate::o8_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::stocks::Envelope> for Envelope {
    fn from(e: o8_xml::stocks::Envelope) -> Self {
        Envelope {
            body: e.Body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetStockChangeAuthResponse
}

impl From<o8_xml::stocks::Body> for Body {
    fn from(b: o8_xml::stocks::Body) -> Self {
        Body {
            response: b.GetCikkekKeszletValtozasAuthResponse.into()
        }
    }
}



#[derive(Serialize)]
pub struct GetStockChangeAuthResponse {
    pub result: GetStockChangeAuthResult
}

impl From<o8_xml::stocks::GetCikkekKeszletValtozasAuthResponse> for GetStockChangeAuthResponse {
    fn from(r: o8_xml::stocks::GetCikkekKeszletValtozasAuthResponse) -> Self {
        GetStockChangeAuthResponse {
            result: r.GetCikkekKeszletValtozasAuthResult.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetStockChangeAuthResult {
    pub answer: Answer
}

impl From<o8_xml::stocks::GetCikkekKeszletValtozasAuthResult> for GetStockChangeAuthResult {
    fn from(r: o8_xml::stocks::GetCikkekKeszletValtozasAuthResult) -> Self {
        GetStockChangeAuthResult {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Option<Error>
}

impl From<o8_xml::stocks::valasz> for Answer {
    fn from(v: o8_xml::stocks::valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikkek.into(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl From<o8_xml::stocks::Hiba> for Error {
    fn from(hiba: o8_xml::stocks::Hiba) -> Self {
        Error {
            code: hiba.kod,
            description: hiba.leiras
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl From<o8_xml::stocks::cikkek> for Products {
    fn from(c: o8_xml::stocks::cikkek) -> Self {
        Products {
            product: c.cikk
                        .into_iter()
                        .map(|p| p.into())
                        .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub stock: Option<f64>
}

impl From<o8_xml::stocks::cikk> for Product {
    fn from(c: o8_xml::stocks::cikk) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam, 
            stock: c.szabad
        }
    }
}