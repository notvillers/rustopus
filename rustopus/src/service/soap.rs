use reqwest::{blocking::Client, Response};
use reqwest::header::CONTENT_TYPE;
use chrono::{DateTime, Utc};

fn get_products_xml(xmlns: String, web_update: DateTime<Utc>, authcode: String) -> String {
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
        web_update,
        authcode
    )
}

pub fn get_products(url: &String, xmlns: &String, authcode: &String, web_update: &Option<DateTime<Utc>>) -> String {
    let web_update: DateTime<Utc> = web_update.unwrap_or_else(Utc::now);
    let soap_request: String = get_products_xml(xmlns.clone(), web_update, authcode.clone());

    let client: Client = Client::new();
    let response: Result<reqwest::blocking::Response, reqwest::Error> = client
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send();

    match response {
        Ok(response) => {
            let response_text: Result<String, reqwest::Error> = response.text();
            match response_text {
                Ok(response_text) => {
                    response_text
                }
                Err(_) => {
                    "ERROR".to_string()
                }
            }
        }
        Err(_) => {
            "ERROR".to_string()
        }
    }
}