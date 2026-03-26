use std::fmt;
use chrono::{DateTime, Utc};
use quick_xml;
use lazy_static::lazy_static;
use std::pin::Pin;
use futures::Future;
use crate::global::errors::{self, RustopusError};
use crate::forms::r#in::xml::defaults::CallData;
use crate::service::{
    log::elogger,
    dates,
    get::{
        products::{ProductsData, get_products},
        stocks::{StocksData, get_stocks},
        prices::{PricesData, get_prices},
        images::{ImagesData, get_images},
        barcodes::{BarcodesData, get_barcode},
        invoices::{InvoicesEnvelope, get_invoices},
        bulk::{BulkData, get_bulk}
    }
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
    Products(ProductsData),
    Stocks(StocksData),
    Prices(PricesData),
    Images(ImagesData),
    Barcodes(BarcodesData),
    Invoices(InvoicesEnvelope),
    Bulk(BulkData)
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
    pub fn to_data(self) -> Pin<Box<dyn Future<Output = ResponseGet> + Send>> {
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
            let envelope = self.to_data().await;
            to_xml_string(&envelope)
        })
    }
}


/// This function converts `T` into xml string, if possible, else `"<Envelope></Envelope>"` 
pub fn to_xml_string<T: serde::Serialize>(val: &T) -> String {
    match quick_xml::se::to_string(val) {
        Ok(val) => return val,
        Err(de_error) => elogger(format!("{}: {} ({})", errors::GLOBAL_CONVERT_ERROR.code, errors::GLOBAL_CONVERT_ERROR.description, de_error))
    }
    "<Envelope></Envelope>".into()
}
