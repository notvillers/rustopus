use actix_web::{get, Responder};
use crate::routes::default::send_xml;

#[get("/get-test")]
async fn get_test() -> impl Responder {
    send_xml("<Envelope>OK</Envelope>".to_string())
}
