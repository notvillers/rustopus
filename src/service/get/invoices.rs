use crate::o8_xml::{invoices as o8_invoices, defaults::CallData};
use crate::partner_xml::invoices as p_invoices;
use crate::service::soap::get_response;
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::get_data::{ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum InvoicesEnvelope {
    Hu(o8_invoices::Envelope),
    En(p_invoices::Envelope)
}


/// This function gets english invoices envelope from the given `CallData`
pub async fn get_invoices(call_data: CallData) -> InvoicesEnvelope {
    let request = o8_invoices::get_request_string_opt(&call_data.xmlns, &call_data.pid, &call_data.type_mod, &call_data.from_date, &call_data.to_date, &call_data.unpaid, &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_invoices::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => InvoicesEnvelope::Hu(envelope),
                _ => InvoicesEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            InvoicesEnvelope::En(p_invoices::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description))
        }
    }
}
