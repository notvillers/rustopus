use std::fmt;
use chrono::{DateTime, Utc};
use quick_xml;
use lazy_static::lazy_static;
use std::pin::Pin;
use futures::Future;

use crate::global::errors;
use crate::global::errors::RustopusError;
use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::service::log::elogger;
use crate::service::dates;

lazy_static! {
    static ref FIRST_DATE: DateTime<Utc> = dates::get_first_date();
}


pub enum ErrorType {
    DeError(quick_xml::DeError),
    Text(&'static str)
}

impl fmt::Display for ErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorType::Text(e) => write!(f, "{}", e),
            ErrorType::DeError(e) => write!(f, "{}", e)
        }
    }
}


fn error_logger(in_error: ErrorType, error: &RustopusError) {
    elogger(format!("{}: {} ({})", error.code, error.description, in_error.to_string()));
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ResponseGet {
    Products(partner_xml::products::Envelope),
    Stocks(partner_xml::stocks::Envelope),
    Prices(partner_xml::prices::Envelope),
    Images(partner_xml::images::Envelope),
    Barcodes(partner_xml::barcode::Envelope),
    Invoices(partner_xml::invoices::Envelope),
    Bulk(partner_xml::bulk::Envelope)
}

pub enum RequestGet {
    Products(o8_xml::defaults::CallData),
    Stocks(o8_xml::defaults::CallData),
    Prices(o8_xml::defaults::CallData),
    Images(o8_xml::defaults::CallData),
    Barcodes(o8_xml::defaults::CallData),
    Invoices(o8_xml::defaults::CallData),
    Bulk(o8_xml::defaults::CallData)
}

impl RequestGet {
    pub fn to_envelope(self) -> Pin<Box<dyn Future<Output = ResponseGet> + Send>> {
        Box::pin(async move {
            match self {
                RequestGet::Products(call_data) => ResponseGet::Products(get_products(call_data).await),
                RequestGet::Stocks(call_data) => ResponseGet::Stocks(get_stocks(call_data).await),
                RequestGet::Prices(call_data) => ResponseGet::Prices(get_prices(call_data).await),
                RequestGet::Images(call_data) => ResponseGet::Images(get_images(call_data).await),
                RequestGet::Barcodes(call_data) => ResponseGet::Barcodes(get_barcode(call_data).await),
                RequestGet::Invoices(call_data) => ResponseGet::Invoices(get_invoices(call_data).await),
                RequestGet::Bulk(call_data) => ResponseGet::Bulk(get_bulk(call_data).await)
            }
        })
    }


    pub fn to_xml(self) -> Pin<Box<dyn Future<Output=String> + Send>> {
        Box::pin(async move {
            let envelope = self.to_envelope().await;
            to_xml_string(&envelope)
        })
    }
}


fn to_xml_string<T: serde::Serialize>(val: &T) -> String {
    match quick_xml::se::to_string(val) {
        Ok(val) => val,
        Err(de_error) => {
            elogger(format!("{}: {} ({})", errors::GLOBAL_CONVERT_ERROR.code, errors::GLOBAL_CONVERT_ERROR.description, de_error));
            String::from("<Envelope></Envelope>")
        }
    }
}


async fn get_products(call_data: o8_xml::defaults::CallData) -> partner_xml::products::Envelope {
    let request = o8_xml::products::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::products::Envelope>(&response) {
        Ok(envelope) => envelope.to_en(),
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            partner_xml::products::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_stocks(call_data: o8_xml::defaults::CallData) -> partner_xml::stocks::Envelope {
    let request = o8_xml::stocks::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::stocks::Envelope>(&response) {
        Ok(envelope) => envelope.to_en(),
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            partner_xml::stocks::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_prices(call_data: o8_xml::defaults::CallData) -> partner_xml::prices::Envelope {
    match call_data.pid {
        Some(pid) => {
            let request = o8_xml::prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
            let response = soap::get_response(&call_data.url, request).await;
            match quick_xml::de::from_str::<o8_xml::prices::Envelope>(&response) {
                Ok(envelope) => envelope.to_en(),
                Err(error) => {
                    let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
                    error_logger(ErrorType::DeError(error), &rustopus_error);
                    partner_xml::prices::error_struct(rustopus_error.code, rustopus_error.description)
                }
            }
        }
        _ => {
            let rustopus_error = errors::GLOBAL_PID_ERROR;
            error_logger(ErrorType::Text("PID missing"), &rustopus_error);
            partner_xml::prices::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_images(call_data: o8_xml::defaults::CallData) -> partner_xml::images::Envelope {
    let request = o8_xml::images::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::images::Envelope>(&response) {
        Ok(envelope) => envelope.to_en(),
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            partner_xml::images::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_barcode(call_data: o8_xml::defaults::CallData) -> partner_xml::barcode::Envelope {
    let request = o8_xml::barcode::get_request_string(&call_data.xmlns, &call_data.from_date.unwrap_or(*FIRST_DATE), &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::barcode::Envelope>(&response) {
        Ok(envelope) => envelope.to_en(),
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            partner_xml::barcode::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_invoices(call_data: o8_xml::defaults::CallData) -> partner_xml::invoices::Envelope {
    let request = o8_xml::invoices::get_request_string_opt(&call_data.xmlns, &call_data.pid, &call_data.type_mod, &call_data.from_date, &call_data.to_date, &call_data.unpaid, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    match quick_xml::de::from_str::<o8_xml::invoices::Envelope>(&response) {
        Ok(envelope) => envelope.to_en(),
        Err(error) => {
            let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(error), &rustopus_error);
            partner_xml::invoices::error_struct(rustopus_error.code, rustopus_error.description)
        }
    }
}


async fn get_bulk(call_data: o8_xml::defaults::CallData) -> partner_xml::bulk::Envelope {
    let products = match RequestGet::Products(call_data.clone()).to_envelope().await {
        ResponseGet::Products(envelope) if envelope.body.response.result.answer.error.is_none() => envelope,
        _ => partner_xml::products::error_struct(errors::BULK_GET_PRODUCTS_ERROR.code, errors::BULK_GET_PRODUCTS_ERROR.description)
    };

    if let Some(error) = products.body.response.result.answer.error {
        let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
        error_logger(ErrorType::Text("Can not get products"), &rustopus_error);
        return partner_xml::bulk::error_struct(vec![rustopus_error.into(), error])
    }

    let stocks = match RequestGet::Stocks(call_data.clone()).to_envelope().await {
        ResponseGet::Stocks(envelope) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::stocks::error_struct(errors::BULK_GET_STOCKS_ERROR.code, errors::BULK_GET_STOCKS_ERROR.description))
    };

    let prices = match RequestGet::Prices(call_data.clone()).to_envelope().await {
        ResponseGet::Prices(envelope) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::prices::error_struct(errors::BULK_GET_PRICES_ERROR.code, errors::BULK_GET_PRICES_ERROR.description))
    };

    let images = match RequestGet::Images(call_data.clone()).to_envelope().await {
        ResponseGet::Images(envelope) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::images::error_struct(errors::BULK_GET_IMAGES_ERROR.code, errors::BULK_GET_IMAGES_ERROR.description))
    };

    let barcodes = match RequestGet::Barcodes(call_data).to_envelope().await {
        ResponseGet::Barcodes(envelope) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::barcode::error_struct(errors::BULK_GET_BARCODES_ERROR.code, errors::BULK_GET_BARCODES_ERROR.description))
    };

    (products, prices, stocks, images, barcodes).into()
}
