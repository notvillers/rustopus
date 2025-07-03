use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::routes::default::{GetResponse, GetPidResponse};
use crate::routes::default::{send_xml, get_auth, get_url, get_xmlns, get_pid};
use crate::converters::prices::{get_data, send_error_xml};
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip_uuid;
use crate::service::slave::get_uuid;

#[derive(Deserialize)]
pub struct PriceRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


const REQUEST_NAME: &'static str = "PRICES REQUEST";

async fn prices_handler(req: HttpRequest, params: PriceRequest) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await;
    
    let authcode = match get_auth(REQUEST_NAME, &ip_address, &uuid, params.authcode, send_error_xml) {
        GetResponse::Text(auth) => auth,
        GetResponse::Response(response) => return response
    };

    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, params.url, send_error_xml) {
        GetResponse::Text(url) => url,
        GetResponse::Response(response) => return response
    };

    let xmlns = get_xmlns(params.xmlns, &url);

    let pid = match get_pid(REQUEST_NAME, &ip_address, &uuid, params.pid, send_error_xml) {
        GetPidResponse::Number(pid) => pid,
        GetPidResponse::Response(response) => return response
    };
    
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting prices request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_data(&url, &xmlns, &pid, &authcode).await;
    log_with_ip_uuid(&ip_address, &uuid, "After prices request got");

    send_xml(xml)
}


#[get("/get-prices")]
async fn get_prices_handler(req: HttpRequest, query: web::Query<PriceRequest>) -> impl Responder {
    prices_handler(req, query.into_inner()).await
}