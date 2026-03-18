use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::global::errors;
use crate::service::get_data::{ErrorType, error_logger};

#[derive(serde::Serialize)]
pub enum PricesEnvelope {
    Hu(o8_xml::prices::Envelope),
    En(partner_xml::prices::Envelope)
}


/// This function gets english prices envelope from the given `CallData`
pub async fn get_prices(call_data: o8_xml::defaults::CallData) -> PricesEnvelope {
    if let Some(pid) = call_data.pid {
        let request = o8_xml::prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
        let response = soap::get_response(&call_data.url, request).await;
        match quick_xml::de::from_str::<o8_xml::prices::Envelope>(&response) {
            Ok(envelope) => {
                match call_data.is_hu() {
                    true => return PricesEnvelope::Hu(envelope),
                    _ => return PricesEnvelope::En(envelope.to_en())
                }
            },
            Err(error) => {
                let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
                error_logger(ErrorType::DeError(error), &rustopus_error);
                return PricesEnvelope::En(partner_xml::prices::error_struct(rustopus_error.code, rustopus_error.description))
            }
        };
    };
    let rustopus_error = errors::GLOBAL_PID_ERROR;
    error_logger(ErrorType::Text("get_prices - PID missing"), &rustopus_error);
    PricesEnvelope::En(partner_xml::prices::error_struct(rustopus_error.code, rustopus_error.description))
}
