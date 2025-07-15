// Barcodes english struct(s) for XML(s) got from the Octopus call

use serde::Serialize;
use quick_xml;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::barcode::Envelope> for Envelope {
    fn from(v: o8_xml::barcode::Envelope) -> Self {
        Envelope {
            body: v.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetProductBarcodesResponse
}

impl From<o8_xml::barcode::Body> for Body {
    fn from(v: o8_xml::barcode::Body) -> Self {
        Body {
            response: v.get_vonalkodok_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductBarcodesResponse {
    pub result: GetProductBarcodesResult
}

impl From<o8_xml::barcode::GetVonalkodokAuthResponse> for GetProductBarcodesResponse {
    fn from(v: o8_xml::barcode::GetVonalkodokAuthResponse) -> Self {
        GetProductBarcodesResponse {
            result: v.get_vonalkodok_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductBarcodesResult {
    pub answer: Answer
}

impl From<o8_xml::barcode::GetVonalkodokAuthResult> for GetProductBarcodesResult {
    fn from(v: o8_xml::barcode::GetVonalkodokAuthResult) -> Self {
        GetProductBarcodesResult {
            answer: v.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub barcodes: Barcodes,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::barcode::Valasz> for Answer {
    fn from(v: o8_xml::barcode::Valasz) -> Self {
        Answer {
            version: v.verzio,
            barcodes: v.vonalkodok.into(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Barcodes {
    pub barcode: Vec<Barcode>
}

impl From<o8_xml::barcode::Vonalkodok> for Barcodes {
    fn from(v: o8_xml::barcode::Vonalkodok) -> Self {
        Barcodes {
            barcode: v.vonalkod
                .into_iter()
                .map(|b| b.into())
                .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Barcode {
    pub id: u64,
    pub no: String,
    pub ean: String,
    pub unit: String,
    pub main_ean: bool
}

impl From<o8_xml::barcode::Vonalkod> for Barcode {
    fn from(v: o8_xml::barcode::Vonalkod) -> Self {
        Barcode {
            id: v.cikkid,
            no: v.cikkszam,
            ean: v.vonalkod,
            unit: v.me,
            main_ean: if v.elsean == 1 { true } else { false }
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetProductBarcodesResponse {
                result: GetProductBarcodesResult {
                    answer: Answer {
                        version: "1.0".to_string(),
                        barcodes: Barcodes {
                            barcode: vec![]
                        },
                        error: Some(
                            partner_xml::defaults::Error::load(code, description)
                        )
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    match quick_xml::se::to_string(&error_struct(code, description)) {
        Ok(xml) => xml,
        _ => "<Envelope></Envelope>".to_string()
    }
}
