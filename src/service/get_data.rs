use std::fmt;
use std::vec;
use chrono::{DateTime, Utc};
use quick_xml;
use lazy_static::lazy_static;
use std::pin::Pin;
use futures::Future;

use crate::global::errors::{self, RustopusError};
use crate::forms::r#in::xml::defaults::CallData;
use crate::partner_xml;
use crate::service::{log::elogger, dates};

use crate::service::get::{
    products::{ProductsEnvelope, get_products},
    stocks::{StocksEnvelope, get_stocks},
    prices::{PricesEnvelope, get_prices},
    images::{ImagesEnvelope, get_images},
    barcodes::{BarcodesEnvelope, get_barcode},
    invoices::{InvoicesEnvelope, get_invoices}
};

lazy_static! {
    pub static ref FIRST_DATE: DateTime<Utc> = dates::get_first_date();
}

/// `ErrorType` enum for `error_logger`
pub enum ErrorType {
    DeError(quick_xml::DeError),
    Text(&'static str)
}

impl fmt::Display for ErrorType {
    /// fmt display for `ErrorType` enum
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ErrorType::Text(e) => write!(f, "{}", e),
            ErrorType::DeError(e) => write!(f, "{}", e)
        }
    }
}


/// Logs special error with `ErrorType`` enum
pub fn error_logger(in_error: ErrorType, error: &RustopusError) {
    elogger(format!("{}: {} ({})", error.code, error.description, in_error.to_string()));
}


/// `ResponseGet` enum for easier response handle
#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ResponseGet {
    Products(ProductsEnvelope),
    Stocks(StocksEnvelope),
    Prices(PricesEnvelope),
    Images(ImagesEnvelope),
    Barcodes(BarcodesEnvelope),
    Invoices(InvoicesEnvelope),
    Bulk(partner_xml::bulk::Envelope)
}


/// `RequestGet` enum for easier request handle
pub enum RequestGet {
    Products(CallData),
    Stocks(CallData),
    Prices(CallData),
    Images(CallData),
    Barcodes(CallData),
    Invoices(CallData),
    Bulk(CallData)
}

impl RequestGet {
    /// This function converts the `RequestGet` enum to `ResponseGet`
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

    /// This function converts the `RequestGet` enum directly into xml string
    pub fn to_xml(self) -> Pin<Box<dyn Future<Output=String> + Send>> {
        Box::pin(async move {
            let envelope = self.to_envelope().await;
            to_xml_string(&envelope)
        })
    }
}


/// This function converts `T` into xml string, if possible, else `"<Envelope></Envelope>"` 
fn to_xml_string<T: serde::Serialize>(val: &T) -> String {
    match quick_xml::se::to_string(val) {
        Ok(val) => return val,
        Err(de_error) => elogger(format!("{}: {} ({})", errors::GLOBAL_CONVERT_ERROR.code, errors::GLOBAL_CONVERT_ERROR.description, de_error))
    }
    "<Envelope></Envelope>".into()
}


/// This function gets english bulk envelope from the given `CallData`. It combines a lot of other requests.
async fn get_bulk(mut call_data: CallData) -> partner_xml::bulk::Envelope {
    call_data.language = None;

    let ResponseGet::Products(ProductsEnvelope::En(products)) = RequestGet::Products(call_data.clone()).to_envelope().await else {
        let rustopus_error = errors::BULK_GET_PRODUCTS_ERROR;
        error_logger(ErrorType::Text("'En' did not return!"), &rustopus_error);
        return partner_xml::bulk::error_struct(vec![rustopus_error.into()])
    };

    if let Some(error) = products.body.response.result.answer.error {
        let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
        error_logger(ErrorType::Text("Can not get products"), &rustopus_error);
        return partner_xml::bulk::error_struct(vec![rustopus_error.into(), error])
    };

    let stocks = match RequestGet::Prices(call_data.clone()).to_envelope().await {
        ResponseGet::Stocks(StocksEnvelope::En(envelope)) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::stocks::error_struct(errors::BULK_GET_STOCKS_ERROR.code, errors::BULK_GET_STOCKS_ERROR.description))
    };

    let prices = match RequestGet::Prices(call_data.clone()).to_envelope().await {
        ResponseGet::Prices(PricesEnvelope::En(envelope)) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::prices::error_struct(errors::BULK_GET_PRICES_ERROR.code, errors::BULK_GET_PRICES_ERROR.description))
    };

    let images = match RequestGet::Images(call_data.clone()).to_envelope().await {
        ResponseGet::Images(ImagesEnvelope::En(envelope)) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::images::error_struct(errors::BULK_GET_IMAGES_ERROR.code, errors::BULK_GET_IMAGES_ERROR.description))
    };

    let barcodes = match RequestGet::Barcodes(call_data).to_envelope().await {
        ResponseGet::Barcodes(BarcodesEnvelope::En(envelope)) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(partner_xml::barcode::error_struct(errors::BULK_GET_BARCODES_ERROR.code, errors::BULK_GET_BARCODES_ERROR.description))
    };

    (products, prices, stocks, images, barcodes).into()
}
