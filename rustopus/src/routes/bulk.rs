use actix_web::web::Query;
use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;
use crate::soap::get_first_date;

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

async fn bulk_handler(params: BulkRequest) -> impl Responder {
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => return raise_read_instruction()
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => return raise_read_instruction()
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
        _ => return raise_read_instruction()
    };
    
    let xml = get_bulk(&url, &xmlns, &authcode, &get_first_date(), &pid).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("get-bulk")]
pub async fn get_bulk_handler(query: web::Query<BulkRequest>) -> impl Responder {
    let params = query.into_inner();

    bulk_handler(params).await
}