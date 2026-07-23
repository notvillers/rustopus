use actix_web::{
    get, HttpRequest, Responder,
    web::Query
};

use crate::{
    routes::default::{
        RequestParameters, GetStringResponse, GetI64Response,
        send_xml, send_csv, send_xlsx, return_internal_server_error,
        get_auth, get_url, get_xmlns, get_pid
    },
    forms::{
        r#in::xml::defaults::CallData,
        out::{
            xml::prices::error_struct_xml,
            csv::prices::HU_HEADERS
        }
    },
    service::{
        slave::get_uuid,
        log::log_with_ip_uuid,
        ipv4::log_ip,
        get_data::{RequestGet, ResponseGet},
        get::prices::{PricesData, PricesCSV}
    }
};

/// Name of the current request
const REQUEST_NAME: &str = "PRICES REQUEST";

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
        url,
        xmlns,
        // Getting partner ID from parameters
        pid: match get_pid(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
            GetI64Response::Number(pid) => Some(pid),
            GetI64Response::Response(response) => return response
        },
        language: params.language,
        data_type: params.data_type,
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, {:?}", REQUEST_NAME, call_data));

    // Capturing language before `call_data` is consumed (drives CSV header language)
    let is_hu = call_data.is_hu();

    // Getting data
    let data = RequestGet::Prices(call_data).to_data().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Handling got data
    match data {
        ResponseGet::Prices(PricesData::XLSX(PricesCSV::En(d))) => send_xlsx(&d.prices, "prices.xlsx", if is_hu { Some(HU_HEADERS) } else { None }),
        ResponseGet::Prices(PricesData::CSV(PricesCSV::En(d))) => send_csv(&d.prices, "prices.csv", if is_hu { Some(HU_HEADERS) } else { None }),
        ResponseGet::Prices(PricesData::XML(d)) => send_xml(d.to_xml()),
        _ => return_internal_server_error()
    }
}


/// GET handler
#[get("/get-price")]
async fn get(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}


/// GET handler alias
#[get("/get-prices")]
async fn get_alias(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}