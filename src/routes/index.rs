use actix_web::{
    get, HttpResponse, Responder,
    http::header::LOCATION
};

/// Handler
async fn handler() -> impl Responder {
    HttpResponse::Found()
        .append_header((LOCATION, "/docs/"))
        .finish()
}


/// Get handler
#[get("/")]
async fn get() -> impl Responder {
    handler().await
}
