use serde::Serialize;

use crate::o8_xml;
use crate::service::errors;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::prices::Envelope> for Envelope {
    fn from(envelope: o8_xml::prices::Envelope) -> Self {
        Envelope {
            body: envelope.Body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetPriceAuthResponse
}

impl From<o8_xml::prices::Body> for Body {
    fn from(body: o8_xml::prices::Body) -> Self {
        Body {
            response: body.GetArlistaAuthResponse.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResponse {
    pub result: GetPriceAuthResult
}

impl From<o8_xml::prices::GetArlistaAuthResponse> for GetPriceAuthResponse {
    fn from(response: o8_xml::prices::GetArlistaAuthResponse) -> Self {
        GetPriceAuthResponse {
            result: response.GetArlistaAuthResult.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResult {
    pub answer: Answer
}

impl From<o8_xml::prices::GetArlistaAuthResult> for GetPriceAuthResult {
    fn from(result: o8_xml::prices::GetArlistaAuthResult) -> Self {
        GetPriceAuthResult {
            answer: result.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub prices: Prices,
    pub error: Option<Error>
}

impl From<o8_xml::prices::valasz> for Answer {
    fn from(valasz: o8_xml::prices::valasz) -> Self {
        Answer {
            version: valasz.verzio,
            prices: valasz.arak.into(),
            error: valasz.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Error {
    pub code: u64,
    pub description: String
}

impl From<o8_xml::prices::Hiba> for Error {
    fn from(hiba: o8_xml::prices::Hiba) -> Self {
        Error {
            code: hiba.kod,
            description: errors::translate_error(&hiba.leiras)
        }
    }
}


#[derive(Serialize)]
pub struct Prices {
    pub price: Vec<Price>
}

impl From<o8_xml::prices::arak> for Prices {
    fn from(arak: o8_xml::prices::arak) -> Self {
        Prices {
            price: arak.ar
                .into_iter()
                .map(|p| p.into())
                .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Price {
    pub id: u64,
    pub no: String,
    pub list_price: Option<f64>,
    pub price: Option<f64>,
    pub sale_price: Option<f64>,
    pub currency: String
}

impl From<o8_xml::prices::ar> for Price {
    fn from(ar: o8_xml::prices::ar) -> Self {
        Price {
            id: ar.cikkid,
            no: ar.cikkszam,
            list_price: ar.listaar,
            price: ar.ar,
            sale_price: ar.akcios_ar,
            currency: ar.devizanem
        }
    }
}
