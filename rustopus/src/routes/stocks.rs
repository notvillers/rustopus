use actix_web::{get, post, web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::converters::stocks::get_stocks;
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip;
use crate::service::soap_config::get_default_url;
use crate::routes;

#[derive(Deserialize)]
pub struct StockRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


async fn stocks_handler(req: HttpRequest, params: StockRequest) -> impl Responder {
    let ip_address = log_ip(req).await;
    let authcode = match params.authcode {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            let err_msg = "Authcode missing for stocks request";
            log_with_ip(&ip_address, err_msg);
            return routes::default::bad_user_request(Some(err_msg.to_string()))
        }
    };

    let url = match params.url {
        Some(ref s) if !s.trim().is_empty() => s,
        _ => {
            &match get_default_url() {
                Some(default_url) => {
                    log_with_ip(&ip_address, format!("Using default url: '{}'", default_url));
                    default_url
                }
                _ => {
                    let err_msg = "URL missing for stocks request";
                    log_with_ip(&ip_address, err_msg);
                    return routes::default::bad_user_request(Some(err_msg.to_string()))
                }
            }
        }
    };

    let mut xmlns = params.xmlns.unwrap_or_default();
    if xmlns.trim().is_empty() &&url.contains("/services/") {
        if let Some(pos) = url.find("/services/") {
            let end = pos + "/services/".len();
            xmlns = url[..end].to_string();
        }
    }

    log_with_ip(&ip_address, format!("Before getting stock request, url: {}, auth: {}", url, authcode));
    let xml = get_stocks(url, &xmlns, authcode, &get_first_date()).await;
    std::mem::drop(xmlns);
    log_with_ip(&ip_address, "After stocks request got");
    std::mem::drop(ip_address);

    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


#[get("/get-stocks")]
async fn get_stocks_handler(req: HttpRequest, query: web::Query<StockRequest>) -> impl Responder {
    stocks_handler(req, query.into_inner()).await
}


#[post("/get-stocks")]
async fn post_stocks_handler(req: HttpRequest, json: web::Json<StockRequest>) -> impl Responder {
    stocks_handler(req, json.into_inner()).await
}
