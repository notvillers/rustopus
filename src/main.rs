use std::{env, panic};
use actix_web::{App, HttpResponse, HttpServer, Responder, web, middleware::{Compress, DefaultHeaders}};
use actix_files::Files;

mod macros;
mod service;
mod forms;
mod routes;
mod global;
mod tools;
mod language;

use crate::{
    routes::{barcode, bulk, image, index, invoice, mat, order, price, product, stock, test}, service::{
        log::{elogger, logger}, mcp, soap_config::{
            SOAP_URL, SoapConfig, check_soap_config, get_soap_path
        }
    }
};

async fn not_found() -> impl Responder {
    HttpResponse::NotFound()
        .content_type("text/plain")
        .body("Page not found")
}


/// Security response headers applied to every response.
///
/// Values are tuned so the self-hosted Swagger UI at `/docs/` keeps working:
/// all of its scripts are served from the same origin, and it injects inline
/// `<style>` blocks at runtime (hence `style-src 'unsafe-inline'`).
fn security_headers() -> DefaultHeaders {
    DefaultHeaders::new()
        .add(("Content-Security-Policy", "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'; frame-ancestors 'none'; base-uri 'self'; form-action 'self'"))
        .add(("Strict-Transport-Security", "max-age=31536000; includeSubDomains"))
        .add(("X-Content-Type-Options", "nosniff"))
        .add(("X-Frame-Options", "DENY"))
        .add(("Referrer-Policy", "no-referrer"))
        .add(("Permissions-Policy", "geolocation=(), microphone=(), camera=()"))
        .add(("X-Permitted-Cross-Domain-Policies", "none"))
        .add(("Cross-Origin-Embedder-Policy", "require-corp"))
        .add(("Cross-Origin-Opener-Policy", "same-origin"))
        .add(("Cross-Origin-Resource-Policy", "same-origin"))
}


#[actix_web::main]
async fn main() -> std::io::Result<()> {
    panic::set_hook(Box::new(|info| {
        elogger(format!("Panic: {:?}", info));
    }));

    let config = service::config::get_settings();

    let soap_url: Option<String> = if check_soap_config() {
        Some(SoapConfig::load().url
            .unwrap_or_default())
            .filter(|s| !s.is_empty())
    } else {
        elogger(format!("'{:#?}' not found. (Do not bother this message, if you are not willing to work with static 'url'.)", get_soap_path()));
        None
    };

    let _ = SOAP_URL.set(soap_url);

    logger(format!("Running on '{}:{}', with {} worker{}", config.server.host, config.server.port, config.server.workers, if config.server.workers > 1 { "s" } else { "" }));
    
    let docs_dir = match env::current_dir() {
        Ok(dir) => dir.join("src").join("static").join("docs"),
        Err(e) => {
            elogger(format!("Failed to get current directory: '{}'", e));
            return Err(std::io::Error::other(e));
        }
    };

    let license_dir = match env::current_dir() {
        Ok(dir) => dir.join("src").join("static").join("LICENSE"),
        Err(e) => {
            elogger(format!("Failed to get current directory: '{}'", e));
            return Err(std::io::Error::other(e));
        }
    };

    // MCP endpoint. Off by default: with `[mcp] enabled = false` nothing below
    // is built, no route is registered and no background task is spawned, so
    // the REST service keeps its original startup and memory profile.
    let mcp_config = service::config::get_mcp_settings();
    let mcp_service = if mcp_config.is_enabled() {
        // Built once, outside the `HttpServer::new` closure, so every actix
        // worker shares one session manager (see `mcp::tools::build_service`).
        logger("MCP enabled: serving /mcp");
        mcp::precache::spawn(mcp_config.precache_interval_secs());
        Some(mcp::tools::build_service())
    } else {
        None
    };

    // Admin dashboard. Registered only when MCP is on *and* a token is set, so
    // the credential store behind it is never reachable unauthenticated.
    let admin_token = mcp_config.is_enabled().then(|| mcp_config.admin_token()).flatten();
    match (&admin_token, mcp_config.is_enabled()) {
        (Some(_), _) => logger("MCP admin dashboard enabled: serving /admin"),
        (None, true) => logger("MCP admin dashboard not served: no admin token set (RUSTOPUS_ADMIN_TOKEN or [mcp] admin_token)"),
        (None, false) => ()
    }

    let admin_dir = match env::current_dir() {
        Ok(dir) => dir.join("src").join("static").join("admin"),
        Err(e) => {
            elogger(format!("Failed to get current directory: '{}'", e));
            return Err(std::io::Error::other(e));
        }
    };

    let server = HttpServer::new(move || {
        // `rmcp-actix-web` mounts a service scope rather than a `get`/`get_alias`
        // pair. That is deliberate and not a convention slip: /mcp is a protocol
        // transport (POST/GET/DELETE on one path, session-managed), not a
        // fetcher route, so the singular/plural alias pattern does not apply.
        let mcp_scope = mcp_service.clone().map(|service| service.scope_with_path("/mcp"));
        let admin_scope = admin_token.clone().map(|token| {
            mcp::admin::scope(mcp::admin::AdminState::new(token, admin_dir.clone()))
        });

        let app = App::new()
            .wrap(Compress::default())
            .wrap(security_headers())
            .default_service(web::to(not_found))
            .service(index::get)
            .service(Files::new("/docs/", docs_dir.clone())
                .index_file("index.html")
                .use_last_modified(true))
            .service(Files::new("/LICENSE/", license_dir.clone())
                .index_file("index.html")
                .use_last_modified(true))
            .service(product::get).service(product::get_alias)
            .service(stock::get).service(stock::get_alias)
            .service(price::get).service(price::get_alias)
            .service(image::get).service(image::get_alias)
            .service(barcode::get).service(barcode::get_alias)
            .service(bulk::get).service(bulk::get_alias)
            .service(invoice::get).service(invoice::get_alias)
            .service(mat::get).service(mat::get_alias)
            .service(order::post).service(order::post_alias)
            .service(test::get_handler);

        let app = match admin_scope {
            Some(scope) => app.service(scope),
            None => app
        };

        match mcp_scope {
            Some(scope) => app.service(scope),
            None => app
        }
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
