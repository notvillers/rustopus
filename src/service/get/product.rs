use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::global::errors;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
pub enum ProductEnvelope {
    Hu(o8_xml::products::Envelope),
    En(partner_xml::products::Envelope)
}


/// This function gets english products envelope from the given `CallData`
pub async fn get_products(call_data: o8_xml::defaults::CallData) -> ProductEnvelope {
    let request = o8_xml::products::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::products::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => return ProductEnvelope::Hu(envelope),
                false => return ProductEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            return ProductEnvelope::En(partner_xml::products::error_struct(rustopus_error.code, rustopus_error.description))
        }
    }
}