use crate::forms::{
    r#in::xml::{
        defaults::CallData, images as o8_images
    },
    out::{
        xml::images as p_images,
        csv::images as csv_images
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
pub enum ImagesXML {
    Hu(o8_images::Envelope),
    En(p_images::Envelope)
}

impl ImagesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ImagesCSV {
    En(csv_images::Products)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ImagesData {
    XML(ImagesXML),
    CSV(ImagesCSV)
}


/// This function gets english images envelope from the given `CallData`
pub async fn get_images(call_data: CallData) -> ImagesData {
    let request = o8_images::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    return match quick_xml::de::from_str::<o8_images::Envelope>(&response) {
        Ok(envelope) => {
            if call_data.clone().is_csv() {
                return ImagesData::CSV(ImagesCSV::En(envelope.into()))
            }
            match call_data.is_hu() {
                true => ImagesData::XML(ImagesXML::Hu(envelope)),
                _ => ImagesData::XML(ImagesXML::En(envelope.into()))
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            ImagesData::XML(ImagesXML::En(p_images::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
        }
    }
}
