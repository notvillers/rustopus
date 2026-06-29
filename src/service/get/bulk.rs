// Bulk GET
use crate::{
    forms::{
        r#in::xml::defaults::CallData,
        out::{
            csv::bulk as csv_bulk,
            xml::{barcode, bulk, bulk_hu, images, prices, stocks}
        }
    }, global::errors, macros::get::get_models, service::{
        get::{
            barcodes::{BarcodesData, BarcodesXML},
            images::{ImagesData, ImagesXML}, prices::{PricesData, PricesXML}, products::{ProductsData, ProductsXML}, stocks::{StocksData, StocksXML}
        }, get_data::{
            ErrorType, RequestGet, ResponseGet, error_logger, to_xml_string
        }
    }
};

get_models! {
    pub enum BulkXML {
        En(bulk::Envelope),
        Hu(bulk_hu::Envelope)
    }
    
    pub enum BulkCSV {
        En(csv_bulk::Products)
    }
    
    pub enum BulkData {
        XML(BulkXML),
        CSV(BulkCSV)
    }
}


impl BulkXML {
    pub fn to_xml(&self) -> String {
        to_xml_string(self)
    }
}


/// This function gets english bulk envelope from the given `CallData`. It combines a lot of other requests.
pub async fn get_bulk(mut call_data: CallData) -> BulkData {
    // Capture the requested output format before wiping the flags for the sub-calls
    // (the inner product/price/stock/... calls must always run in English).
    let is_hu = call_data.is_hu();
    let is_csv = call_data.is_csv();
    call_data.language = None;
    call_data.data_type = None;

    // Handling products
    let ResponseGet::Products(ProductsData::XML(ProductsXML::En(products))) = RequestGet::Products(call_data.clone()).to_data().await else {
        let rustopus_error = errors::BULK_GET_PRODUCTS_ERROR;
        error_logger(ErrorType::Text("`ProductsData::XML(ProductsXML::En())` did not return!"), &rustopus_error);
        return BulkData::XML(BulkXML::En(bulk::error_struct(vec![rustopus_error.into()])))
    };

    if let Some(error) = products.body.response.result.answer.error {
        let rustopus_error = errors::GLOBAL_GET_DATA_ERROR;
        error_logger(ErrorType::Text("Can not get products"), &rustopus_error);
        return BulkData::XML(BulkXML::En(bulk::error_struct(vec![rustopus_error.into(), error])))
    };

    // Products succeeded, so the remaining independent calls can run concurrently.
    let (prices_response, stocks_response, images_response, barcodes_response) = futures::join!(
        RequestGet::Prices(call_data.clone()).to_data(),
        RequestGet::Stocks(call_data.clone()).to_data(),
        RequestGet::Images(call_data.clone()).to_data(),
        RequestGet::Barcodes(call_data.clone()).to_data()
    );

    let prices = match prices_response {
        ResponseGet::Prices(PricesData::XML(PricesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(prices::error_struct(errors::BULK_GET_PRICES_ERROR.code, errors::BULK_GET_PRICES_ERROR.description))
    };

    let stocks = match stocks_response {
        ResponseGet::Stocks(StocksData::XML(StocksXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(stocks::error_struct(errors::BULK_GET_STOCKS_ERROR.code, errors::BULK_GET_STOCKS_ERROR.description))
    };

    let images = match images_response {
        ResponseGet::Images(ImagesData::XML(ImagesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(images::error_struct(errors::BULK_GET_IMAGES_ERROR.code, errors::BULK_GET_IMAGES_ERROR.description))
    };

    let barcodes = match barcodes_response {
        ResponseGet::Barcodes(BarcodesData::XML(BarcodesXML::En(envelope))) if envelope.body.response.result.answer.error.is_none() => Some(envelope),
        _ => Some(barcode::error_struct(errors::BULK_GET_BARCODES_ERROR.code, errors::BULK_GET_BARCODES_ERROR.description))
    };

    let envelope: bulk::Envelope = (products, prices, stocks, images, barcodes).into();

    match (is_csv, is_hu) {
        (true, _) => BulkData::CSV(BulkCSV::En(envelope.into())),
        (false, true) => BulkData::XML(BulkXML::Hu(envelope.into())),
        _ => BulkData::XML(BulkXML::En(envelope))
    }
}
