// Barcodes english struct(s) for XML(s) got from the Octopus call
use quick_xml;

use crate::{
    macros::out::OutModelDeriveOnly,
    forms::{
        r#in::xml::barcode as o8_barcode,
        out::xml::defaults as p_defaults
    }
};


OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }
    
    pub struct Body {
        pub response: GetProductBarcodesResponse
    }

    pub struct GetProductBarcodesResponse {
        pub result: GetProductBarcodesResult
    }
    
    pub struct GetProductBarcodesResult {
        pub answer: Answer
    }

    pub struct Answer {
        pub version: String,
        pub barcodes: Barcodes,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }

    pub struct Barcodes {
        pub barcode: Vec<Barcode>
    }

    pub struct Barcode {
        pub id: u64,
        pub no: String,
        pub ean: String,
        pub unit: String,
        pub main_ean: bool
    }
}


impl From<o8_barcode::Envelope> for Envelope {
    fn from(v: o8_barcode::Envelope) -> Self {
        Self {
            body: v.body.into()
        }
    }
}


impl From<o8_barcode::Body> for Body {
    fn from(v: o8_barcode::Body) -> Self {
        Self {
            response: v.get_vonalkodok_auth_response.into()
        }
    }
}


impl From<o8_barcode::GetVonalkodokAuthResponse> for GetProductBarcodesResponse {
    fn from(v: o8_barcode::GetVonalkodokAuthResponse) -> Self {
        Self {
            result: v.get_vonalkodok_auth_result.into()
        }
    }
}


impl From<o8_barcode::GetVonalkodokAuthResult> for GetProductBarcodesResult {
    fn from(v: o8_barcode::GetVonalkodokAuthResult) -> Self {
        Self {
            answer: v.valasz.into()
        }
    }
}


impl From<o8_barcode::Valasz> for Answer {
    fn from(v: o8_barcode::Valasz) -> Self {
        Self {
            version: v.verzio,
            barcodes: v.vonalkodok.into(),
            error: v.hiba.map(|x| x.into())
        }
    }
}


impl From<o8_barcode::Vonalkodok> for Barcodes {
    fn from(v: o8_barcode::Vonalkodok) -> Self {
        Self {
            barcode: v.vonalkod
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}


impl From<o8_barcode::Vonalkod> for Barcode {
    fn from(v: o8_barcode::Vonalkod) -> Self {
        Self {
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
                        version: "1.0".into(),
                        barcodes: Barcodes {
                            barcode: vec![]
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
