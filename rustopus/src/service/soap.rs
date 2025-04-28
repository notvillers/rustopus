use reqwest::{blocking::Client, Response};
use reqwest::header::CONTENT_TYPE;
use chrono::{Date, DateTime, NaiveDateTime, TimeZone, Utc};

pub fn get_first_date() -> DateTime<Utc> {
    let naive_datetime = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(2000, 1, 1).expect("Invalid date provided"), 
        chrono::NaiveTime::from_hms_opt(0, 0, 1).expect("Invalid time provided"));

    Utc.from_utc_datetime(&naive_datetime)
}


fn get_response(url: &str, soap_request: String) -> String {
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


pub fn get_products(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {
    let soap_request: String = get_products_xml(xmlns, &web_update, &authcode);

    let response_text: String = get_response(url, soap_request);

    response_text
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


pub fn get_stock(url: &str, xmlns: &str, authcode: &str, web_update: &DateTime<Utc>) -> String {

    let soap_request: String = get_stock_xml(xmlns, &web_update, authcode);

    let response_text: String = get_response(url, soap_request);

    response_text
}