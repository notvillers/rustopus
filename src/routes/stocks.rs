use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::converters::stocks::{get_data, send_error_xml};
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip_uuid;
use crate::service::soap_config::get_default_url;
use crate::service::slave::get_uuid;
use crate::routes::default::send_xml;
use crate::global::errors;

#[derive(Deserialize)]
pub struct StockRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


const REQUEST_NAME: &'static str = "STOCKS REQUEST";

async fn stocks_handler(req: HttpRequest, params: StockRequest) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            let error = errors::GLOBAL_AUTH_ERROR;
            log_with_ip_uuid(&ip_address, &uuid, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
            return send_xml(send_error_xml(error.code, error.description))
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip_uuid(&ip_address, &uuid, format!("Using default url: '{}'", default_url));
                    default_url
                }
                _ => {
                    let error = errors::GLOBAL_URL_ERROR;
                    log_with_ip_uuid(&ip_address, &uuid, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
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

    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting stock request, url: {}, auth: {}", url, authcode));
    let xml = get_data(url, &xmlns, authcode, &get_first_date()).await;
    log_with_ip_uuid(&ip_address, &uuid, format!("{}\tAfter stocks request got", uuid));

    send_xml(xml)
}


#[get("/get-stocks")]
async fn get_stocks_handler(req: HttpRequest, query: web::Query<StockRequest>) -> impl Responder {
    stocks_handler(req, query.into_inner()).await
}
