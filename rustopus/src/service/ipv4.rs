use reqwest;
use actix_web::HttpRequest;
use crate::service::log::logger;

pub async fn get_ip() -> String {
    let response = reqwest::get("https://ip.villers.website").await;
    match response {
        Ok(response) => {
            let body = response.text().await;
            match body {
                Ok(body) => {
                    return body.trim().to_string()
                }
                Err(e) => {
                    println!("ipv4 error: {}", e)
                }
            }
        }
        Err(e) => {
            println!("ipv4 error: {}", e)
        }
    }
    "unknown ipv4 address".to_string()
}

pub fn log_ip(req: HttpRequest) -> String {
    match req.peer_addr() {
        Some(peer_address) => {
            peer_address.ip().to_string()
        }
        _ => {
            "unknown IP address".to_string()
        }
    }
}