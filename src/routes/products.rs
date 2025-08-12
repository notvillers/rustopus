use actix_web::{get, web, HttpRequest, Responder};

use crate::routes::default::{RequestParameters, GetStringResponse, GetDateResponse, get_auth, get_url, get_xmlns, send_xml, get_date};
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::service::ipv4::log_ip;
use crate::partner_xml::products::error_struct_xml;
use crate::o8_xml::defaults::CallData;
use crate::service::get_data::RequestGet;

/// Name of the current request
const REQUEST_NAME: &'static str = "PRODUCTS REQUEST";

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
        pid: None,
        from_date: match get_date(REQUEST_NAME, &ip_address, &uuid, params.from_date, error_struct_xml, Some("from_date")) {
            GetDateResponse::DateTime(datetime) => Some(datetime),
            _ => None
        },
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}", REQUEST_NAME, call_data.url, call_data.authcode));

    // Getting data
    let xml = RequestGet::Products(call_data).to_xml().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Sending back xml as reponse
    send_xml(xml)
}


/// GET handler
#[get("/get-products")]
pub async fn get_handler(req: HttpRequest, query: web::Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
