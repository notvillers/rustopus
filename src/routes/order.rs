use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use crate::routes::default::{
    RequestParameters, GetStringResponse,
    get_url, get_xmlns, get_auth,
    send_xml
};
use crate::forms::{
    r#in::xml::orders::Order,
    out::xml::orders::{Rendeles, get_request_string, error_struct_xml}
};
use crate::service::{
    slave::get_uuid,
    log::log_with_ip_uuid,
    ipv4::log_ip,
    get_data::to_xml_string,
    soap::get_response
};

const REQUEST_NAME: &str = "ORDER SUBMISSION";

fn lowercase_xml_tags(xml: &str) -> String {
        let re = regex::Regex::new(r"<(/?)([A-Za-z][A-Za-z0-9_\-.]*)").unwrap();
    re.replace_all(xml, |c: &regex::Captures| {
        format!("<{}{}", &c[1], c[2].to_lowercase())
    }).to_string()
}


fn to_single_line(xml: &str) -> String {
    let re = regex::Regex::new(r">\s+<").unwrap();
    re.replace_all(xml.trim(), "> <").to_string()
}


async fn handler(req: HttpRequest, params: RequestParameters, body: web::Bytes) -> impl Responder {
    let uuid = get_uuid();
    let ip_address = log_ip(req).await.to_string();

    let authcode = match get_auth(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
        GetStringResponse::Text(auth) => auth,
        GetStringResponse::Response(response) => return response
    };

    // Trying to get url from parameters
    let url = match get_url(REQUEST_NAME, &ip_address, &uuid, &params, error_struct_xml) {
        GetStringResponse::Text(url) => url,
        GetStringResponse::Response(response) => return response
    };

    // Getting XMLNS from parameters, otherwise using url
    let xmlns = get_xmlns(&params, &url);

    // 1. Decode body
    let raw = match std::str::from_utf8(&body) {
        Ok(s) => lowercase_xml_tags(s),
        Err(_) => return HttpResponse::BadRequest()
            .content_type("application/xml")
            .body("<error><message>Body is not valid UTF-8</message></error>")
    };

    log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: received: {}", to_single_line(&raw)));

    // 2. Parse into Order struct
    let order: Order = match quick_xml::de::from_str(&raw) {
        Ok(o) => o,
        Err(e) => {
            log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: parse error: {e}"));
            return HttpResponse::BadRequest()
                .content_type("application/xml")
                .body(format!("<error><message>Invalid XML: {e}</message></error>"));
        }
    };

    let order_hu: Rendeles = order.into();
    let order_hu_xml_string = to_xml_string(&order_hu);
    log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: formatted to: {}", to_single_line(&order_hu_xml_string)));

    let request = get_request_string(&xmlns, &order_hu_xml_string, &authcode);
    log_with_ip_uuid(&ip_address, &uuid, format!("Request: {}", request));

    let response = get_response(&url, request).await;
    log_with_ip_uuid(&ip_address, &uuid, format!("Response: {}", response));
    
    send_xml(response)
}


#[post("/post-order")]
pub async fn post_handler(req: HttpRequest, query: web::Query<RequestParameters>, body: web::Bytes, ) -> impl Responder {
    handler(req, query.into_inner(), body).await
}
