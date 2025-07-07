use std::time::Duration;

use reqwest::Client;
use reqwest::header::CONTENT_TYPE;

use crate::service::config;
use crate::service::log::logger;

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
                logger(format!("Response error: {}", e));
                "<Envelope></Envelope>".to_string()
            }
        },
        Err(e) => {
            logger(format!("Response error: {}", e));
            "<Envelope></Envelope>".to_string()
        }
    }
}
