use actix_web::web::Query;
use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use crate::service::soap::get_first_date;
use crate::service::log::logger;
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
    let ip_address = log_ip(req);
    logger(format!("Bulk request from '{}'", ip_address));
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            logger(format!("Authcode missing for bulk request '{}'", ip_address));
            return raise_read_instruction()
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            logger(format!("URL missing for bulk request '{}'", ip_address));
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
            logger(format!("PID missing for bulk request '{}'", ip_address));
            return raise_read_instruction()
        }
    };

    logger(format!("Getting bulk request '{}'", ip_address));
    let xml = get_bulk(&url, &xmlns, &authcode, &get_first_date(), &pid).await;
    logger(format!("Bulk request got '{}'", ip_address));

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("get-bulk")]
pub async fn get_bulk_handler(req: HttpRequest, query: web::Query<BulkRequest>) -> impl Responder {
    let params = query.into_inner();

    bulk_handler(req, params).await
}