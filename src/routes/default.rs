use actix_web::HttpResponse;
use crate::global::errors;
use crate::service::log::{log_with_ip_uuid, elog_with_ip_uuid};
use crate::service::soap_config::get_default_url;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct RequestParameters {
    pub authcode: Option<String>,
    pub url: Option<String>,
    pub xmlns: Option<String>,
    pub pid: Option<i64>
}


pub enum GetResponse {
    Text(String),
    Response(actix_web::HttpResponse)
}


pub enum GetPidResponse {
    Number(i64),
    Response(actix_web::HttpResponse)
}


pub fn send_xml(xml: String) -> HttpResponse {
    HttpResponse::Ok()
        .content_type("application/xml")
        .body(xml)
}


pub fn get_auth(request_name: &str, ip_address: &str, uuid: &str, param: Option<String>, send_error_xml_fn: fn(u64, &str) -> String) -> GetResponse {
    match param {
        Some(ref s) if !s.trim().is_empty() => GetResponse::Text(s.to_string()),
        _ => {
            let error = errors::GLOBAL_AUTH_ERROR;
            elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
            GetResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
        }
    }
}


pub fn get_url(request_name: &str, ip_address: &str, uuid: &str, param: Option<String>, send_error_xml_fn: fn(u64, &str) -> String) -> GetResponse {
    match param {
        Some(ref s) if !s.trim().is_empty() => GetResponse::Text(s.to_string()),
        _ => {
            match get_default_url() {
                Some(default_url) => {
                    log_with_ip_uuid(ip_address, uuid, format!("Using default url: '{}'", default_url));
                    GetResponse::Text(default_url)
                }
                _ => {
                    let error = errors::GLOBAL_URL_ERROR;
                    elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
                    GetResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
                }
            }
        }
    }
}


pub fn get_xmlns(param: Option<String>, url: &str) -> String {
    let serv_str = "/services/";
    let mut xmlns = param.unwrap_or_default();
    if xmlns.trim().is_empty() && url.contains(serv_str) {
        if let Some(pos) = url.find(serv_str) {
            let end = pos + serv_str.len();
            xmlns = url[..end].to_string();
        }
    }
    return xmlns
}


pub fn get_pid(request_name: &str, ip_address: &str, uuid: &str, param: Option<i64>, send_error_xml_fn: fn(u64, &str) -> String) -> GetPidResponse {
    match param {
        Some(ref s) => GetPidResponse::Number(*s),
        _ => {
            let error = errors::GLOBAL_PID_ERROR;
            elog_with_ip_uuid(ip_address, uuid, format!("{}: {} ({})", error.code, error.description, request_name));
            GetPidResponse::Response(send_xml(send_error_xml_fn(error.code, error.description)))
        }
    }
}
