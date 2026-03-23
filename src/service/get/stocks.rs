use crate::forms::r#in::xml::{stocks as o8_stocks, defaults::CallData };
use crate::forms::out::xml::stocks as p_stocks;
use crate::service::soap::get_response;
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum StocksEnvelope {
    Hu(o8_stocks::Envelope),
    En(p_stocks::Envelope)
}


/// This function gets english stocks envelope from the given `CallData`
pub async fn get_stocks(call_data: CallData) -> StocksEnvelope {
    let request = o8_stocks::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_stocks::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => StocksEnvelope::Hu(envelope),
                _ => StocksEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            StocksEnvelope::En(p_stocks::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description))
        }
    }
}
