use actix_web;

pub fn raise_read_instruction() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}
