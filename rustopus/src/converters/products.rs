use chrono::{DateTime, Utc};

use crate::o8_xml;
use crate::partner_xml;
use crate::partner_xml::products::Size;
use crate::service::soap;
use quick_xml;


pub async fn get_products(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let hu_products_xml = get_products_xml(url, xmlns, authcode, web_update).await;
    let hu_envelope = get_products_envelope(&hu_products_xml);
    match hu_envelope {
        Ok(hu_envelope) => {
            convert_products_envelope_to_xml(hu_envelope)
        }
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn get_products_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
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


async fn get_products_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_products_request_string(xmlns, web_update, authcode);
    soap::get_response(url, soap_request).await
}


fn get_products_envelope(response_text: &str) -> Result<o8_xml::products::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


fn convert_products_envelope_to_xml(hu_envelope: o8_xml::products::Envelope) -> String {
    let en_envelope = convert_products_envelope(hu_envelope);
    let eng_xml = quick_xml::se::to_string(&en_envelope);

    match eng_xml {
        Ok(eng_xml) =>{
            eng_xml
        }
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn convert_products_envelope(hu_envelope: o8_xml::products::Envelope) -> partner_xml::products::Envelope {
    envelope_to_en(hu_envelope)
}


fn envelope_to_en(envelope: o8_xml::products::Envelope) -> partner_xml::products::Envelope {
    partner_xml::products::Envelope {
        body: body_to_en(envelope.Body)
    }
}


fn body_to_en(body: o8_xml::products::Body) -> partner_xml::products::Body {
    partner_xml::products::Body {
        response: response_to_en(body.GetCikkekAuthResponse)
    }
}


fn response_to_en(response: o8_xml::products::GetCikkekAuthResponse) -> partner_xml::products::GetProductsAuthResponse {
    partner_xml::products::GetProductsAuthResponse {
        result: result_to_en(response.GetCikkekAuthResult)
    }
}


fn result_to_en(result: o8_xml::products::GetCikkekAuthResult) -> partner_xml::products::GetProductsAuthResult {
    partner_xml::products::GetProductsAuthResult {
        answer: answer_to_en(result.valasz)
    }
}


fn answer_to_en(answer: o8_xml::products::valasz) -> partner_xml::products::Answer {
    partner_xml::products::Answer {
        version: answer.verzio,
        products: products_to_en(answer.cikk),
        error: answer.hiba.map(|h| h.into())
    }
}


fn products_to_en(prods: Vec<o8_xml::products::Cikk>) -> Vec<partner_xml::products::Product> {
    let mut eng_products: Vec<partner_xml::products::Product> = Vec::new();
    for prod in prods {
        eng_products.push(product_to_en(prod));
    }
    eng_products
}


fn product_to_en(prod: o8_xml::products::Cikk) -> partner_xml::products::Product {
    let prod_size = prod.meret.unwrap();
    partner_xml::products::Product {
        id: prod.cikkid,
        no: prod.cikkszam,
        name: prod.cikknev,
        unit: prod.me,
        base_unit: prod.alapme,
        base_unit_qty: prod.alapmenny,
        brand: prod.gyarto,
        category_code: prod.cikkcsoportkod,
        category_name: prod.cikkcsoportnev,
        description: prod.leiras,
        weight: prod.tomeg,
        size: Size {
            x: prod_size.xmeret,
            y: prod_size.ymeret,
            z: prod_size.zmeret
        },
        main_category_code: prod.focsoportkod,
        main_category_name: prod.focsoportnev,
        sell_unit: prod.ertmenny,
        origin_country: prod.szarmorszag
    }
}
