use actix_web::{get, web, HttpRequest, Responder};

use crate::routes::default::{
    RequestParameters, GetStringResponse, GetDateResponse,
    send_xml, send_csv, return_internal_server_error,
    get_auth, get_url, get_xmlns, get_date
};
use crate::forms::{
    r#in::xml::defaults::CallData,
    out::xml::stocks::error_struct_xml
};
use crate::service::{
    slave::get_uuid,
    log::log_with_ip_uuid,
    ipv4::log_ip,
    get_data::{RequestGet, ResponseGet},
    get::stocks::{StocksData, StocksCSV}
};

/// Name of the current request
const REQUEST_NAME: &'static str = "STOCKS REQUEST";

/// Handler
async fn handler(req: HttpRequest, params: RequestParameters) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await.to_string();

    // Trying to get url from parameters
    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
        GetStringResponse::Text(url) => url,
        GetStringResponse::Response(response) => return response // Error response if something went wrong
    };

    // Getting XMLNS from parameters, otherwise using url
    let xmlns = get_xmlns(&params, &url);

    // Creating call data from parameters
    let call_data = CallData {
        // Getting authentication code from parameters
        authcode: match get_auth(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
            GetStringResponse::Text(auth) => auth,
            GetStringResponse::Response(response) => return response // Error response if something went wrong
        },
        url: url,
        xmlns: xmlns,
        pid: None,
        // Getting `from_date` from parameters
        from_date: if let GetDateResponse::DateTime(datetime) = get_date(REQUEST_NAME, &ip_address, &uuid, params.from_date, error_struct_xml, Some("from_date"), true) {
            Some(datetime)
        } else {
            None
        },
        language: params.language,
        data_type: params.data_type,
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}", REQUEST_NAME, call_data.url, call_data.authcode));
    if call_data.clone().is_hu() {
        log_with_ip_uuid(&ip_address, &uuid, format!("Request is hungarian ('{}')", call_data.clone().language.unwrap_or("Err.".to_string())));
    }
    if call_data.clone().is_csv() {
        log_with_ip_uuid(&ip_address, &uuid, format!("Request is csv ('{}')", call_data.clone().data_type.unwrap_or("Err.".to_string())));
    }

    // Getting data
    let data = RequestGet::Stocks(call_data).to_data().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Handling got data
    match data {
        ResponseGet::Stocks(StocksData::CSV(StocksCSV::En(d))) => return send_csv(&d.products, "stocks.csv"),
        ResponseGet::Stocks(StocksData::XML(d)) => return send_xml(d.to_xml()),
        _ => {}
    }

    // Error if something went wrong at handling
    return_internal_server_error()
}


/// GET handler
#[get("/get-stocks")]
async fn get_handler(req: HttpRequest, query: web::Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
