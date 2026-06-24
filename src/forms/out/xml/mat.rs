/// Mat english struct(s) for XML(s) for from the Octopus call
use std::num::NonZeroU8;
use quick_xml;

use crate::{
    macros::out::OutModelDeriveOnly,
    forms::{
        r#in::xml::mat as o8_mat,
        out::xml::defaults as p_defaults
    }
};

OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }

    pub struct Body {
        pub response: GetMatAuthResponse
    }    

    pub struct GetMatAuthResponse {
        pub result: GetMatAuthResult
    }

    pub struct GetMatAuthResult {
        pub answer: Answer
    }

    pub struct Answer {
        pub version: String,
        pub attributes: Attributes,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }

    pub struct Attributes {
        pub attribute: Vec<Attribute>
    }

    #[derive(Clone)]
    pub struct Attribute {
        pub id: u64,
        pub code: Option<String>,
        pub name: Option<String>,
        pub product_id: Option<u64>,
        pub product_no: Option<String>,
        pub string_value: Option<String>,
        pub num_value: Option<f64>,
        pub order: Option<i64>,
        pub delstatus: Option<NonZeroU8>,
        pub filter: Option<NonZeroU8>,
        pub data_type: Option<NonZeroU8>,
        pub value_set: Option<i64>
    }
}


impl From<o8_mat::Envelope> for Envelope {
    fn from(e: o8_mat::Envelope) -> Self {
        Self {
            body: e.body.into()
        }
    }
}


impl From<o8_mat::Body> for Body {
    fn from(b: o8_mat::Body) -> Self {
        Self {
            response: b.get_matmodell_auth_response.into()
        }
    }
}


impl From<o8_mat::GetMatmodellAuthResponse> for GetMatAuthResponse {
    fn from(r: o8_mat::GetMatmodellAuthResponse) -> Self {
        Self {
            result: r.get_matmodell_auth_result.into()
        }
    }
}


impl From<o8_mat::GetMatmodellAuthResult> for GetMatAuthResult {
    fn from(r: o8_mat::GetMatmodellAuthResult) -> Self {
        Self {
            answer: r.valasz.into()
        }
    }
}


impl From<o8_mat::Valasz> for Answer {
    fn from(v: o8_mat::Valasz) -> Self {
        Self {
            version: v.verzio,
            attributes: v.tulajdonsagok.tulajdonsag.into_iter().collect::<Attributes>(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


impl FromIterator<o8_mat::Tulajdonsag> for Attributes {
    fn from_iter<I: IntoIterator<Item = o8_mat::Tulajdonsag>>(iter: I) -> Self {
        Self {
            attribute: iter.into_iter().map(|x| x.into()).collect()
        }
    }
}


impl From<o8_mat::Tulajdonsag> for Attribute {
    fn from(f: o8_mat::Tulajdonsag) -> Self {
        Self {
            id: f.azonosito,
            code: f.tulajdonsagkod,
            name: f.tulajdonsagnev,
            product_id: f.cikkid,
            product_no: f.cikkszam,
            string_value: f.szovegertek,
            num_value: f.szamertek,
            order: f.sorrend,
            delstatus: f.delstatus,
            filter: f.szures,
            data_type: f.adattipus,
            value_set: f.ertekkeszlet_id
        }
    }
}


pub fn error_sturuct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetMatAuthResponse {
                result: GetMatAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        attributes: Attributes{
                            attribute: vec![]
                        },
                        error: Some(p_defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_sturuct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
