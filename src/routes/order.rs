use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use crate::routes::default::{
    RequestParameters,
    send_xml
};
use crate::forms::{
    r#in::xml::orders::Order,
    out::xml::orders::Rendeles
};
use crate::service::{
    slave::get_uuid,
    log::log_with_ip_uuid,
    ipv4::log_ip,
    get_data::to_xml_string
};

const REQUEST_NAME: &str = "ORDER SUBMISSION";

fn lowercase_xml_tags(xml: &str) -> String {
        let re = regex::Regex::new(r"<(/?)([A-Za-z][A-Za-z0-9_\-.]*)").unwrap();
    re.replace_all(xml, |c: &regex::Captures| {
        format!("<{}{}", &c[1], c[2].to_lowercase())
    }).to_string()
}

async fn handler(req: HttpRequest, _: RequestParameters, body: web::Bytes) -> impl Responder {
    let uuid = get_uuid();
    let ip = log_ip(req).await.to_string();

    // 1. Decode body
    let raw = match std::str::from_utf8(&body) {
        Ok(s) => lowercase_xml_tags(s),
        Err(_) => return HttpResponse::BadRequest()
            .content_type("application/xml")
            .body("<error><message>Body is not valid UTF-8</message></error>")
    };

    // 2. Parse into Order struct
    let order: Order = match quick_xml::de::from_str(&raw) {
        Ok(o) => o,
        Err(e) => {
            log_with_ip_uuid(&ip, &uuid, format!("{REQUEST_NAME}: parse error: {e}"));
            return HttpResponse::BadRequest()
                .content_type("application/xml")
                .body(format!("<error><message>Invalid XML: {e}</message></error>"));
        }
    };

    send_xml(to_xml_string(&Into::<Rendeles>::into(order)))
}


#[post("/post-order")]
pub async fn post_handler(
    req: HttpRequest,
    query: web::Query<RequestParameters>,
    body: web::Bytes,
) -> impl Responder {
    handler(req, query.into_inner(), body).await
}
