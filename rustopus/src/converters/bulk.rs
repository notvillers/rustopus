use chrono::{DateTime, Utc};
use quick_xml;
use crate::converters;
use crate::o8_xml;
use crate::partner_xml;
use crate::service::log::logger;

pub async fn get_bulk(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>, pid: &i64) -> String {
    let products = get_products(url, xmlns, authcode, web_update).await;
    let products_env: o8_xml::products::Envelope = match products {
        Ok(products) => {
            products
        },
        Err(e) => {
            logger(format!("Bulk get product error {}", e));
            return format!("<Envelope>{}</Envelope>", e)
        }
    };
    let stocks = get_stocks(url, xmlns, authcode, web_update).await;
    let stocks_env: o8_xml::stocks::Envelope = match stocks {
        Ok(stocks) => {
            stocks
        },
        Err(e) => {
            logger(format!("Bulk get stocks error: {}", e));
            return format!("<Envelope>{}</Envelope>", e)
        }
    };
    let prices = get_prices(url, xmlns, pid, authcode).await;
    let prices_env: o8_xml::prices::Envelope = match prices {
        Ok(prices) => {
            prices
        }
        Err(e) => {
            logger(format!("Bulk get prices error: {}", e));
            return format!("<Envelope>{}</Envelope>", e)
        }
    };

    let bulk_env = create_envelope(products_env, prices_env, stocks_env);
    create_xml(bulk_env)
}


fn create_xml(envelope: partner_xml::bulk::Envelope) -> String {
    match quick_xml::se::to_string(&envelope) {
        Ok(xml) => {
            xml
        }
        Err(e) => {
            logger(format!("XML creating error {}", e));
            format!("<Envelope>{}</Envelope>", e)
        }
    }
}


async fn get_products(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    let products_response = converters::products::get_products_xml(url, xmlns, authcode, web_update).await;
    converters::products::get_products_envelope(&products_response)
}


async fn get_stocks(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> Result<o8_xml::stocks::Envelope, quick_xml::DeError> {
    let stocks_response = converters::stocks::get_stocks_xml(url, xmlns, authcode, web_update).await;
    converters::stocks::get_stocks_envelope(&stocks_response)
}


async fn get_prices(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> Result<o8_xml::prices::Envelope, quick_xml::DeError> {
    let prices_response = converters::prices::get_prices_xml(url, xmlns, pid, authcode).await;
    converters::prices::get_prices_envelope(&prices_response)
}


fn create_envelope(products: o8_xml::products::Envelope, prices: o8_xml::prices::Envelope, stocks: o8_xml::stocks::Envelope) -> partner_xml::bulk::Envelope {
    partner_xml::bulk::Envelope {
        body: create_body(products, prices, stocks)
    }
}


fn create_body(products: o8_xml::products::Envelope, prices: o8_xml::prices::Envelope, stocks: o8_xml::stocks::Envelope) -> partner_xml::bulk::Body {
    partner_xml::bulk::Body {
        response: create_response(products, prices, stocks)
    }
}


fn create_response(products: o8_xml::products::Envelope, prices: o8_xml::prices::Envelope, stocks: o8_xml::stocks::Envelope) -> partner_xml::bulk::Response {
    partner_xml::bulk::Response {
        result: create_result(products, prices, stocks)
    }
}


fn create_result(products: o8_xml::products::Envelope, prices: o8_xml::prices::Envelope, stocks: o8_xml::stocks::Envelope) -> partner_xml::bulk::Result {
    partner_xml::bulk::Result {
        answer: create_answer(products, prices, stocks)
    }
}

fn create_answer(products: o8_xml::products::Envelope, prices: o8_xml::prices::Envelope, stocks: o8_xml::stocks::Envelope) -> partner_xml::bulk::Answer {
    let mut errors: Vec<partner_xml::bulk::Error> = Vec::new();
    match products.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.hiba {
        Some(e) => {
            let error: partner_xml::bulk::Error = e.into();
            errors.push(error);
        }
        _ => {}
    }
    match prices.Body.GetArlistaAuthResponse.GetArlistaAuthResult.valasz.hiba {
        Some(e) => {
            let error: partner_xml::bulk::Error = e.into();
            errors.push(error);
        }
        _ => {}
    }
    match stocks.Body.GetCikkekKeszletValtozasAuthResponse.GetCikkekKeszletValtozasAuthResult.valasz.hiba {
        Some(e) => {
            let error: partner_xml::bulk::Error = e.into();
            errors.push(error);
        }
        _ => {}
    }

    partner_xml::bulk::Answer {
        version: "1.0".to_string(),
        products: partner_xml::bulk::Products {
            product: create_products(
                &products.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.cikk,
                &prices.Body.GetArlistaAuthResponse.GetArlistaAuthResult.valasz.arak.ar,
                &stocks.Body.GetCikkekKeszletValtozasAuthResponse.GetCikkekKeszletValtozasAuthResult.valasz.cikkek.cikk)
        },
        error: errors
    }

}


fn create_products(products: &Vec<o8_xml::products::Cikk>, prices: &Vec<o8_xml::prices::ar>, stocks: &Vec<o8_xml::stocks::cikk>) -> Vec<partner_xml::bulk::Product> {
    let mut bulk_products: Vec<partner_xml::bulk::Product> = Vec::new();
    for product in products {
        let price = prices.iter().find(|price| price.cikkid == product.cikkid);
        let stock = stocks.iter().find(|stock| stock.cikkid == product.cikkid);
        bulk_products.push(create_product(product, price, stock));
    }
    bulk_products
}


fn create_product(product: &o8_xml::products::Cikk, price: Option<&o8_xml::prices::ar>, stock: Option<&o8_xml::stocks::cikk>) -> partner_xml::bulk::Product {
    let product: partner_xml::bulk::Product = (product, price, stock).into();
    product
}