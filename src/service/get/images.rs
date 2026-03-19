use crate::o8_xml::{images as o8_images, defaults::CallData};
use crate::partner_xml::images as p_images;
use crate::service::soap::get_response;
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ImagesEnvelope {
    Hu(o8_images::Envelope),
    En(p_images::Envelope)
}


/// This function gets english images envelope from the given `CallData`
pub async fn get_images(call_data: CallData) -> ImagesEnvelope {
    let request = o8_images::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_images::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => ImagesEnvelope::Hu(envelope),
                _ => ImagesEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            ImagesEnvelope::En(p_images::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description))
        }
    }
}
