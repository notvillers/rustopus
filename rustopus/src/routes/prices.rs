use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::routes;
use crate::converters::prices::get_prices;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;

#[derive(Deserialize)]
pub struct PriceRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


async fn prices_handler(req: HttpRequest, params: PriceRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "Authcode missing for price request");
            return routes::default::raise_read_instruction()
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "URL missing for price request");
            return routes::default::raise_read_instruction()
        }
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() &&url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    let pid = match params.pid {
        Some(ref s) => s,
        _ => {
            log_with_ip(&ip_address, "PID missing for price request");
            return routes::default::raise_read_instruction()
        }
    };
    
    log_with_ip(&ip_address, format!("Before getting prices request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_prices(url, &xmlns, pid, authcode).await;
    log_with_ip(&ip_address, "After prices request got");

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-prices")]
async fn get_prices_handler(req: HttpRequest, query: web::Query<PriceRequest>) -> impl Responder {
    prices_handler(req, query.into_inner()).await
}


#[post("/get-prices")]
async fn post_prices_handler(req: HttpRequest, json: web::Json<PriceRequest>) -> impl Responder {
    prices_handler(req, json.into_inner()).await
}