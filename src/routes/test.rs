use actix_web::{get, HttpRequest, Responder};
use crate::partner_xml::test::{Envelope, create_xml};
use crate::service::slave::get_uuid;
use crate::service::ipv4::log_ip;
use crate::routes::default::send_xml;

async fn test_handler(req: HttpRequest) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await;

    let xml = create_xml(Envelope::load(None, Some(ip_address), Some(uuid), None));

    send_xml(xml)
}


#[get("/get-test")]
async fn get_test(req: HttpRequest) -> impl Responder {
    test_handler(req).await
}
