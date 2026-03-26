use crate::global::errors::GLOBAL_GET_DATA_ERROR;
use crate::forms::{
    r#in::xml::{
        barcode as o8_barcode,
        defaults::CallData
    },
    out::{
        xml::barcode as p_barcode,
        csv::barcodes as csv_barcodes
    }
};
use crate::service::{
    soap::get_response,
    get_data::{
        FIRST_DATE, ErrorType,
        error_logger, to_xml_string
    }
};

#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BarcodesXML {
    Hu(o8_barcode::Envelope),
    En(p_barcode::Envelope)
}

impl BarcodesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BarcodesCSV {
    En(csv_barcodes::Barcodes)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum BarcodesData {
    XML(BarcodesXML),
    CSV(BarcodesCSV)
}


/// This function gets english barcodes envelope from the given `CallData`
pub async fn get_barcode(call_data: CallData) -> BarcodesData {
    let request = o8_barcode::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    return match quick_xml::de::from_str::<o8_barcode::Envelope>(&response) {
        Ok(envelope) => {
            if call_data.clone().is_csv() {
                return BarcodesData::CSV(BarcodesCSV::En(envelope.into()))
            }
            match call_data.is_hu() {
                true => BarcodesData::XML(BarcodesXML::Hu(envelope)),
                _ => BarcodesData::XML(BarcodesXML::En(envelope.into()))
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            BarcodesData::XML(BarcodesXML::En(p_barcode::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
        }
    }
}
