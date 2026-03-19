use actix_web::{get, web, HttpRequest, Responder};

use crate::routes::default::{RequestParameters, GetStringResponse, GetDateResponse, get_auth, get_url, get_xmlns, get_date};
use crate::service::slave::get_uuid;
use crate::service::log::log_with_ip_uuid;
use crate::service::ipv4::log_ip;
use crate::partner_xml::products::error_struct_xml;
use crate::o8_xml::defaults::CallData;
use actix_web::HttpResponse;

use crate::o8_xml::products::{self as o8_products};
use crate::partner_csv::products::{self as csv_products, Products};
use crate::service::get_data::FIRST_DATE;
use crate::service::soap::get_response;

/// Name of the current request
const REQUEST_NAME: &'static str = "PRODUCTS REQUEST (CSV)";

pub fn csv_response<T: serde::Serialize>(records: &[T], filename: &str) -> HttpResponse {
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_writer(vec![]);

    for record in records {
        wtr.serialize(record).unwrap();
    }
    let data = wtr.into_inner().unwrap();

    HttpResponse::Ok()
        .content_type("text/csv")
        .insert_header(("Content-Disposition", format!("attachment; filename=\"{}\"", filename)))
        .body(data)
}


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
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address, &uuid, format!("Before getting {}, url: {}, auth: {}", REQUEST_NAME, call_data.url, call_data.authcode));

    // csvtest
    let request  = o8_products::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = get_response(&call_data.url, request).await;
    let prods = match quick_xml::de::from_str::<o8_products::Envelope>(&response) {
        Ok(envelope) => Some(envelope),
        Err(_) => None
    };
    let csv_prods: csv_products::Products = match prods {
        Some(p) => p.into(),
        _ => Products {
            products: vec![]
        }
    };

    // After log
    log_with_ip_uuid(&ip_address, &uuid, format!("After {} got", REQUEST_NAME));

    csv_response(&csv_prods.products, "products.csv")
}


/// GET handler
#[get("/get-csv")]
pub async fn get_handler(req: HttpRequest, query: web::Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
