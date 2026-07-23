// Products GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            products as o8_products,
            defaults::CallData
        },
        out::{
            xml::products as p_products,
            csv::products as csv_products
        }
    },
    service::{
        soap::get_response_shared,
        get_data::{
            FIRST_DATE, ErrorType,
            error_logger, to_xml_string
        },
        get::defaults::{
            ReturnType as RT,
            get_return_type
        }
    }
};

get_models! {
    pub enum ProductsXML {
        Hu(o8_products::Envelope),
        En(p_products::Envelope)
    }
    
    pub enum ProductsCSV {
        En(csv_products::Products)
    }
    
    pub enum ProductsData {
        XML(ProductsXML),
        CSV(ProductsCSV),
        XLSX(ProductsCSV)
    }
}


impl ProductsXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


pub async fn get_products(call_data: CallData) -> ProductsData {
    let request = o8_products::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response_shared(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_products::Envelope>(&response) {
        Ok(envelope) => {
            match get_return_type(call_data) {
                RT::Xlsx => ProductsData::XLSX(ProductsCSV::En(envelope.into())),
                RT::Csv => ProductsData::CSV(ProductsCSV::En(envelope.into())),
                RT::XmlHu => ProductsData::XML(ProductsXML::Hu(envelope)),
                _ => ProductsData::XML(ProductsXML::En(envelope.into()))
            }
        },
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            ProductsData::XML(ProductsXML::En(p_products::error_struct(rustopus_error.code, rustopus_error.description)))
        }
    }
}
