// Images GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            defaults::CallData,
            images as o8_images
        },
        out::{
            xml::images as p_images,
            csv::images as csv_images
        }
    },
    service::{
        soap::get_response,
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
    pub enum ImagesXML {
        Hu(o8_images::Envelope),
        En(p_images::Envelope)
    }
    
    pub enum ImagesCSV {
        En(csv_images::Products)
    }
    
    pub enum ImagesData {
        XML(ImagesXML),
        CSV(ImagesCSV),
        XLSX(ImagesCSV)
    }
}


impl ImagesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}




/// This function gets english images envelope from the given `CallData`
pub async fn get_images(call_data: CallData) -> ImagesData {
    let request = o8_images::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_images::Envelope>(&response) {
        Ok(envelope) => {
            match get_return_type(call_data) {
                RT::Xlsx => ImagesData::XLSX(ImagesCSV::En(envelope.into())),
                RT::Csv => ImagesData::CSV(ImagesCSV::En(envelope.into())),
                RT::XmlHu => ImagesData::XML(ImagesXML::Hu(envelope)),
                _ => ImagesData::XML(ImagesXML::En(envelope.into()))
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            ImagesData::XML(ImagesXML::En(p_images::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
        }
    }
}
