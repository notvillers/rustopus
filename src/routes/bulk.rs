use actix_web::{get, web::Query, HttpRequest, Responder};
use serde::Deserialize;

use crate::routes::default::{GetResponse, GetPidResponse};
use crate::routes::default::{send_xml, get_auth, get_url, get_xmlns, get_pid};
use crate::service::soap::get_first_date;
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::ipv4::log_ip;
use crate::converters::bulk::{get_data, send_error_xml};

#[derive(Deserialize)]
pub struct BulkRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


const REQUEST_NAME: &'static str = "BULK REQUEST";

async fn bulk_handler(req: HttpRequest, params: BulkRequest) -> impl Responder {
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

    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting bulk request, url: {}, auth: {}, pid: {}", url, authcode, pid));
    let xml = get_data(&url, &xmlns, &authcode, &get_first_date(), &pid).await;
    log_with_ip_uuid(&ip_address, &uuid, "After bulk request got");

    send_xml(xml)
}


#[get("/get-bulk")]
pub async fn get_bulk_handler(req: HttpRequest, query: Query<BulkRequest>) -> impl Responder {
    bulk_handler(req, query.into_inner()).await
}
