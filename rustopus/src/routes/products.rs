use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::products::get_products;
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;
use crate::service::soap_config::{get_default_url};
use crate::routes;

#[derive(Deserialize)]
pub struct ProductRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>
}


async fn products_handler(req: HttpRequest, params: ProductRequest) -> impl Responder {
    let ip_address = log_ip(req).await;

    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            let err_msg = "Authcode missing for products request";
            log_with_ip(&ip_address, err_msg);
            return routes::default::bad_user_request(Some(err_msg.to_string()))
        }
    };
    
    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ =>  {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip(&ip_address, format!("Using default url: '{}'", default_url));
                    default_url
                },
                _ => {
                    let err_msg = "URL missing for products request and default not found";
                    log_with_ip(&ip_address, err_msg);
                    return routes::default::bad_user_request(Some(err_msg.to_string()))
                }
            }
        }
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    };

    log_with_ip(&ip_address, format!("Before getting products request, url: {}, auth: {}", url, authcode));
    let xml = get_products(url, &xmlns, authcode, &get_first_date()).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, "After products request got");
    std::mem::drop(ip_address);

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-products")]
pub async fn get_products_handler(req: HttpRequest, query: web::Query<ProductRequest>) -> impl Responder {
    products_handler(req, query.into_inner()).await
}


#[post("get-products")]
pub async fn post_products_handler(req: HttpRequest, json: web::Json<ProductRequest>) -> impl Responder {
    products_handler(req, json.into_inner()).await
}