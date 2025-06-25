use chrono::{DateTime, Utc};
use quick_xml;
use crate::converters;
use crate::o8_xml;
use crate::partner_xml;
use crate::service::log::logger;
use crate::global::errors;

/// Returns error string with auth. code
/// # Parameters
/// * error: `global::errors::RustopusError` 
/// * authcode: `&str`
/// # Returns
/// `String`
fn auth_err_str(error: errors::RustopusError, authcode: &str) -> String {
    format!("{}, {}: {}", authcode, error.code, error.description)
}


/// `async` Get the data into reformatted string
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `String`
pub async fn get_data(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>, pid: &i64) -> String {
    let products_env: o8_xml::products::Envelope = match get_products(url, xmlns, authcode, web_update).await {
        Ok(products) => products,
        Err(de_error) => return log_and_send_error_xml(de_error, errors::GLOBAL_GET_DATA_ERROR, Some(&auth_err_str(errors::BULK_GET_PRODUCTS_ERROR, authcode)))
    };
    let stocks_env: Option<o8_xml::stocks::Envelope> = match get_stocks(url, xmlns, authcode, web_update).await {
        Ok(stocks) => Some(stocks),
        Err(de_error) => {
            log_error(de_error, &errors::GLOBAL_GET_DATA_ERROR, Some(&auth_err_str(errors::BULK_GET_STOCKS_ERROR, authcode)));
            None
        }
    };
    let prices_env: Option<o8_xml::prices::Envelope> = match get_prices(url, xmlns, pid, authcode).await {
        Ok(prices) => Some(prices),
        Err(de_error) => {
            log_error(de_error, &errors::GLOBAL_GET_DATA_ERROR, Some(&auth_err_str(errors::BULK_GET_PRICES_ERROR, authcode)));
            None
        }
    };
    // TODO: Images
    let images_env: Option<o8_xml::images::Envelope> = match get_images(url, xmlns, web_update, authcode).await {
        Ok(images) => Some(images),
        Err(de_error) => {
            log_error(de_error, &errors::GLOBAL_GET_DATA_ERROR, Some(&auth_err_str(errors::BULK_GET_IMAGES_ERROR, authcode)));
            None
        }
    };

    create_xml(create_envelope(products_env, prices_env, stocks_env, images_env))
}


/// Creates xml from struct
/// # Parameters
/// * envelope: `partner_xml::bulk::Envelope`
/// # Returns
/// `String`
fn create_xml(envelope: partner_xml::bulk::Envelope) -> String {
    match quick_xml::se::to_string(&envelope) {
        Ok(xml) => xml,
        Err(de_error) => log_and_send_error_xml(de_error, errors::GLOBAL_CONVERT_ERROR, None)
    }
}


/// `async` Get products data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `Result<o8_xml::products::Envelope, quick_xml::DeError>`
async fn get_products(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    converters::products::get_envelope(&converters::products::get_xml(url, xmlns, authcode, web_update).await)
}


/// `async` Get stocks data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `Result<o8_xml::stocks::Envelope, quick_xml::DeError>`
async fn get_stocks(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> Result<o8_xml::stocks::Envelope, quick_xml::DeError> {
    converters::stocks::get_envelope(&converters::stocks::get_xml(url, xmlns, authcode, web_update).await)
}


/// `async` Get prices data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `Result<o8_xml::prices::Envelope, quick_xml::DeError>`
async fn get_prices(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> Result<o8_xml::prices::Envelope, quick_xml::DeError> {
    converters::prices::get_envelope(&converters::prices::get_xml(url, xmlns, pid, authcode).await)
}


/// `async` Get prices data
/// # Parameters
/// * url: `&str`
/// * xmlns: `&str`
/// * authcode: `&str`
/// * web_update `&DateTime<Utc>`
/// # Return
/// `Result<o8_xml::images::Envelope, quick_xml::DeError>`
async fn get_images(url: &str, xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> Result<o8_xml::images::Envelope, quick_xml::DeError> {
    converters::images::get_envelope(&converters::images::get_xml(url, xmlns, authcode, web_update).await)
}


/// Creates envelope from data
/// # Parameters
/// * products: `o8_xml::products::Envelope`
/// * prices: `Option<o8_xml::prices::Envelope>`
/// * stocks: `Option<o8_xml::stocks::Envelope>`
/// # Returns
/// `partner_xml::bulk::Envelope`
fn create_envelope(products: o8_xml::products::Envelope, prices: Option<o8_xml::prices::Envelope>, stocks: Option<o8_xml::stocks::Envelope>, images: Option<o8_xml::images::Envelope>) -> partner_xml::bulk::Envelope {
    partner_xml::bulk::Envelope {
        body: create_body(products, prices, stocks, images)
    }
}


/// Creates body from data
/// # Parameters
/// * products: `o8_xml::products::Envelope`
/// * prices: `Option<o8_xml::prices::Envelope>`
/// * stocks: `Option<o8_xml::stocks::Envelope>`
/// # Returns
/// `partner_xml::bulk::Body`
fn create_body(products: o8_xml::products::Envelope, prices: Option<o8_xml::prices::Envelope>, stocks: Option<o8_xml::stocks::Envelope>, images: Option<o8_xml::images::Envelope>) -> partner_xml::bulk::Body {
    partner_xml::bulk::Body {
        response: create_response(products, prices, stocks, images)
    }
}

/// Creates response from data
/// # Parameters
/// * products: `o8_xml::products::Envelope`
/// * prices: `Option<o8_xml::prices::Envelope>`
/// * stocks: `Option<o8_xml::stocks::Envelope>`
/// # Returns
/// `partner_xml::bulk::Response`
fn create_response(products: o8_xml::products::Envelope, prices: Option<o8_xml::prices::Envelope>, stocks: Option<o8_xml::stocks::Envelope>, images: Option<o8_xml::images::Envelope>) -> partner_xml::bulk::Response {
    partner_xml::bulk::Response {
        result: create_result(products, prices, stocks, images)
    }
}


/// Creates result from data
/// # Parameters
/// * products: `o8_xml::products::Envelope`
/// * prices: `Option<o8_xml::prices::Envelope>`
/// * stocks: `Option<o8_xml::stocks::Envelope>`
/// # Returns
/// `partner_xml::bulk::Result`
fn create_result(products: o8_xml::products::Envelope, prices: Option<o8_xml::prices::Envelope>, stocks: Option<o8_xml::stocks::Envelope>, images: Option<o8_xml::images::Envelope>) -> partner_xml::bulk::Result {
partner_xml::bulk::Result {
        answer: create_answer(products, prices, stocks, images)
    }
}


/// Creates answer from data
/// # Parameters
/// * products: `o8_xml::products::Envelope`
/// * prices: `Option<o8_xml::prices::Envelope>`
/// * stocks: `Option<o8_xml::stocks::Envelope>`
/// # Returns
/// `partner_xml::bulk::Answer`
fn create_answer(products: o8_xml::products::Envelope, prices: Option<o8_xml::prices::Envelope>, stocks: Option<o8_xml::stocks::Envelope>, images: Option<o8_xml::images::Envelope>) -> partner_xml::bulk::Answer {
    let mut errors: Vec<partner_xml::defaults::Error> = vec![];
    if let Some(e) = products.body.get_cikkek_auth_response.get_cikkek_auth_result.valasz.hiba {
        let error: partner_xml::defaults::Error = e.into();
        errors.push(error);
    }
    match &prices {
        Some(prices) => {
            if let Some(e) = &prices.body.get_arlista_auth_response.get_arlista_auth_result.valasz.hiba {
                let error: partner_xml::defaults::Error = e.into();
                errors.push(error);
            }
        }
        _ => {
            errors.push(
                partner_xml::defaults::Error {
                    code: errors::BULK_GET_PRICES_ERROR.code,
                    description: errors::BULK_GET_PRICES_ERROR.description.to_string()
                }
            );
        }
    }
    match &stocks {
        Some(stocks) => {
            if let Some(e) = &stocks.body.get_cikkek_keszlet_valtozas_auth_response.get_cikkek_keszlet_valtozas_auth_result.valasz.hiba {
                let error: partner_xml::defaults::Error = e.into();
                errors.push(error);
            }
        }
        _ => {
            errors.push(
                partner_xml::defaults::Error {
                    code: errors::BULK_GET_STOCKS_ERROR.code,
                    description: errors::BULK_GET_STOCKS_ERROR.description.to_string()
                }
            );
        }
    }
    match &images {
        Some(images) => {
            if let Some(e) = &images.body.get_cikk_kepek_auth_response.get_cikk_kepek_auth_result.valasz.hiba {
                let error: partner_xml::defaults::Error = e.into();
                errors.push(error);
            }
        },
        _ => {
            errors.push(
                partner_xml::defaults::Error {
                    code: errors::BULK_GET_IMAGES_ERROR.code,
                    description: errors::BULK_GET_IMAGES_ERROR.description.to_string()
                }
            );
        }
    }

    partner_xml::bulk::Answer {
        version: "1.0".to_string(),
        products: partner_xml::bulk::Products {
            product: create_products(
                &products.body.get_cikkek_auth_response.get_cikkek_auth_result.valasz.cikk,
                &match prices {
                    Some(prices) => prices.body.get_arlista_auth_response.get_arlista_auth_result.valasz.arak.ar,
                    _ => vec![]
                },
                &match stocks {
                    Some(stocks) => stocks.body.get_cikkek_keszlet_valtozas_auth_response.get_cikkek_keszlet_valtozas_auth_result.valasz.cikkek.cikk,
                    _ => vec![]
                },
                &match images {
                    Some(images) => images.body.get_cikk_kepek_auth_response.get_cikk_kepek_auth_result.valasz.cikk,
                    _ => vec![]
                }
            )
        },
        error: errors
    }
}


/// Creates products from data
/// # Parameters
/// * products: `&Vec<o8_xml::products::Cikk>`
/// * prices: `&Vec<o8_xml::prices::Ar>`
/// * stocks: `&Vec<o8_xml::stocks::Cikk>`
/// # Returns
/// `Vec<partner_xml::bulk::Product>`
fn create_products(products: &Vec<o8_xml::products::Cikk>, prices: &Vec<o8_xml::prices::Ar>, stocks: &Vec<o8_xml::stocks::Cikk>, images: &Vec<o8_xml::images::Cikk>) -> Vec<partner_xml::bulk::Product> {
    let mut bulk_products: Vec<partner_xml::bulk::Product> = vec![];
    for product in products {
        let price = prices.iter().find(|price| price.cikkid == product.cikkid);
        let stock = stocks.iter().find(|stock| stock.cikkid == product.cikkid);
        let image = images.iter().find(|image| image.cikkid == product.cikkid);
        bulk_products.push(create_product(product, price, stock, image));
    }
    bulk_products
}


/// Creates product from data
/// # Parameters
/// * products: `&o8_xml::products::Cike`
/// * prices: `Option<&o8_xml::prices::Ar>`
/// * stocks: `Option<&o8_xml::stocks::Cikk>`
/// # Returns
/// `partner_xml::bulk::Product`
fn create_product(product: &o8_xml::products::Cikk, price: Option<&o8_xml::prices::Ar>, stock: Option<&o8_xml::stocks::Cikk>, image: Option<&o8_xml::images::Cikk>) -> partner_xml::bulk::Product {
    let product: partner_xml::bulk::Product = (product, price, stock, image).into();
    product
}


/// Logs error
/// # Parameters
/// * de_error: `quick_xml::DeError`
/// * error: `global::errors::RustopusError`
/// * description_info: `Option<&str>`
fn log_error(de_error: quick_xml::DeError, error: &errors::RustopusError, description_info: Option<&str>) {
    let concat_description = match description_info {
        Some(info) => format!("{} - {}", error.description, info),
        _ => error.description.to_string()
    };
    logger(format!("{}: {} ({})", error.code, concat_description, de_error));
}


/// Logs error and send error struct xml
/// # Parameters
/// * de_error: `quick_xml::DeError`
/// * error: `global::errors:RustopusError`
/// # Returns
/// `String`
fn log_and_send_error_xml(de_error: quick_xml::DeError, error: errors::RustopusError, description_info: Option<&str>) -> String {
    log_error(de_error, &error, description_info);
    match description_info {
        Some(info) => return send_error_xml(error.code, &format!("{} - {}", error.description, info)),
        _ => return send_error_xml(error.code, error.description)
    };
}


/// Send error struct xml
/// # Parameters
/// * code: `u64`
/// * description: `&str`
/// # Returns
/// `String`
pub fn send_error_xml(code: u64, description: &str) -> String {
    let errors: Vec<partner_xml::defaults::Error> = vec![
        partner_xml::defaults::Error {
            code: code,
            description: description.to_string()
        }
    ];
    match quick_xml::se::to_string(&partner_xml::bulk::error_struct(errors)) {
        Ok(e_xml) => e_xml,
        Err(e) => {
            logger(format!("{}: {}", description, e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
