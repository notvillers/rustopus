// Stocks GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            defaults::CallData,
            stocks as o8_stocks
        }, 
        out::{
            csv::stocks as csv_stocks,
            xml::stocks as p_stocks
        }
    },
    service::{
        soap::get_response_shared,
        get_data::{
            ErrorType, FIRST_DATE,
            error_logger, to_xml_string
        },
        get::defaults::{
            ReturnType as RT,
            check_return_type
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
        Xml(StocksXML),
        Csv(StocksCSV),
        Xlsx(StocksCSV)
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
    let response = get_response_shared(&call_data.url, request).await;
    // Resolved before the envelope is inspected, because `get_return_type`
    // consumes `call_data` and both branches below need the answer.
    match quick_xml::de::from_str::<o8_stocks::Envelope>(&response) {
        Ok(envelope) => {
            let error = envelope.body.get_cikkek_keszlet_valtozas_auth_response.get_cikkek_keszlet_valtozas_auth_result.valasz.hiba.clone();
            let return_type = check_return_type(call_data, error, "stocks");

            match return_type {
                RT::Xlsx => StocksData::Xlsx(StocksCSV::En(envelope.into())),
                RT::Csv => StocksData::Csv(StocksCSV::En(envelope.into())),
                RT::XmlHu => StocksData::Xml(StocksXML::Hu(envelope)),
                _ => StocksData::Xml(StocksXML::En(envelope.into()))
            }
        },
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            StocksData::Xml(StocksXML::En(p_stocks::error_struct(rustopus_error.code, rustopus_error.description)))
        }
    }
}