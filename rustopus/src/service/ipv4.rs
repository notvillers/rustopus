use reqwest;
use actix_web::{web::get, HttpRequest};
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

pub async fn log_ip(req: HttpRequest) -> String {
    match req.headers().get("X-Forwarder-For").and_then(|v| v.to_str().ok()) {
        Some(ip) => {
            ip.to_string()
        }
        _ => {
            match req.peer_addr() {
                Some(peer_address) => {
                    let ip = peer_address.ip().to_string();
                    if ip == get_ip().await {
                        return format!("host ip: {}", ip).to_string()
                    }
                    ip.to_string()
                }
                _ => {
                    "unkown ip address".to_string()
                }
            }
        }
    }
}
