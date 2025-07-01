use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::converters::images::{get_data, send_error_xml};
use crate::routes::default::send_xml;
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;
use crate::service::soap_config::get_default_url;
use crate::routes;
use crate::global::errors;

#[derive(Deserialize)]
pub struct ImagesRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>
}


const REQUEST_NAME: &'static str = "IMAGES REQUEST";

async fn products_handler(req: HttpRequest, params: ImagesRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            let error = errors::GLOBAL_AUTH_ERROR;
            log_with_ip(&ip_address, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
            return routes::default::send_xml(send_error_xml(error.code, error.description))
        }
    };
    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ =>  {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip(&ip_address, format!("Using default url: '{}'", default_url));
                    default_url
                }
                _ => {
                    let error = errors::GLOBAL_URL_ERROR;
                    log_with_ip(&ip_address, format!("{}: {} ({})", error.code, error.description, REQUEST_NAME));
                    return routes::default::send_xml(send_error_xml(error.code, error.description))
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
    }

    log_with_ip(&ip_address, format!("Before getting images request, url: {}, auth: {}", url, authcode));
    let xml = get_data(url, &xmlns, authcode, &get_first_date()).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, "After images request got");
    std::mem::drop(ip_address);

    send_xml(xml)
}


#[get("/get-images")]
pub async fn get_images_handler(req: HttpRequest, query: web::Query<ImagesRequest>) -> impl Responder {
    products_handler(req, query.into_inner()).await
}
