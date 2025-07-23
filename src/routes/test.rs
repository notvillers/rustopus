use actix_web::{get, HttpRequest, Responder};
use crate::partner_xml::test::{Envelope, create_xml};
use crate::service::slave::get_uuid;
use crate::service::ipv4::log_ip;
use crate::routes::default::send_xml;

/// Handler
async fn handler(req: HttpRequest) -> impl Responder {
    // ID with UUID
    let uuid = get_uuid();

    // IP address of the request
    let ip_address = log_ip(req).await;

    // Getting data
    let xml = create_xml(Envelope::load(None, Some(ip_address), Some(uuid), None));

    // Sending back xml as response
    send_xml(xml)
}


/// GET handler
#[get("/get-test")]
async fn get_handler(req: HttpRequest) -> impl Responder {
    handler(req).await
}
