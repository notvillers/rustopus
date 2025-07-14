use actix_web::{get, web, HttpRequest, Responder};
use serde::Deserialize;

use crate::routes::default::GetResponse;
use crate::routes::default::{send_xml, get_auth, get_url, get_xmlns};
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::ipv4::log_ip;
use crate::partner_xml::barcode::error_struct_xml;
use crate::o8_xml::defaults::CallData;
use crate::service::new_soap::RequestGet;

#[derive(Deserialize)]
pub struct BarcodeRequest {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>
}


const REQUEST_NAME: &'static str = "BARCODE REQUEST";

async fn handler(req: HttpRequest, params: BarcodeRequest) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await;

    let authcode = match get_auth(REQUEST_NAME, &ip_address, &uuid, params.authcode, error_struct_xml) {
        GetResponse::Text(auth) => auth,
        GetResponse::Response(reponse) => return reponse
    };

    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, params.url, error_struct_xml) {
        GetResponse::Text(url) => url,
        GetResponse::Response(response) => return response
    };

    let xmlns = get_xmlns(params.xmlns, &url);

    let call_data = CallData {
        authcode: authcode,
        url: url,
        xmlns: xmlns,
        pid: None
    };

    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}", REQUEST_NAME, call_data.url, call_data.authcode));
    let xml = RequestGet::Barcode(call_data).to_xml().await;
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    send_xml(xml)
}

#[get("/get-barcodes")]
pub async fn get_handler(req: HttpRequest, query: web::Query<BarcodeRequest>) -> impl Responder {
    handler(req, query.into_inner()).await
}
