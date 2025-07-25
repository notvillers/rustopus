use std::fmt;
use reqwest;
use actix_web::HttpRequest;

use crate::service::log::{logger, elogger};

pub enum RequestIP {
    Ok(String),
    Err(String)
}

impl fmt::Display for RequestIP {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestIP::Ok(s) => write!(f, "{}", s),
            RequestIP::Err(s) => write!(f, "{}", s)
        }
    }
}


pub async fn get_ip() -> RequestIP {
    match reqwest::get("https://ip.villers.website").await {
        Ok(response) => {
            match response.text().await {
                Ok(body) => return RequestIP::Ok(body.trim().to_string()),
                Err(e) => elogger(format!("ipv4 error: {}", e)),
            }
        }
        Err(e) => elogger(format!("ipv4 error: {}", e))
    }
    RequestIP::Err("unknown ipv4 address".to_string())
}


pub async fn log_ip(req: HttpRequest) -> RequestIP {
    let ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next());

    match ip {
        Some(ip) => RequestIP::Ok(ip.to_string()),
        _ => {
            match req.peer_addr() {
                Some(peer_address) => {
                    let ip = peer_address.ip().to_string();
                    if ip == get_ip().await.to_string() {
                        logger(format!("IP request is coming from the host: {}", ip));
                    }
                    RequestIP::Ok(ip)
                }
                _ => {
                    elogger("Can not get IP address");
                    RequestIP::Err("unknown ip address".to_string())
                }
            }
        }
    }
}
