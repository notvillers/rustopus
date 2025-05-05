use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::stocks::get_stocks;
use crate::soap::get_first_date;

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


async fn stocks_handler(params: StockRequest) -> impl Responder {
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

    let xml = get_stocks(url, &xmlns, authcode, &get_first_date()).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-stocks")]
async fn get_stocks_handler(query: web::Query<StockRequest>) -> impl Responder {
    let params = query.into_inner();

    stocks_handler(params).await
}


#[post("/get-stocks")]
async fn post_stocks_handler(json: web::Json<StockRequest>) -> impl Responder {
    let params = json.into_inner();

    stocks_handler(params).await
}
