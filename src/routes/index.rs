use actix_web::{get, HttpResponse, Responder, http::header};

/// Handler
async fn handler() -> impl Responder {
    HttpResponse::Found()
        .append_header((header::LOCATION, "/docs/"))
        .finish()
}


/// Get handler
#[get("/")]
async fn get_handler() -> impl Responder {
    handler().await
}
