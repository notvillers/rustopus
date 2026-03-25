use crate::forms::{
    r#in::xml::{
        products as o8_products,
        defaults::CallData
    },
    out::{
        xml::products as p_products,
        csv::products as csv_products
    }
};
use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::service::{
    soap::get_response,
    get_data::{
        FIRST_DATE, ErrorType,
        error_logger, to_xml_string
    }
};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ProductsXML {
    Hu(o8_products::Envelope),
    En(p_products::Envelope)
}

impl ProductsXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ProductsCSV {
    En(csv_products::Products)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ProductsData {
    XML(ProductsXML),
    CSV(ProductsCSV)
}


pub async fn get_products(call_data: CallData) -> ProductsData {
    let request = o8_products::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    return match quick_xml::de::from_str::<o8_products::Envelope>(&response) {
        Ok(envelope) => {
            match call_data.clone().is_csv() {
                true => return ProductsData::CSV(ProductsCSV::En(envelope.into())),
                _ => {}
            }
            match call_data.is_hu() {
                true => ProductsData::XML(ProductsXML::Hu(envelope)),
                _ => ProductsData::XML(ProductsXML::En(envelope.to_en()))
            }
        }
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            ProductsData::XML(ProductsXML::En(p_products::error_struct(rustopus_error.code, rustopus_error.description)))
        }
    }
}
