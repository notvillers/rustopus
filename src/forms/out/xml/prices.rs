/// Prices english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::{
        r#in::xml::prices as o8_prices,
        out::xml::defaults as p_defaults
    }
};

OutModelDeriveSerializeOnly! {
    pub struct Envelope {
        pub body: Body
    }
    
    pub struct Body {
        pub response: GetPriceAuthResponse
    }
    
    pub struct GetPriceAuthResponse {
        pub result: GetPriceAuthResult
    }
    
    pub struct Answer {
        pub version: String,
        pub prices: Prices,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }
    
    pub struct Prices {
        pub price: Vec<Price>
    }
    
    pub struct Price {
        pub id: u64,
        pub no: String,
        pub list_price: Option<f64>,
        pub price: Option<f64>,
        pub sale_price: Option<f64>,
        pub currency: String
    }
}


impl From<o8_prices::Envelope> for Envelope {
    fn from(envelope: o8_prices::Envelope) -> Self {
        Self {
            body: envelope.body.into()
        }
    }
}


impl From<o8_prices::Body> for Body {
    fn from(body: o8_prices::Body) -> Self {
        Self {
            response: body.get_arlista_auth_response.into()
        }
    }
}


impl From<o8_prices::GetArlistaAuthResponse> for GetPriceAuthResponse {
    fn from(response: o8_prices::GetArlistaAuthResponse) -> Self {
        Self {
            result: response.get_arlista_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetPriceAuthResult {
    pub answer: Answer
}


impl From<o8_prices::GetArlistaAuthResult> for GetPriceAuthResult {
    fn from(result: o8_prices::GetArlistaAuthResult) -> Self {
        Self {
            answer: result.valasz.into()
        }
    }
}


impl From<o8_prices::Valasz> for Answer {
    fn from(valasz: o8_prices::Valasz) -> Self {
        Self {
            version: valasz.verzio,
            prices: valasz.arak.into(),
            error: valasz.hiba.map(|e| e.into())
        }
    }
}


impl From<o8_prices::Arak> for Prices {
    fn from(arak: o8_prices::Arak) -> Self {
        Self {
            price: arak.ar
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}


impl From<o8_prices::Ar> for Price {
    fn from(ar: o8_prices::Ar) -> Self {
        Self {
            id: ar.cikkid,
            no: ar.cikkszam,
            list_price: ar.listaar,
            price: ar.ar,
            sale_price: ar.akcios_ar,
            currency: ar.devizanem
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope {
        body: Body {
            response: GetPriceAuthResponse {
                result: GetPriceAuthResult {
                    answer: Answer {
                        version: "1.0".into(),
                        prices: Prices {
                            price: vec![]
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


#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim from a production log: Octopus refusing a call because the
    /// partner exceeded its rate limit. Note there is **no** `<arak>` element —
    /// an error response carries the error and nothing else.
    const RATE_LIMITED: &str = r#"<?xml version="1.0" encoding="utf-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema"><soap:Body><GetArlistaAuthResponse xmlns="https://orink.hu/services/"><GetArlistaAuthResult><valasz verzio="1.0" xmlns=""><hiba><kod>3</kod><leiras>Túl sok kérés</leiras></hiba></valasz></GetArlistaAuthResult></GetArlistaAuthResponse></soap:Body></soap:Envelope>"#;

    #[test]
    fn an_error_only_response_parses_instead_of_failing_on_the_missing_data_element() {
        // `arak` used to be a required field, so this envelope died with
        // `missing field 'arak'` before `hiba` was ever read — and the caller got
        // a generic parse error rather than the reason Octopus gave.
        let envelope: o8_prices::Envelope = quick_xml::de::from_str(RATE_LIMITED)
            .expect("an error-only response must still deserialize");

        let answer = envelope.body.get_arlista_auth_response.get_arlista_auth_result.valasz;
        let error = answer.hiba.expect("the error must survive parsing");
        assert_eq!(error.kod, 3);
        assert_eq!(error.leiras, "Túl sok kérés");
        assert!(answer.arak.ar.is_empty(), "no prices came with the error");
    }

    #[test]
    fn the_hungarian_reason_reaches_the_caller_translated() {
        let envelope: o8_prices::Envelope = quick_xml::de::from_str(RATE_LIMITED).expect("parses");

        // The whole point of errors.json: an English-facing caller gets a
        // sentence they can act on, not Hungarian they cannot read.
        let translated: Envelope = envelope.into();
        let error = translated.body.response.result.answer.error.expect("error survives conversion");
        assert_eq!(error.code, 3);
        assert_eq!(error.description, "Request limit exceeded");
    }
}
