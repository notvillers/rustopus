mod service;
use crate::service::{soap, ipv4};

mod o8_xml;

mod partner_xml;

use actix_web::{get, post, web, App, HttpResponse, HttpServer, Responder, Result};
use actix_files::NamedFile;
use serde::Deserialize;

mod converters;

async fn not_found() -> impl Responder {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("Page not found")
}


#[get("/")]
async fn index() -> impl Responder {
    let index_str = format!(r#"RustOpus @ Villers
__________________

Solution to convert hungarian Octopus 8 ERP SOAP's XML tags to english.
Some Octopus partner limits the IP addresses, if needed, give them '{}'

APIs
    /get-products
        methods
            GET
            POST
        arguments
            url
            authcode
            xmlns (optional)
        example
            GET
                curl "https://octopus.villers.website/get-products?url=https://domain.com/services/vision.asmx&authcode=your_auth_code"

    /get-stocks
        methods
            GET
            POST
        arguments
            url
            authcode
            xmlns (optional)
        example
            GET
                curl "https://octopus.villers.website/get-stocks?url=https://domain.com/services/vision.asmx&authcode=your_auth_code""#, 
            ipv4::get_ip().await);
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

    let xml = converters::products::get_products(url, &xmlns, authcode, &soap::get_first_date()).await;

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


#[derive(Deserialize)]
pub struct StockRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


async fn stocks_handler(params: StockRequest) -> impl Responder {
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
    if xmlns.trim().is_empty() &&url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    let xml = converters::stocks::get_stocks(url, &xmlns, authcode, &soap::get_first_date()).await;

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


#[get("/docs")]
async fn swagger_ui() -> Result<impl Responder> {
    Ok(NamedFile::open("./static/swagger.html")?)
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = service::config::get_settings();
    let host = "0.0.0.0";
    let port = config.server.port;

    println!("Running on '{}:{}'", host, port);

    HttpServer::new(|| {
        App::new()
            .default_service(web::to(not_found))
            .service(index).service(swagger_ui)
            .service(actix_files::Files::new("/", "./static").show_files_listing())
            .service(get_products_handler)
            .service(post_products_handler)
            .service(get_stocks_handler)
            .service(post_stocks_handler)
    })
    .bind((host, port))?
    .run()
    .await
}
