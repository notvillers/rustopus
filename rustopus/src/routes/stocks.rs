use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::stocks::get_stocks;
use crate::soap::get_first_date;

use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;

fn raise_read_instruction() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}


#[derive(Deserialize)]
pub struct StockRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


async fn stocks_handler(req: HttpRequest, params: StockRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "Authcode missing for stocks request");
            return raise_read_instruction()
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "URL missing for stocks request");
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

    log_with_ip(&ip_address, format!("Getting stock request, url: {}, auth: {}", url, authcode));
    let xml = get_stocks(url, &xmlns, authcode, &get_first_date()).await;
    log_with_ip(&ip_address, "Stocks request got");

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-stocks")]
async fn get_stocks_handler(req: HttpRequest, query: web::Query<StockRequest>) -> impl Responder {
    stocks_handler(req, query.into_inner()).await
}


#[post("/get-stocks")]
async fn post_stocks_handler(req: HttpRequest, json: web::Json<StockRequest>) -> impl Responder {
    stocks_handler(req, json.into_inner()).await
}
