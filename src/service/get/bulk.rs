use crate::forms::{
    r#in::xml::defaults::CallData,
    out::xml::{prices, stocks, images, barcode, bulk}
};
use crate::global::errors;
use crate::service::{
    get_data::{
        RequestGet, ResponseGet, ErrorType,
        error_logger
    },
    get::{
        products::{ProductsData, ProductsXML},
        prices::{PricesData, PricesXML},
        stocks::{StocksData, StocksXML},
        images::{ImagesData, ImagesXML},
        barcodes::{BarcodesData, BarcodesXML}
    }
};

/// This function gets english bulk envelope from the given `CallData`. It combines a lot of other requests.
pub async fn get_bulk(mut call_data: CallData) -> bulk::Envelope {
    call_data.language = None;
    call_data.data_type = None;

    // Handling products
    let ResponseGet::Products(ProductsData::XML(ProductsXML::En(products))) = RequestGet::Products(call_data.clone()).to_data().await else {
        let rustopus_error = errors::BULK_GET_PRODUCTS_ERROR;
        error_logger(ErrorType::Text("`ProductsData::XML(ProductsXML::En())` did not return!"), &rustopus_error);
        return bulk::error_struct(vec![rustopus_error.into()])
    };

    if let Some(error) = products.body.response.result.answer.error {
        let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
        error_logger(ErrorType::Text("Can not get products"), &rustopus_error);
        return bulk::error_struct(vec![rustopus_error.into(), error])
    };

    let prices = match RequestGet::Prices(call_data.clone()).to_data().await {
        ResponseGet::Prices(PricesData::XML(PricesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(prices::error_struct(errors::BULK_GET_PRICES_ERROR.code, errors::BULK_GET_PRICES_ERROR.description))
    };

    let stocks = match RequestGet::Stocks(call_data.clone()).to_data().await {
        ResponseGet::Stocks(StocksData::XML(StocksXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(stocks::error_struct(errors::BULK_GET_STOCKS_ERROR.code, errors::BULK_GET_STOCKS_ERROR.description))
    };

    let images = match RequestGet::Images(call_data.clone()).to_data().await {
        ResponseGet::Images(ImagesData::XML(ImagesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(images::error_struct(errors::BULK_GET_IMAGES_ERROR.code, errors::BULK_GET_IMAGES_ERROR.description))
    };

    let barcodes = match RequestGet::Barcodes(call_data).to_data().await {
        ResponseGet::Barcodes(BarcodesData::XML(BarcodesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(barcode::error_struct(errors::BULK_GET_BARCODES_ERROR.code, errors::BULK_GET_BARCODES_ERROR.description))
    };

    (products, prices, stocks, images, barcodes).into()
}
