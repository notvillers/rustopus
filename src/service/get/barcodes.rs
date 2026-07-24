// Barcodes GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            barcode as o8_barcode,
            defaults::CallData
        },
        out::{
            xml::barcode as p_barcode,
            csv::barcodes as csv_barcodes
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
    pub enum BarcodesXML {
        Hu(o8_barcode::Envelope),
        En(p_barcode::Envelope)
    }

    pub enum BarcodesCSV {
        En(csv_barcodes::Barcodes)
    }

    pub enum BarcodesData {
        Xml(BarcodesXML),
        Csv(BarcodesCSV),
        Xlsx(BarcodesCSV)
    }
}


impl BarcodesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


/// This function gets english barcodes envelope from the given `CallData`
pub async fn get_barcode(call_data: CallData) -> BarcodesData {
    let request = o8_barcode::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response_shared(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_barcode::Envelope>(&response) {
        Ok(envelope) => {
            match get_return_type(call_data) {
                RT::Xlsx => BarcodesData::Xlsx(BarcodesCSV::En(envelope.into())),
                RT::Csv => BarcodesData::Csv(BarcodesCSV::En(envelope.into())),
                RT::XmlHu => BarcodesData::Xml(BarcodesXML::Hu(envelope)),
                _ => BarcodesData::Xml(BarcodesXML::En(envelope.into()))
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            BarcodesData::Xml(BarcodesXML::En(p_barcode::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
        }
    }
}
