use actix_web::{get, post, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::routes::default::send_xml;
use crate::converters::prices::{get_data, send_error_xml};
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;
use crate::service::soap_config::get_default_url;
use crate::global::errors;

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
            let error = errors::GLOBAL_AUTH_ERROR;
            log_with_ip(&ip_address, format!("{}: {}", error.code, error.description));
            return send_xml(send_error_xml(error.code, error.description))
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip(&ip_address, format!("Using default url: '{}'", default_url));
                    default_url
                }
                _ => {
                    let error = errors::GLOBAL_URL_ERROR;
                    log_with_ip(&ip_address, format!("{}: {}", error.code, error.description));
                    return send_xml(send_error_xml(error.code, error.description))
                }
            }
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
            let error = errors::GLOBAL_PID_ERROR;
            log_with_ip(&ip_address, format!("{}: {}", error.code, error.description));
            return send_xml(send_error_xml(error.code, error.description))
        }
    };
    
    log_with_ip(&ip_address, format!("Before getting prices request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_data(url, &xmlns, pid, authcode).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, "After prices request got");
    std::mem::drop(ip_address);

    send_xml(xml)
}


#[get("/get-prices")]
async fn get_prices_handler(req: HttpRequest, query: web::Query<PriceRequest>) -> impl Responder {
    prices_handler(req, query.into_inner()).await
}


#[post("/get-prices")]
async fn post_prices_handler(req: HttpRequest, json: web::Json<PriceRequest>) -> impl Responder {
    prices_handler(req, json.into_inner()).await
}