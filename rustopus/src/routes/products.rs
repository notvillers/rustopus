use actix_web::{get, post, web, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::products::get_products;
use crate::soap::get_first_date;

fn raise_read_instruction() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}


#[derive(Deserialize)]
pub struct ProductRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>
}


async fn products_handler(params: ProductRequest) -> impl Responder {
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => {
            s
        }
        _ => {
            return raise_read_instruction()
        }
    };
    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => {
            s
        }
        _ =>  {
            return raise_read_instruction()
        }
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    let xml = get_products(url, &xmlns, authcode, &get_first_date()).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-products")]
pub async fn get_products_handler(query: web::Query<ProductRequest>) -> impl Responder {
    products_handler(query.into_inner()).await
}


#[post("get-products")]
pub async fn post_products_handler(json: web::Json<ProductRequest>) -> impl Responder {
    products_handler(json.into_inner()).await
}