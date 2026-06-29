use actix_web::{
    get, HttpRequest, Responder,
    web::Query
};

use crate::{
    routes::default::{
        RequestParameters, GetStringResponse, GetDateResponse,
        send_xml, send_csv, return_internal_server_error,
        get_auth, get_date, get_url, get_xmlns
    },
    forms::{
        r#in::xml::defaults::CallData,
        out::{
            xml::images::error_struct_xml,
            csv::images::HU_HEADERS
        }
    },
    service::{
        slave::get_uuid,
        log::log_with_ip_uuid,
        ipv4::log_ip,
        get_data::{RequestGet, ResponseGet},
        get::images::{ImagesData, ImagesCSV}
    }
};

/// Name of the current request
const REQUEST_NAME: &'static str = "IMAGES REQUEST";

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
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, {:?}", REQUEST_NAME, call_data));

    // Capturing language before `call_data` is consumed (drives CSV header language)
    let is_hu = call_data.is_hu();

    // Getting data
    let data = RequestGet::Images(call_data).to_data().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Handling got data
    match data {
        ResponseGet::Images(ImagesData::CSV(ImagesCSV::En(d))) => send_csv(&d.products, "images.csv", if is_hu { Some(HU_HEADERS) } else { None }),
        ResponseGet::Images(ImagesData::XML(d)) => send_xml(d.to_xml()),
        _ => return_internal_server_error()
    }
}


/// GET handler
#[get("/get-images")]
pub async fn get_handler(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
