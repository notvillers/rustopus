use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::prices::get_prices;

fn raise_read_instruction() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}


#[derive(Deserialize)]
pub struct PriceRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


async fn prices_handler(params: PriceRequest) -> impl Responder {
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
    
    let xml = get_prices(url, &xmlns, pid, authcode).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-prices")]
async fn get_prices_handler(query: web::Query<PriceRequest>) -> impl Responder {
    let params = query.into_inner();

    prices_handler(params).await
}


#[post("/get-prices")]
async fn post_prices_handler(json: web::Json<PriceRequest>) -> impl Responder {
    let params = json.into_inner();

    prices_handler(params).await
}