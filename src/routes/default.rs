use chrono::{DateTime, Utc};
use actix_web::HttpResponse;
use serde::Deserialize;

use crate::global::errors;
use crate::service::log::{log_with_ip_uuid, elog_with_ip_uuid};
use crate::service::soap_config::get_default_url;

#[derive(Deserialize)]
pub struct RequestParameters {
    pub authcode: Option<String>,
    pub auth: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>,
    pub type_mod: Option<i64>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub unpaid: Option<i64>
}


pub enum GetStringResponse {
    Text(String),
    Response(actix_web::HttpResponse)
}


pub enum GetI64Response {
    Number(i64),
    Response(actix_web::HttpResponse)
}


pub enum GetDateResponse {
    DateTime(DateTime<Utc>),
    Response(actix_web::HttpResponse)
}


pub fn send_xml(xml: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


/// Tries to get authentication from the parameter, sends back error xml on fail
pub fn get_auth(request_name: &str, ip_address: &str, uuid: &str, params: &RequestParameters, send_error_xml_fn: fn(u64, &str) -> String) -> GetStringResponse {
    if let Some(s) = params.authcode.as_ref().filter(|x| !x.trim().is_empty()) {
        return GetStringResponse::Text(s.to_string())
    }
    if let Some(s) = params.auth.as_ref().filter(|x| !x.trim().is_empty()) {
        return GetStringResponse::Text(s.to_string())
    }
    let error = errors::GLOBAL_AUTH_ERROR;
    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
    GetStringResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
}


/// Tries to get url from the parameter, if not found, then tries to get default url from the `./soap.json` file, sends back error xml on fail
pub fn get_url(request_name: &str, ip_address: &str, uuid: &str, params: &RequestParameters, send_error_xml_fn: fn(u64, &str) -> String) -> GetStringResponse {
    if let Some(s) = params.url.as_ref().filter(|x| !x.trim().is_empty()) {
        return GetStringResponse::Text(s.into())
    }
    if let Some(s) = get_default_url() {
        log_with_ip_uuid(ip_address, uuid, format!("Using default url: '{}'", s));
        return GetStringResponse::Text(s)
    }
    let error = errors::GLOBAL_URL_ERROR;
    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
    GetStringResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
}


/// Tries to get xmlns from parameter, if not found, then using url parameter
pub fn get_xmlns(params: &RequestParameters, url: &str) -> String {
    let serv_str = "/services/";
    let mut xmlns = params.xmlns.clone().unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains(serv_str) {
        if let Some(pos) = url.find(serv_str) {
            let end = pos + serv_str.len();
            xmlns = url[..end].to_string();
        }
    }
    return xmlns
}


/// Tries to get pid (Partner ID) from parameter, sends back error xml on fail
pub fn get_pid(request_name: &str, ip_address: &str, uuid: &str, params: &RequestParameters, send_error_xml_fn: fn(u64, &str) -> String) -> GetI64Response {
    if let Some(s) = params.pid {
        return GetI64Response::Number(s)
    }
    let error = errors::GLOBAL_PID_ERROR;
    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
    GetI64Response::Response(send_xml(send_error_xml_fn(error.code, error.description)))
}


/// Tries to get date from parameter, sends back error xml on fail
pub fn get_date(request_name: &str, ip_address: &str, uuid: &str, param: Option<DateTime<Utc>>, send_error_xml_fn: fn(u64, &str) -> String, param_name: Option<&str>) -> GetDateResponse {
    if let Some(s) = param {
        return GetDateResponse::DateTime(s)
    }
    let error = errors::GLOBAL_MISSING_ERROR;
    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} -> {} ({})", error.code, error.description, param_name.unwrap_or("_"), request_name));
    GetDateResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
}


/// Tries to get i64 from parameter, send back error xml on fail
pub fn get_i64(request_name: &str, ip_address: &str, uuid: &str, param: Option<i64>, send_error_xml_fn: fn(u64, &str) -> String, param_name: Option<&str>) -> GetI64Response {
    if let Some(s) = param {
        return GetI64Response::Number(s)
    }
    let error = errors::GLOBAL_MISSING_ERROR;
    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} -> {} ({})", error.code, error.description, param_name.unwrap_or("_"), request_name));
    GetI64Response::Response(send_xml(send_error_xml_fn(error.code, error.description)))
}
