use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use quick_xml;
use lazy_static::lazy_static;
use crate::global::errors;
use crate::global::errors::RustopusError;
use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::service::log::logger;

use std::pin::Pin;
use futures::Future;

lazy_static! {
    static ref FIRST_DATE: DateTime<Utc> = get_first_date();
}

pub fn get_first_date() -> DateTime<Utc> {
    get_date_from_parts(None, None, None, None, None, None)
}


fn get_date_from_parts(year: Option<i32>, month: Option<u32>, day: Option<u32>, hour: Option<u32>, min: Option<u32>, sec: Option<u32>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(year.unwrap_or(1900), month.unwrap_or(1), day.unwrap_or(1)).unwrap_or(NaiveDate::MIN),
            chrono::NaiveTime::from_hms_opt(hour.unwrap_or(0), min.unwrap_or(0), sec.unwrap_or(1)).unwrap_or(NaiveTime::MIN)
        )
    )
}


pub enum ErrorType {
    DeError(quick_xml::DeError),
    Text(&'static str)
}


fn error_logger(in_error: ErrorType, error: &RustopusError) {
    let error_string  = match in_error {
        ErrorType::DeError(e) => e.to_string(),
        ErrorType::Text(e) => e.to_string()
    };
    logger(format!("{}: {} ({})", error.code, error.description, error_string));
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ResponseGet {
    Products(partner_xml::products::Envelope),
    Stocks(partner_xml::stocks::Envelope),
    Prices(partner_xml::prices::Envelope),
    Images(partner_xml::images::Envelope),
    Barcode(partner_xml::barcode::Envelope),
    Bulk(partner_xml::bulk::Envelope)
}

pub enum RequestGet {
    Products(o8_xml::defaults::CallData),
    Stocks(o8_xml::defaults::CallData),
    Prices(o8_xml::defaults::CallData),
    Images(o8_xml::defaults::CallData),
    Barcode(o8_xml::defaults::CallData),
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
                RequestGet::Barcode(call_data) => ResponseGet::Barcode(get_barcode(call_data).await),
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
            logger(format!("{}: {} ({})", errors::GLOBAL_CONVERT_ERROR.code, errors::GLOBAL_CONVERT_ERROR.description, de_error));
            "<Envelope></Envelope>".to_string()
        }
    }
}


async fn get_products(call_data: o8_xml::defaults::CallData) -> partner_xml::products::Envelope {
    let request = o8_xml::products::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::products::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(de_error), &error);
            return partner_xml::products::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_stocks(call_data: o8_xml::defaults::CallData) -> partner_xml::stocks::Envelope {
    let request = o8_xml::stocks::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::stocks::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(de_error), &error);
            return partner_xml::stocks::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_prices(call_data: o8_xml::defaults::CallData) -> partner_xml::prices::Envelope {
    match call_data.pid {
        Some(pid) => {
            let request = o8_xml::prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
            let response = soap::get_response(&call_data.url, request).await;
            let hu_envelope: o8_xml::prices::Envelope = match quick_xml::de::from_str(&response) {
                Ok(envelope) => envelope,
                Err(de_error) => {
                    let error = errors::GLOBAL_GET_DATA_ERROR;
                    error_logger(ErrorType::DeError(de_error), &error);
                    return partner_xml::prices::error_struct(error.code, error.description)
                }
            };
            hu_envelope.to_en()
        }
        _ => {
            let error = errors::GLOBAL_PID_ERROR;
            error_logger(ErrorType::Text("PID missing"), &error);
            partner_xml::prices::error_struct(errors::GLOBAL_PID_ERROR.code, errors::GLOBAL_PID_ERROR.description)
        }
    }
}


async fn get_images(call_data: o8_xml::defaults::CallData) -> partner_xml::images::Envelope {
    let request = o8_xml::images::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::images::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(de_error), &error);
            return partner_xml::images::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_barcode(call_data: o8_xml::defaults::CallData) -> partner_xml::barcode::Envelope {
    let request = o8_xml::barcode::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::barcode::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            error_logger(ErrorType::DeError(de_error), &error);
            return partner_xml::barcode::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_bulk(call_data: o8_xml::defaults::CallData) -> partner_xml::bulk::Envelope {
    let products = match RequestGet::Products(call_data.clone()).to_envelope().await {
        ResponseGet::Products(envelope) if envelope.body.response.result.answer.error.is_none() => envelope,
        _ => partner_xml::products::error_struct(errors::BULK_GET_PRODUCTS_ERROR.code, errors::BULK_GET_PRODUCTS_ERROR.description)
    };

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

    let barcodes = match RequestGet::Barcode(call_data).to_envelope().await {
        ResponseGet::Barcode(envelope) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::barcode::error_struct(errors::BULK_GET_BARCODES_ERROR.code, errors::BULK_GET_BARCODES_ERROR.description))
    };

    if let Some(e) = products.body.response.result.answer.error {
        let error = errors::GLOBAL_GET_DATA_ERROR;
        error_logger(ErrorType::Text("Can not get products"), &error);
        return partner_xml::bulk::error_struct(vec![errors::GLOBAL_GET_DATA_ERROR.into(), e])
    }

    (products, prices, stocks, images, barcodes).into()
}
