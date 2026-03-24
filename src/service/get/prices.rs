use crate::forms::{
    r#in::xml::{
        prices as o8_prices,
        defaults::CallData
    },
    out::{
        xml::prices as p_prices,
        csv::prices as csv_prices
    }
};
use crate::service::soap::get_response;
use crate::global::errors::{GLOBAL_GET_DATA_ERROR,GLOBAL_PID_ERROR};
use crate::service::get_data::{
    ErrorType,
    error_logger, to_xml_string
};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum PricesXML {
    Hu(o8_prices::Envelope),
    En(p_prices::Envelope)
}

impl PricesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum PricesCSV {
    En(csv_prices::Prices)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum PricesData {
    XML(PricesXML),
    CSV(PricesCSV)
}


/// This function gets english prices envelope from the given `CallData`
pub async fn get_prices(call_data: CallData) -> PricesData {
    if let Some(pid) = call_data.pid {
        let request = o8_prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
        let response = get_response(&call_data.url, request).await;
        return match quick_xml::de::from_str::<o8_prices::Envelope>(&response) {
            Ok(envelope) => {
                match call_data.clone().is_csv() {
                    true => return PricesData::CSV(PricesCSV::En(envelope.into())),
                    _ => {}
                }
                match call_data.is_hu() {
                    true => PricesData::XML(PricesXML::Hu(envelope)),
                    _ => PricesData::XML(PricesXML::En(envelope.to_en()))
                }
            },
            Err(error) => {
                error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
                PricesData::XML(PricesXML::En(p_prices::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
            }
        };
    }
    error_logger(ErrorType::Text("get_prices - PID missing"), &GLOBAL_PID_ERROR);
    PricesData::XML(PricesXML::En(p_prices::error_struct(GLOBAL_PID_ERROR.code, GLOBAL_PID_ERROR.description)))
}
