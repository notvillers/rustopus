use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

use quick_xml::de::from_str;
use quick_xml::se::to_string;

use crate::o8_xml::{self};
use crate::partner_xml;
use crate::partner_xml::products::Size;

pub fn get_first_date() -> DateTime<Utc> {
    let naive_datetime = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(2000, 1, 1).expect("Invalid date provided"), 
        chrono::NaiveTime::from_hms_opt(0, 0, 1).expect("Invalid time provided"));

    Utc.from_utc_datetime(&naive_datetime)
}


async fn get_response(url: &str, soap_request: String) -> String {
    let client = Client::new();
    match client
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send()
        .await
    {
        Ok(resp) => match resp.text().await {
            Ok(text) => {
                text
            }
            Err(_) => {
                "<Envelope></Envelope>".to_string()
            }
        },
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn get_products_xml(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetCikkekAuth xmlns="{}">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetCikkekAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


pub async fn get_products(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_products_xml(xmlns, &web_update, &authcode);

    let response_text = get_response(url, soap_request).await;

    let envelope = get_products_envelope(&response_text);

    match envelope {
        Ok(envelope) => {
            let eng_envelope = products_to_en_struct(envelope);
            let eng_xml = to_string(&eng_envelope);

            match eng_xml {
                Ok(eng_xml) => {
                    eng_xml
                }
                Err(_) => {
                    "<Envelope></Envelope>".to_string()
                }
            }
        }
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


pub fn get_products_envelope(response_text: &str) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    from_str(response_text)
}


pub fn products_to_en_struct(envelope: o8_xml::products::Envelope) -> partner_xml::products::Envelope {
    let mut eng_products: Vec<partner_xml::products::Product> = Vec::new();
    for c in envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.cikk {
        let eng_product = hun_to_en_product(c);
        eng_products.push(eng_product);
    }

    let verzio = envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.verzio;

    let hiba = envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.hiba;

    let eng_answer = partner_xml::products::Answer {
        version: verzio,
        products: eng_products,
        error: hiba.map(|h| h.into())
    };

    let eng_getproductsauthresult = partner_xml::products::GetProductsAuthResult {
        answer: eng_answer
    };

    let eng_getproductsauthresponse = partner_xml::products::GetProductsAuthResponse {
        result: eng_getproductsauthresult
    };

    let eng_body = partner_xml::products::Body {
        response: eng_getproductsauthresponse
    };

    let eng_envelope = partner_xml::products::Envelope {
        body: eng_body
    };

    eng_envelope


}


fn hun_to_en_product(product: o8_xml::products::Cikk) -> partner_xml::products::Product {
    let c = product;

    let c_meret = c.meret.unwrap();
    let eng_product = partner_xml::products::Product {
        id: c.cikkid,
        no: c.cikkszam,
        name: c.cikknev,
        unit: c.me,
        base_unit: c.alapme,
        base_unit_qty: c.alapmenny,
        brand: c.gyarto,
        category_code: c.cikkcsoportkod,
        category_name: c.cikkcsoportnev,
        description: c.leiras,
        weight: c.tomeg,
        size: Size {
            x: c_meret.xmeret,
            y: c_meret.ymeret,
            z: c_meret.zmeret
        },
        main_category_code: c.focsoportkod,
        main_category_name: c.focsoportnev,
        sell_unit: c.ertmenny,
        origin_country: c.szarmorszag
    };

    eng_product
}


fn get_stock_xml(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {

    format!(r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetCikkekKeszletValtozasAuth xmlns="{}">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetCikkekKeszletValtozasAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        xmlns,
        web_update.format("%Y-%m-%dT%H:%M:%S").to_string(),
        authcode
    )
}


pub async fn get_stock(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {

    let soap_request = get_stock_xml(xmlns, &web_update, authcode);

    let response_text = get_response(url, soap_request).await;

    response_text
}