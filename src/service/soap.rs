use std::time::Duration;
use once_cell::sync::Lazy;
use reqwest::{
    Client,
    header::CONTENT_TYPE
};

use crate::service::{
    config,
    log::elogger
};

/// Process-wide reqwest client, built once so the connection pool (and TLS
/// sessions) are reused across every outbound Octopus call.
static CLIENT: Lazy<Client> = Lazy::new(|| {
    match Client::builder()
        .timeout(Duration::from_secs(config::get_settings().server.timeout))
        .build() {
            Ok(client) => client,
            Err(error) => {
                elogger(format!("Error creating reqwest client: {error}"));
                Client::new()
        }
    }
});

/// This function handles the request to the given url with the given soap string, theoretically it can handle other requests too
pub async fn get_response(url: &str, soap_request: String) -> String {
    match CLIENT
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send()
        .await {
            Ok(resp) => match resp.text().await {
                Ok(text) => return text,
                Err(error) => elogger(format!("Response error: {}", error))
            },
            Err(error) => elogger(format!("Response error: {}", error))
    }
    "<Envelope></Envelope>".into()
}
