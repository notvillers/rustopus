use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use crate::service::soap::get_first_date;
use crate::service::log::log_with_ip;
use crate::ipv4::log_ip;

use crate::converters::bulk::get_bulk;


fn raise_read_instruction() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}

#[derive(Deserialize)]
pub struct BulkRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}

async fn bulk_handler(req: HttpRequest, params: BulkRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "Authcode missing for bulk request");
            return raise_read_instruction()
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "URL missing for bulk request");
            return raise_read_instruction()
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
            log_with_ip(&ip_address, "PID missing for bulk request");
            return raise_read_instruction()
        }
    };

    log_with_ip(&ip_address, format!("Before getting bulk request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_bulk(&url, &xmlns, &authcode, &get_first_date(), &pid).await;
    log_with_ip(&ip_address, "After bulk request got");

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-bulk")]
pub async fn get_bulk_handler(req: HttpRequest, query: web::Query<BulkRequest>) -> impl Responder {
    bulk_handler(req, query.into_inner()).await
}


#[post("/get-bulk")]
pub async fn post_bulk_handler(req: HttpRequest, json: web::Json<BulkRequest>) -> impl Responder {
    bulk_handler(req, json.into_inner()).await
}