use actix_web::{
    get, HttpRequest, HttpResponse, Responder,
    http::header::LOCATION
};
use actix_files::NamedFile;

/// Handler
///
/// Serves the docs landing page directly at `/`; falls back to a redirect
/// to `/docs/` if the file cannot be opened.
async fn handler(req: HttpRequest) -> impl Responder {
    match NamedFile::open_async("./src/static/docs/index.html").await {
        Ok(file) => file.into_response(&req),
        Err(_) => HttpResponse::Found()
            .append_header((LOCATION, "/docs/"))
            .finish()
    }
}


/// Get handler
#[get("/")]
async fn get(req: HttpRequest) -> impl Responder {
    handler(req).await
}
