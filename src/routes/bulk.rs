use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::converters::bulk::{get_data, send_error_xml};
use crate::routes::default::send_xml;
use crate::service::soap::get_first_date;
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip;
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
            log_with_ip(&ip_address, format!("{}\t{}: {} ({})", uuid, error.code, error.description, REQUEST_NAME));
            return send_xml(send_error_xml(error.code, error.description));
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip(&ip_address, format!("{}\tUsing default url: '{}'", uuid, default_url));
                    default_url
                }
                _ => {
                    let error = errors::GLOBAL_URL_ERROR;
                    log_with_ip(&ip_address, format!("{}\t{}: {} ({})", uuid, error.code, error.description, REQUEST_NAME));
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
            log_with_ip(&ip_address, format!("{}\t{}: {} ({})", uuid, error.code, error.description, REQUEST_NAME));
            return send_xml(send_error_xml(error.code, error.description));
        }
    };

    log_with_ip(&ip_address, format!("{}\tBefore getting bulk request, url: {}, auth: {}, pid: {}", uuid, url, authcode, pid));
    let xml = get_data(&url, &xmlns, &authcode, &get_first_date(), &pid).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, format!("{}\tAfter bulk request got", uuid));
    std::mem::drop(ip_address);

    send_xml(xml)
}


#[get("/get-bulk")]
pub async fn get_bulk_handler(req: HttpRequest, query: web::Query<BulkRequest>) -> impl Responder {
    bulk_handler(req, query.into_inner()).await
}
