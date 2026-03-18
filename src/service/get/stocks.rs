use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::global::errors;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
pub enum StocksEnvelope {
    Hu(o8_xml::stocks::Envelope),
    En(partner_xml::stocks::Envelope)
}


/// This function gets english stocks envelope from the given `CallData`
pub async fn get_stocks(call_data: o8_xml::defaults::CallData) -> StocksEnvelope {
    let request = o8_xml::stocks::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::stocks::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => return StocksEnvelope::Hu(envelope),
                _ => return StocksEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            StocksEnvelope::En(partner_xml::stocks::error_struct(rustopus_error.code, rustopus_error.description))
        }
    }
}
