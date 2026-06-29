// Stocks GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            stocks as o8_stocks,
            defaults::CallData
        },
        out::{
            xml::stocks as p_stocks,
            csv::stocks as csv_stocks
        }
    },
    service::{
        soap::get_response,
        get_data::{
            FIRST_DATE, ErrorType,
            error_logger, to_xml_string
        }
    }
};

get_models! {
    pub enum StocksXML {
        Hu(o8_stocks::Envelope),
        En(p_stocks::Envelope)
    }
    
    pub enum StocksCSV {
        En(csv_stocks::Products)
    }

    pub enum StocksData {
        XML(StocksXML),
        CSV(StocksCSV)
    }
}


impl StocksXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


/// This function gets english stocks envelope from the given `CallData`
pub async fn get_stocks(call_data: CallData) -> StocksData {
    let request = o8_stocks::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    return match quick_xml::de::from_str::<o8_stocks::Envelope>(&response) {
        Ok(envelope) => {
            match (call_data.is_csv(), call_data.is_hu()) {
                (true, _) => StocksData::CSV(StocksCSV::En(envelope.into())),
                (_, true) => StocksData::XML(StocksXML::Hu(envelope)),
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
