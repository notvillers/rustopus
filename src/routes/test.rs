use actix_web::{get, HttpRequest, Responder};

use crate::routes::default::send_xml;
use crate::service::slave::get_uuid;
use crate::service::ipv4::{log_ip, RequestIP};
use crate::service::log::log_with_ip_uuid;
use crate::partner_xml::test::create_xml;
use crate::global::errors;

/// Name of the current request
const REQUEST_NAME: &'static str = "TEST REQUEST";

/// Handler
async fn handler(req: HttpRequest) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await;

    // Error if can not get IP
    let error = if let RequestIP::Err(_) = ip_address {
        Some(errors::UNDEFINED_ERROR.into())
    } else {
        None
    };

    // Before log
    log_with_ip_uuid(&ip_address.to_string(), &uuid, format!("Before getting {}", REQUEST_NAME));

    // Getting data
    let xml = create_xml((None, Some(ip_address.to_string()), Some(uuid.clone()), error).into());

    // After log
    log_with_ip_uuid(&ip_address.to_string(), &uuid, format!("After {} got.", REQUEST_NAME));

    // Sending back xml as response
    send_xml(xml)
}


/// GET handler
#[get("/get-test")]
async fn get_handler(req: HttpRequest) -> impl Responder {
    handler(req).await
}
