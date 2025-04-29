mod service;
use crate::service::soap;

mod o8_xml;

mod partner_xml;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, HttpRequest};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ProductRequest {
    pub authcode: String,
    pub url: String,
    pub xmlns: String,
}


#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body("rustopus")
}


#[get("/get-products")]
async fn get_products_handler(query: web::Query<ProductRequest>) -> impl Responder {
    let mut req = query.into_inner();

    if req.xmlns.trim().is_empty() &&req.url.contains("/services/") {
        if let Some(pos) = req.url.find("/services/") {
            let end = pos + "/services/".len();
            req.xmlns = req.url[..end].to_string();
        }
    }

    let xml = soap::get_products(&req.url, &req.xmlns, &req.authcode, &soap::get_first_date()).await;

    println!("{}", xml);

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[post("/get-products")]
async fn post_products_handler(json: web::Json<ProductRequest>) -> impl Responder {
    let mut req = json.into_inner();

    if req.xmlns.trim().is_empty() &&req.url.contains("/services/") {
        if let Some(pos) = req.url.find("/services/") {
            let end = pos + "/services/".len();
            req.xmlns = req.url[..end].to_string();
        }
    }

    let xml = soap::get_products(&req.url, &req.xmlns, &req.authcode, &soap::get_first_date()).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = "0.0.0.0";
    let port = 1140;

    println!("Running on http://localhost:1140");

    HttpServer::new(|| {
        App::new()
            .service(index)
            .service(get_products_handler)
            .service(post_products_handler)
    })
    .bind((host, port))?
    .run()
    .await
}
