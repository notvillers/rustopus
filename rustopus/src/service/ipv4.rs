use reqwest;
use actix_web::HttpRequest;

pub async fn get_ip() -> String {
    match reqwest::get("https://ip.villers.website").await {
        Ok(response) => {
            let body = response.text().await;
            match body {
                Ok(body) => return body.trim().to_string(),
                Err(e) => println!("ipv4 error: {}", e)
            }
        }
        Err(e) => println!("ipv4 error: {}", e)
    }
    "unknown ipv4 address".to_string()
}


pub async fn log_ip(req: HttpRequest) -> String {
    let ip = req
        .headers()
        .get("X-Forwarded-For")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split(',').next());

    match ip {
        Some(ip) => ip.to_string(),
        _ => {
            match req.peer_addr() {
                Some(peer_address) => {
                    let ip = peer_address.ip().to_string();
                    if ip == get_ip().await {
                        return format!("host ip: {}", ip)
                    }
                    ip
                }
                _ => "unknown ip address".to_string()
            }
        }
    }
}
