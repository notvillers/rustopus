mod service;
use crate::service::soap;

mod o8_xml;

mod partner_xml;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder};
use serde::Deserialize;


async fn not_found() -> impl Responder {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("Page not found")
}


#[get("/")]
async fn index() -> impl Responder {
    let index_str = r#"RustOpus @ Villers
__________________

Solution to convert hungarian Octopus 8 ERP SOAP data to english

APIs
    /get-products
        methods
            GET
            POST
        arguments
            authcode
            url
            xmlns (optional)   
    /get-stocks (under development)"#;
    HttpResponse::Ok().body(index_str)
}


#[derive(Deserialize)]
pub struct ProductRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


async fn products_handler(params: ProductRequest) -> impl Responder {
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => return HttpResponse::Ok()
                        .content_type("text/plain")
                        .body("Read index for instructions!")
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => return HttpResponse::Ok()
                        .content_type("text/plain")
                        .body("Read index for instructions!")
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    let xml = soap::get_products(url, &xmlns, authcode, &soap::get_first_date()).await;

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-products")]
async fn get_products_handler(query: web::Query<ProductRequest>) -> impl Responder {
    let params = query.into_inner();

    products_handler(params).await
}


#[post("/get-products")]
async fn post_products_handler(json: web::Json<ProductRequest>) -> impl Responder {
    let params = json.into_inner();

    products_handler(params).await
}


#[get("/get-stocks")]
async fn get_stocks_handler() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Under development")
}


#[post("/get-stocks")]
async fn post_stocks_handler() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Under development")
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let host = "0.0.0.0";
    let port = 1140;

    println!("Running on http://{}:{}", host, port);

    HttpServer::new(|| {
        App::new()
            .default_service(web::to(not_found))
            .service(index)
            .service(get_products_handler)
            .service(post_products_handler)
            .service(get_stocks_handler)
            .service(post_stocks_handler)
    })
    .bind((host, port))?
    .run()
    .await
}
