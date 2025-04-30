use crate::o8_xml;
use crate::partner_xml;
use quick_xml;
use chrono::{DateTime, Utc};
use crate::service::soap;


pub async fn get_stocks(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let hu_stocks_xml = get_stocks_xml(url, xmlns, authcode, web_update).await;
    let hu_envelope = get_stocks_envelope(&hu_stocks_xml);
    match hu_envelope {
        Ok(hu_envelope) => {
            convert_stocks_envelope_to_xml(hu_envelope)
        }
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn get_stocks_request_string(xmlns: &str, web_update: &DateTime<Utc>, authcode: &str) -> String {
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


async fn get_stocks_xml(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request = get_stocks_request_string(xmlns, web_update, authcode);
    soap::get_response(url, soap_request).await
}


fn get_stocks_envelope(response_text: &str) -> Result<o8_xml::stocks::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


fn convert_stocks_envelope_to_xml(hu_envelope: o8_xml::stocks::Envelope) -> String {
    let en_envelope = envelope_to_en(hu_envelope);
    let eng_xml = quick_xml::se::to_string(&en_envelope);

    match eng_xml {
        Ok(eng_xml) => {
            eng_xml
        }
        Err(_) => {
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn envelope_to_en(hu_envelope: o8_xml::stocks::Envelope) -> partner_xml::stocks::Envelope {
    partner_xml::stocks::Envelope {
        body: body_to_en(hu_envelope.Body)
    }
}


fn body_to_en(body: o8_xml::stocks::Body) -> partner_xml::stocks::Body {
    partner_xml::stocks::Body {
        response: response_to_en(body.GetCikkekKeszletValtozasAuthResponse)
    }
}


fn response_to_en(response: o8_xml::stocks::GetCikkekKeszletValtozasAuthResponse) -> partner_xml::stocks::GetStockChangeAuthResponse {
    partner_xml::stocks::GetStockChangeAuthResponse {
        result: result_to_en(response.GetCikkekKeszletValtozasAuthResult)
    }
}


fn result_to_en(result: o8_xml::stocks::GetCikkekKeszletValtozasAuthResult) -> partner_xml::stocks::GetStockChangeAuthResult {
    partner_xml::stocks::GetStockChangeAuthResult {
        answer: answer_to_en(result.valasz)
    }
}


fn answer_to_en(answer: o8_xml::stocks::valasz) -> partner_xml::stocks::Answer {
    partner_xml::stocks::Answer {
        version: answer.verzio,
        products: main_products_to_en(answer.cikkek),
        error: answer.hiba.map(|e| e.into())
    }
}


fn main_products_to_en(m_prods: o8_xml::stocks::cikkek) -> partner_xml::stocks::Products {
    partner_xml::stocks::Products {
        product: products_to_en(m_prods.cikk)
    }
}


fn products_to_en(prods: Vec<o8_xml::stocks::cikk>) -> Vec<partner_xml::stocks::Product> {
    let mut eng_products: Vec<partner_xml::stocks::Product> = Vec::new();
    for prod in prods {
        eng_products.push(product_to_en(prod));
    }
    eng_products
}


fn product_to_en(prod: o8_xml::stocks::cikk) -> partner_xml::stocks::Product {
    partner_xml::stocks::Product {
        id: prod.cikkid,
        no: prod.cikkszam,
        stock: prod.szabad
    }
}