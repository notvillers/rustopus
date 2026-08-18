use std::fmt;
use actix_web::HttpRequest;

use crate::service::log::elogger;

/// `RequestIP` enum
pub enum RequestIP {
    Ok(String),
    Err(String)
}

impl fmt::Display for RequestIP {
    /// fmt display for `RequestIP` enum
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RequestIP::Ok(s) => write!(f, "{}", s),
            RequestIP::Err(s) => write!(f, "{}", s)
        }
    }
}


/// This function tries to get ipv4 address from the request
pub async fn log_ip(req: HttpRequest) -> RequestIP {
    if let Some(ip) = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|x| x.to_str().ok())
        .and_then(|x| x.split(',').next()) {
            return RequestIP::Ok(ip.into())
    }
    if let Some(peer_address) = req.peer_addr() {
        let ip = peer_address.ip().to_string();
        return RequestIP::Ok(ip)
    }
    elogger("Can not get IP address");
    RequestIP::Err("unknown IP address".into())
}
