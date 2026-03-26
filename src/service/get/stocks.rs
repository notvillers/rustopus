use crate::forms::{
    r#in::xml::{
        stocks as o8_stocks,
        defaults::CallData
    },
    out::{
        xml::stocks as p_stocks,
        csv::stocks as csv_stocks
    }
};
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::{
    soap::get_response,
    get_data::{
        FIRST_DATE, ErrorType,
        error_logger, to_xml_string
    }
};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum StocksXML {
    Hu(o8_stocks::Envelope),
    En(p_stocks::Envelope)
}

impl StocksXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum StocksCSV {
    En(csv_stocks::Products)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum StocksData {
    XML(StocksXML),
    CSV(StocksCSV)
}


/// This function gets english stocks envelope from the given `CallData`
pub async fn get_stocks(call_data: CallData) -> StocksData {
    let request = o8_stocks::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    return match quick_xml::de::from_str::<o8_stocks::Envelope>(&response) {
        Ok(envelope) => {
            if call_data.clone().is_csv() {
                return StocksData::CSV(StocksCSV::En(envelope.into()))
            }
            match call_data.is_hu() {
                true => StocksData::XML(StocksXML::Hu(envelope)),
                _ => StocksData::XML(StocksXML::En(envelope.into()))
            }
        },
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            StocksData::XML(StocksXML::En(p_stocks::error_struct(rustopus_error.code, rustopus_error.description)))
        }
    }
}
