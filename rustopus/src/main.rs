mod service;
use std::{env, path::PathBuf};

use crate::service::{soap, ipv4, log::logger};

mod o8_xml;

mod partner_xml;

mod routes;

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web::http::header;
use actix_files::Files;
mod converters;

async fn not_found() -> impl Responder {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("Page not found")
}


fn raise_read_instruction() -> HttpResponse {
    HttpResponse::Ok()
        .content_type("text/plain")
        .body("Read index for instructions!")
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
    //HttpResponse::Ok().body(index_str)
    HttpResponse::Found()
        .append_header((header::LOCATION, "/docs/"))
        .finish()
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let config = service::config::get_settings();
    let host = "0.0.0.0";
    let port = config.server.port;

    logger(format!("Running on '{}:{}'", host, port));

    let current_dir = env::current_dir().expect("Failed to get current directory");
    let docs_dir: PathBuf;
    docs_dir = current_dir.join("src").join("static").join("docs");

    HttpServer::new(move || {
        App::new()
            .service(index)
            .service(Files::new("/docs/", docs_dir.clone())
                .index_file("index.html")
                .use_last_modified(true))
            .default_service(web::to(not_found))
            .service(routes::products::get_products_handler)
            .service(routes::products::post_products_handler)
            .service(routes::stocks::get_stocks_handler)
            .service(routes::stocks::post_stocks_handler)
            .service(routes::prices::get_prices_handler)
            .service(routes::prices::post_prices_handler)
            .service(routes::bulk::get_bulk_handler)
    })
    .client_request_timeout(std::time::Duration::from_secs(1200))
    .keep_alive(std::time::Duration::from_secs(1200))
    .bind((host, port))?
    .run()
    .await
}
