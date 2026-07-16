// Mat GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            mat as o8_mat,
            defaults::CallData
        },
        out::{
            xml::mat as p_mat,
            csv::mat as csv_mat
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
    pub enum MatXML {
        Hu(o8_mat::Envelope),
        En(p_mat::Envelope)
    }

    pub enum MatCSV {
        En(csv_mat::Concepts)
    }

    pub enum MatData {
        XML(MatXML),
        CSV(MatCSV),
        XLSX(MatCSV)
    }
}


impl MatXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


pub async fn get_mat(call_data: CallData) -> MatData {
    let request = o8_mat::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response_shared(&call_data.url, request.clone()).await;
    return match quick_xml::de::from_str::<o8_mat::Envelope>(&response) {
        Ok(envelope) => {
            match get_return_type(call_data) {
                RT::Xlsx => MatData::XLSX(MatCSV::En(envelope.into())),
                RT::Csv => MatData::CSV(MatCSV::En(envelope.into())),
                RT::XmlHu => MatData::XML(MatXML::Hu(envelope)),
                _ => MatData::XML(MatXML::En(envelope.into()))
            }
        },
        Err(error) => {
            let rustopus_error = GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            MatData::XML(MatXML::En(p_mat::error_sturuct(rustopus_error.code, rustopus_error.description)))
        }
    }
}
