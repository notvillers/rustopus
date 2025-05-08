use std::time::Duration;

use reqwest::Client;
use reqwest::header::CONTENT_TYPE;
use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};

use crate::service::config;


pub fn get_first_date() -> DateTime<Utc> {
    get_date_from_parts(None, None, None, None, None, None)
}


pub fn get_date_from_parts(year: Option<i32>, month: Option<u32>, day: Option<u32>, hour: Option<u32>, minute: Option<u32>, second: Option<u32>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::new(
            match chrono::NaiveDate::from_ymd_opt(year.unwrap_or(1900), month.unwrap_or(1), day.unwrap_or(1)) {
                Some(date) => date,
                _ => NaiveDate::MIN
            }, 
            match chrono::NaiveTime::from_hms_opt(hour.unwrap_or(0), minute.unwrap_or(0), second.unwrap_or(1)) {
                Some(time) => time,
                _ => NaiveTime::MIN
            }
        )
    )
}


pub async fn get_response(url: &str, soap_request: String) -> String {
    let client = match Client::builder()
        .timeout(Duration::from_secs(config::get_settings().server.timeout))
        .build() {
        Ok(client) => client,
        Err(e) => {
            println!("Error creating reqwest client: {e}");
            Client::new()
        }
    };
    match client
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send()
        .await {
        Ok(resp) => match resp.text().await {
            Ok(text) => text,
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