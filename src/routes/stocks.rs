use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::converters::stocks::{get_data, send_error_xml};
use crate::soap::get_first_date;
use crate::service::ipv4::log_ip;
use crate::service::log::log_with_ip_uuid;
use crate::service::slave::get_uuid;
use crate::routes::default::GetResponse;
use crate::routes::default::{send_xml, get_auth, get_url, get_xmlns};

#[derive(Deserialize)]
pub struct StockRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
}


const REQUEST_NAME: &'static str = "STOCKS REQUEST";

async fn stocks_handler(req: HttpRequest, params: StockRequest) -> impl Responder {
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

    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting stock request, url: {}, auth: {}", url, authcode));
    let xml = get_data(&url, &xmlns, &authcode, &get_first_date()).await;
    log_with_ip_uuid(&ip_address, &uuid, format!("{}\tAfter stocks request got", uuid));

    send_xml(xml)
}


#[get("/get-stocks")]
async fn get_stocks_handler(req: HttpRequest, query: web::Query<StockRequest>) -> impl Responder {
    stocks_handler(req, query.into_inner()).await
}
