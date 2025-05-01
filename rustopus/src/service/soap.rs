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
            Err(e) => {
                println!("Response error: {}", e);
                "<Envelope></Envelope>".to_string()
            }
        },
        Err(e) => {
            println!("Response error: {}", e);
            "<Envelope></Envelope>".to_string()
        }
    }
}