use actix_web::{get, web, HttpRequest, Responder};

use crate::routes::default::GetResponse;
use crate::routes::default::{RequestParameters, send_xml, get_auth, get_url, get_xmlns};
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::ipv4::log_ip;
use crate::partner_xml::images::error_struct_xml;
use crate::o8_xml::defaults::CallData;
use crate::service::get_data::RequestGet;


// Request name
const REQUEST_NAME: &'static str = "IMAGES REQUEST";

/// Handler
async fn handler(req: HttpRequest, params: RequestParameters) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await;

    // Trying to get url from parameters
    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, params.url, error_struct_xml) {
        GetResponse::Text(url) => url,
        GetResponse::Response(response) => return response
    };

    // Getting XMLNS from parameters, otherwise using url
    let xmlns = get_xmlns(params.xmlns, &url);

    // Creating call data from parameters
    let call_data = CallData {
        // Getting authentication code from parameters
        authcode: match get_auth(REQUEST_NAME, &ip_address, &uuid, params.authcode, error_struct_xml) {
            GetResponse::Text(auth) => auth,
            GetResponse::Response(response) => return response
        },
        url: url,
        xmlns: xmlns,
        pid: None
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting images request, url: {}, auth: {}", call_data.url, call_data.authcode));

    // Getting data
    let xml = RequestGet::Images(call_data).to_xml().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, "After images request got");

    // Sending back xml as response
    send_xml(xml)
}


/// GET handler
#[get("/get-images")]
pub async fn get_handler(req: HttpRequest, query: web::Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
