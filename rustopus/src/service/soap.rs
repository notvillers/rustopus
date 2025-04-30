use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use chrono::{DateTime, NaiveDateTime, TimeZone, Utc};

pub fn get_first_date() -> DateTime<Utc> {
    let naive_datetime = NaiveDateTime::new(
        chrono::NaiveDate::from_ymd_opt(2000, 1, 1).expect("Invalid date provided"), 
        chrono::NaiveTime::from_hms_opt(0, 0, 1).expect("Invalid time provided"));

    Utc.from_utc_datetime(&naive_datetime)
}


pub async fn get_response(url: &str, soap_request: String) -> String {
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