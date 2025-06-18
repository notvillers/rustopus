use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::images::get_images;
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;
use crate::routes;

#[derive(Deserialize)]
pub struct ImagesRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>
}


async fn products_handler(req: HttpRequest, params: ImagesRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            log_with_ip(&ip_address, "Authcode missing for images request");
            return routes::default::raise_read_instruction()
        }
    };
    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ =>  {
            log_with_ip(&ip_address, "URL missing for images request");
            return routes::default::raise_read_instruction()
        }
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    log_with_ip(&ip_address, format!("Before getting images request, url: {}, auth: {}", url, authcode));
    let xml = get_images(url, &xmlns, authcode, &get_first_date()).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, "After images request got");
    std::mem::drop(ip_address);

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-images")]
pub async fn get_images_handler(req: HttpRequest, query: web::Query<ImagesRequest>) -> impl Responder {
    products_handler(req, query.into_inner()).await
}


#[post("get-images")]
pub async fn post_images_handler(req: HttpRequest, json: web::Json<ImagesRequest>) -> impl Responder {
    products_handler(req, json.into_inner()).await
}
