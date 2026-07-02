// Invoices GET
use crate::{
    macros::get::get_models,
    global::errors::GLOBAL_GET_DATA_ERROR,
    forms::{
        r#in::xml::{
            defaults::CallData,
            invoices as o8_invoices
        },
        out::{
            csv::invoices as csv_invoices,
            xml::invoices as p_invoices
        }
    },
    service::{
        soap::get_response,
        get_data::{
            ErrorType,
            error_logger, to_xml_string
        },
        get::defaults::{
            ReturnType as RT,
            get_return_type
        }
    }
};

get_models! {
    pub enum InvoicesXML {
        Hu(o8_invoices::Envelope),
        En(p_invoices::Envelope)
    }
    
    pub enum InvoicesCSV {
        En(csv_invoices::Products)
    }
    
    pub enum InvoicesData {
        XML(InvoicesXML),
        CSV(InvoicesCSV),
        XLSX(InvoicesCSV)
    }
}


impl InvoicesXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


/// This function gets english invoices envelope from the given `CallData`
pub async fn get_invoices(call_data: CallData) -> InvoicesData {
    let request = o8_invoices::get_request_string_opt(&call_data.xmlns, &call_data.pid, &call_data.type_mod, &call_data.from_date, &call_data.to_date, &call_data.unpaid, &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_invoices::Envelope>(&response) {
        Ok(envelope) => {
            match get_return_type(call_data) {
                RT::Xlsx => InvoicesData::XLSX(InvoicesCSV::En(envelope.into())),
                RT::Csv => InvoicesData::CSV(InvoicesCSV::En(envelope.into())),
                RT::XmlHu => InvoicesData::XML(InvoicesXML::Hu(envelope)),
                _ => InvoicesData::XML(InvoicesXML::En(envelope.into()))
            }
        },
        Err(error) => {
            error_logger(ErrorType::DeError(error), &GLOBAL_GET_DATA_ERROR);
            InvoicesData::XML(InvoicesXML::En(p_invoices::error_struct(GLOBAL_GET_DATA_ERROR.code, GLOBAL_GET_DATA_ERROR.description)))
        }
    }
}
