use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::converters::bulk::{get_data, send_error_xml};
use crate::routes::default::send_xml;
use crate::service::soap::get_first_date;
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::service::soap_config::get_default_url;
use crate::ipv4::log_ip;
use crate::global::errors;

#[derive(Deserialize)]
pub struct BulkRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


const REQUEST_NAME: &'static str = "BULK REQUEST";

async fn bulk_handler(req: HttpRequest, params: BulkRequest) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            let error = errors::GLOBAL_AUTH_ERROR;
            log_with_ip_uuid(&ip_address, &uuid, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
            return send_xml(send_error_xml(error.code, error.description));
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

    let pid = match params.pid {
        Some(ref s) => s,
        _ => {
            let error = errors::GLOBAL_PID_ERROR;
            log_with_ip_uuid(&ip_address, &uuid, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
            return send_xml(send_error_xml(error.code, error.description));
        }
    };

    log_with_ip_uuid(&ip_address, &uuid, format!("tBefore getting bulk request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_data(&url, &xmlns, &authcode, &get_first_date(), &pid).await;
    log_with_ip_uuid(&ip_address, &uuid, "After bulk request got");

    send_xml(xml)
}


#[get("/get-bulk")]
pub async fn get_bulk_handler(req: HttpRequest, query: web::Query<BulkRequest>) -> impl Responder {
    bulk_handler(req, query.into_inner()).await
}
