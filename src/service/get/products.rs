use crate::forms::r#in::xml::{products as o8_products, defaults::CallData};
use crate::partner_xml::products as p_products;
use crate::forms::out::csv::products as csv_products;
use crate::service::soap::get_response;
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::get_data::{FIRST_DATE, ErrorType, error_logger};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ProductsEnvelope {
    Hu(o8_products::Envelope),
    En(p_products::Envelope)
}


pub enum ProductsXML {
    Hu(o8_products::Envelope),
    En(p_products::Envelope)
}


pub enum ProductsData {
    XML(ProductsXML),
    CSV(csv_products::Products)
}


/// This function gets english products envelope from the given `CallData`
pub async fn get_products(call_data: CallData) -> ProductsEnvelope {
    let request = o8_products::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_products::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.is_hu() {
                true => ProductsEnvelope::Hu(envelope),
                _ => ProductsEnvelope::En(envelope.to_en())
            }
        },
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            ProductsEnvelope::En(p_products::error_struct(rustopus_error.code, rustopus_error.description))
        }
    }
}
