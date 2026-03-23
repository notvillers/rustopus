use crate::forms::r#in::xml::{prices as o8_prices, defaults::CallData};
use crate::forms::out::xml::prices as p_prices;
use crate::service::soap::get_response;
use crate::global::errors::{GLOBAL_GET_DATA_ERROR, GLOBAL_PID_ERROR};
use crate::service::get_data::{ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum PricesEnvelope {
    Hu(o8_prices::Envelope),
    En(p_prices::Envelope)
}


/// This function gets english prices envelope from the given `CallData`
pub async fn get_prices(call_data: CallData) -> PricesEnvelope {
    if let Some(pid) = call_data.pid {
        let request = o8_prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
        let response = get_response(&call_data.url, request).await;
        match quick_xml::de::from_str::<o8_prices::Envelope>(&response) {
            Ok(envelope) => {
                match call_data.is_hu() {
                    true => PricesEnvelope::Hu(envelope),
                    _ => PricesEnvelope::En(envelope.to_en())
                }
            },
            Err(error) => {
                error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
                PricesEnvelope::En(p_prices::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description))
            }
        };
    };
    error_logger(ErrorType::Text("get_prices - PID missing"), &GLOBAL_PID_ERROR);
    PricesEnvelope::En(p_prices::error_struct(GLOBAL_PID_ERROR.code, GLOBAL_PID_ERROR.description))
}
