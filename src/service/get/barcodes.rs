use crate::o8_xml::{barcode as o8_barcode, defaults::CallData};
use crate::partner_xml::barcode as p_barcode;
use crate::service::soap::get_response;
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BarcodesEnvelope {
    Hu(o8_barcode::Envelope),
    En(p_barcode::Envelope)
}


/// This function gets english barcodes envelope from the given `CallData`
pub async fn get_barcode(call_data: CallData) -> BarcodesEnvelope {
    let request = o8_barcode::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_barcode::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => BarcodesEnvelope::Hu(envelope),
                _=> BarcodesEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            BarcodesEnvelope::En(p_barcode::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description))
        }
    }
}
