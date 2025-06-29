use actix_web;

pub fn send_xml(xml: String) -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}
