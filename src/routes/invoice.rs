use actix_web::{
    get, HttpRequest, Responder,
    web::Query
};

use crate::{
    routes::default::{
        GetStringResponse, GetI64Response, GetDateResponse, RequestParameters,
        send_xml, send_csv, send_xlsx, return_internal_server_error,
        get_auth, get_url, get_xmlns, get_pid, get_i64, get_date
    },
    forms::{
        r#in::xml::defaults::CallData,
        out::{
            xml::invoices::error_struct_xml,
            csv::invoices::HU_HEADERS
        }
    },
    service::{
        slave::get_uuid,
        log::log_with_ip_uuid,
        ipv4::log_ip,
        get_data::{RequestGet, ResponseGet},
        dates::{get_first_date, is_min_date},
        get::invoices::{InvoicesData, InvoicesCSV}
    }
};

/// Name of the current request
const REQUEST_NAME: &str = "INVOICES REQUEST";

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
            GetI64Response::Number(num) => Some(num),
            GetI64Response::Response(response) => return response
        },
        // Getting `type_mod` from parameters
        type_mod: if let GetI64Response::Number(num) = get_i64(REQUEST_NAME, &ip_address, &uuid, params.type_mod, error_struct_xml, Some("type_mod")) {
            Some(num)
        } else {
            Some(1)
        },
        // Getting `from_date` from parameters
        from_date: match get_date(REQUEST_NAME, &ip_address, &uuid, params.from_date, error_struct_xml,  Some("from_date"), true) {
            GetDateResponse::DateTime(datetime) => Some(datetime),
            GetDateResponse::Response(response) => {
                let first_date = get_first_date();
                if is_min_date(&first_date) {
                    return response
                }
                Some(first_date)
            }
        },
        to_date: if let GetDateResponse::DateTime(datetime) = get_date(REQUEST_NAME, &ip_address, &uuid, params.to_date, error_struct_xml, Some("to_date"), true) {
            Some(datetime)
        } else {
            None
        },
        unpaid: if let GetI64Response::Number(num) = get_i64(REQUEST_NAME, &ip_address, &uuid, params.unpaid, error_struct_xml, Some("unpaid")) {
            Some(num)
        } else {
            Some(0)
        },
        language: params.language,
        data_type: params.data_type
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, {:?}", REQUEST_NAME, call_data));

    // Capturing language before `call_data` is consumed (drives CSV header language)
    let is_hu = call_data.is_hu();

    // Getting data
    let data = RequestGet::Invoices(call_data).into_data().await;

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    // Handling got data
    match data {
        ResponseGet::Invoices(InvoicesData::Xlsx(InvoicesCSV::En(d))) => return send_xlsx(&d.products, "invoices.csv", if is_hu { Some(HU_HEADERS) } else { None }),
        ResponseGet::Invoices(InvoicesData::Csv(InvoicesCSV::En(d))) => return send_csv(&d.products, "invoices.csv", if is_hu { Some(HU_HEADERS) } else { None }),
        ResponseGet::Invoices(InvoicesData::Xml(d)) => return send_xml(d.to_xml()),
        _ => {}
    }

    // Error if something went wrong at handling
    return_internal_server_error()
}


/// GET handler
#[get("/get-invoice")]
pub async fn get(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}


/// GET handler alias
#[get("/get-invoices")]
pub async fn get_alias(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}