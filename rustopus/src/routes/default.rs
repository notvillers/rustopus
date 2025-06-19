use actix_web;

pub fn raise_read_instruction() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("text/plain")
        .body("Please read '/docs' for instructions!")
}


pub fn bad_user_request(message: Option<String>) -> actix_web::HttpResponse {
    actix_web::HttpResponse::BadRequest()
        .body(message.unwrap_or("Invalid configuration".to_string()))
}
