use actix_web::{
    get, HttpRequest, Responder,
    web::Query
};

use crate::{
    routes::default::{
        RequestParameters,
        send_xml, send_csv, send_xlsx
    },
    forms::{
        r#in::xml::defaults::CallData,
        out::{
            xml::test::{
                Envelope,
                create_xml
            },
            csv::test::Data
        }
    },
    global::errors,
    service::{
        slave::get_uuid,
        ipv4::{RequestIP, log_ip},
        log::log_with_ip_uuid
    }
};

/// Name of the current request
const REQUEST_NAME: &str = "TEST REQUEST";

/// Handler
async fn handler(req: HttpRequest, params: RequestParameters) -> impl Responder {
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

    let call_data = CallData {
        data_type: params.data_type,
        ..Default::default()
    };

    // Before log
    log_with_ip_uuid(&ip_address.to_string(), &uuid, format!("Before getting {}", REQUEST_NAME));

    // Getting data
    let envelope: Envelope = (None, Some(ip_address.to_string()), Some(uuid.clone()), error).into();

    // After log
    log_with_ip_uuid(&ip_address.to_string(), &uuid, format!("After {} got.", REQUEST_NAME));

    // Sending back xml as response
    match (call_data.is_csv(), call_data.is_xlsx()) {
        (true, _) => send_csv(&[Data::from(envelope)], "test.csv", None),
        (_, true) => send_xlsx(&[Data::from(envelope)], "test.xlsx", None),
        _ => send_xml(create_xml(envelope))
    }
}


/// GET handler
#[get("/get-test")]
async fn get_handler(req: HttpRequest, query: Query<RequestParameters>) -> impl Responder {
    handler(req, query.into_inner()).await
}
