use actix_web::{get, web::Query, HttpRequest, Responder};
use crate::routes::default::{
    RequestParameters, GetStringResponse, GetI64Response, GetDateResponse,
    send_xml, send_csv, return_internal_server_error,
    get_auth, get_url, get_xmlns, get_pid, get_date
};
use crate::forms::{
    r#in::xml::defaults::CallData,
    out::xml::bulk::error_struct_xml
};
use crate::service::{
    slave::get_uuid,
    log::log_with_ip_uuid,
    ipv4::log_ip,
    get_data::{RequestGet, ResponseGet},
    get::bulk::{BulkData, BulkCSV}
};

/// Name of the current request
const REQUEST_NAME: &'static str = "BULK REQUEST";

/// Handler
async fn handler(req: HttpRequest, params: RequestParameters) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await.to_string();

    // Trying to get url from parameters
    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
        GetStringResponse::Text(url) => url,
        GetStringResponse::Response(response) => return response
    };

    // Getting XMLNS from parameters, otherwise using url
    let xmlns = get_xmlns(&params, &url);

    // Creating call data from parameters
    let call_data = CallData {
        // Getting authentication code from parameters
        authcode: match get_auth(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
            GetStringResponse::Text(auth) => auth,
            GetStringResponse::Response(response) => return response
        },
        url: url,
        xmlns: xmlns,
        // Getting partner ID from parameters
        pid: match get_pid(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
            GetI64Response::Number(pid) => Some(pid),
            GetI64Response::Response(response) => return response
        },
        from_date: if let GetDateResponse::DateTime(datetime) = get_date(REQUEST_NAME, &ip_address, &uuid, params.from_date, error_struct_xml, Some("from_date"), true) {
            Some(datetime)
        } else {
            None
        },
        data_type: params.data_type,
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}, pid: {:#?}", REQUEST_NAME, call_data.url, call_data.authcode, call_data.pid.unwrap_or(0)));

    // Getting data
    let data = RequestGet::Bulk(call_data).to_data().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Handling got data
    match data {
        ResponseGet::Bulk(BulkData::CSV(BulkCSV::En(d))) => send_csv(&d.products, "bulk.csv"),
        ResponseGet::Bulk(BulkData::XML(d)) => send_xml(d.to_xml()),
        _ => return_internal_server_error()
    }
}


/// GET handler
#[get("/get-bulk")]
pub async fn get_handler(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
