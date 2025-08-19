use std::panic;

mod service;
use std::env;

use crate::service::{ipv4, log::{logger, elogger}};

mod o8_xml;

mod partner_xml;

mod routes;

mod global;

use actix_web::{get, web, App, HttpResponse, HttpServer, Responder};
use actix_web::http::header;
use actix_files::Files;

use crate::service::soap_config::{get_soap_path, check_soap_config};

async fn not_found() -> impl Responder {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("Page not found")
}


#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Found()
        .append_header((header::LOCATION, "/docs/"))
        .finish()
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {

    panic::set_hook(Box::new(|info| {
        elogger(format!("Panic: {:?}", info));
    }));

    let config = service::config::get_settings();

    if !check_soap_config() {
        elogger(format!("'{:#?}' not found. (Do not bother this message if you are not willing to work with static 'url'.)", get_soap_path()));
    }

    logger(format!("Running on '{}:{}', with {} worker(s)", config.server.host, config.server.port, config.server.workers));

    let current_dir = env::current_dir().expect("Failed to get current directory");

    let docs_dir = current_dir.join("src").join("static").join("docs");
    
    let server = HttpServer::new(move || {
        App::new()
            .service(index)
            .service(Files::new("/docs/", docs_dir.clone())
                .index_file("index.html")
                .use_last_modified(true))
            .default_service(web::to(not_found))
            .service(routes::products::get_handler)
            .service(routes::stocks::get_handler)
            .service(routes::prices::get_handler)
            .service(routes::images::get_handler)
            .service(routes::barcode::get_handler)
            .service(routes::bulk::get_handler)
            .service(routes::invoices::get_handler)
            .service(routes::test::get_handler)
    })
        .client_request_timeout(std::time::Duration::from_secs(1200))
        .keep_alive(std::time::Duration::from_secs(1200))
        .bind((config.server.host, config.server.port))?
        .workers(config.server.workers)
        .run();

    match server.await {
        Err(e) => elogger(format!("Server exited with error: {}", e)),
        _ => logger("Server stopped gracefully.")
    }

    Ok(())
}
