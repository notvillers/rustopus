use actix_web::{get, web::Query, HttpRequest, Responder};

use crate::routes::default::{RequestParameters, GetStringResponse, GetI64Response, GetDateResponse, send_xml, get_auth, get_url, get_xmlns, get_pid, get_date};
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::ipv4::log_ip;
use crate::partner_xml::bulk::error_struct_xml;
use crate::o8_xml::defaults::CallData;
use crate::service::get_data::RequestGet;

/// Name of the current request
const REQUEST_NAME: &'static str = "BULK REQUEST";

/// Handler
async fn handler(req: HttpRequest, params: RequestParameters) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await.to_string();

    // Trying to get url from parameters
    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, params.url, error_struct_xml) {
        GetStringResponse::Text(url) => url,
        GetStringResponse::Response(response) => return response
    };

    // Getting XMLNS from parameters, otherwise using url
    let xmlns = get_xmlns(params.xmlns, &url);

    // Creating call data from parameters
    let call_data = CallData {
        // Getting authentication code from parameters
        authcode: match get_auth(REQUEST_NAME, &ip_address, &uuid, params.authcode, error_struct_xml) {
            GetStringResponse::Text(auth) => auth,
            GetStringResponse::Response(response) => return response
        },
        url: url,
        xmlns: xmlns,
        // Getting partner ID from parameters
        pid: match get_pid(REQUEST_NAME, &ip_address, &uuid, params.pid, error_struct_xml) {
            GetI64Response::Number(pid) => Some(pid),
            GetI64Response::Response(response) => return response
        },
        from_date: match get_date(REQUEST_NAME, &ip_address, &uuid, params.from_date, error_struct_xml, Some("from_date")) {
            GetDateResponse::DateTime(datetime) => Some(datetime),
            _ => None
        },
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}, pid: {:#?}", REQUEST_NAME, call_data.url, call_data.authcode, call_data.pid.unwrap()));

    // Getting data
    let xml = RequestGet::Bulk(call_data).to_xml().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Sending back xml as response
    send_xml(xml)
}


/// GET handler
#[get("/get-bulk")]
pub async fn get_handler(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
