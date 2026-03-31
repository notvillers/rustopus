use actix_web::{post, web, HttpRequest, HttpResponse, Responder};
use crate::routes::default::{
    RequestParameters, GetStringResponse,
    get_url, get_xmlns, get_auth,
    send_xml
};
use crate::forms::{
    r#in::xml::{
        orders::Order,
        orders_response::Envelope
    },
    out::xml::{
        orders::{
            Rendeles,
            get_request_string, error_struct_xml
        },
        orders_response::Envelope as p_Envelope
    }
};
use crate::service::{
    slave::get_uuid,
    log::log_with_ip_uuid,
    ipv4::log_ip,
    get_data::to_xml_string,
    soap::get_response
};

fn lowercase_xml_tags(xml: &str) -> String {
        let re = match regex::Regex::new(r"<(/?)([A-Za-z][A-Za-z0-9_\-.]*)") {
            Ok(regex) => regex,
            Err(e) => {
                eprintln!("`lowercase_xml_tags`: {}", e);
                return xml.into()
            }
        };
    re.replace_all(xml, |c: &regex::Captures| {
        format!("<{}{}", &c[1], c[2].to_lowercase())
    }).to_string()
}


fn to_single_line(xml: &str) -> String {
    let re = match regex::Regex::new(r">\s+<") {
        Ok(regex) => regex,
        Err(e) => {
            eprintln!("`to_single_line`: {}", e);
            return xml.into()
        }
    };
    re.replace_all(xml.trim(), "> <").to_string()
}

const REQUEST_NAME: &str = "ORDER SUBMISSION";

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

    // 2. Parse into `Order`
    let order: Order = match quick_xml::de::from_str(&raw) {
        Ok(o) => o,
        Err(e) => {
            log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: parse error: {e}"));
            return HttpResponse::BadRequest()
                .content_type("application/xml")
                .body(format!("<error><message>Invalid XML: {e}</message></error>"));
        }
    };

    // 3. Convert `Order` to `Rendeles`
    let order_hu: Rendeles = order.into();
    let order_hu_xml_string = to_xml_string(&order_hu);
    log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: formatted to: {}", to_single_line(&order_hu_xml_string)));

    // 4. Get the request string
    let request = get_request_string(&xmlns, &order_hu_xml_string, &authcode);
    log_with_ip_uuid(&ip_address, &uuid, format!("Request: {}", request));

    // 5. Gets the response string from Octopus
    let response_str = get_response(&url, request).await;
    log_with_ip_uuid(&ip_address, &uuid, format!("Response: {}", response_str));

    // 6. Deserialize SOAP response into Envelope
    let envelope: Envelope = match quick_xml::de::from_str(&response_str) {
        Ok(e) => e,
        Err(e) => {
            log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: response parse error: {e}"));
            return HttpResponse::InternalServerError()
                .content_type("application/xml")
                .body(format!("<error><message>Failed to parse Octopus response: {e}</message></error>"));
        }
    };

    // 7. Convert response to `p_Envelope` and then to raw XML string
    let response_trans: p_Envelope = envelope.into();
    let response_xml = to_xml_string(&response_trans);

    log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: converted to English response: {}", to_single_line(&response_xml)));

    // 8. Send back English XML response to client
    send_xml(response_xml)
}


#[post("/post-order")]
pub async fn post_handler(req: HttpRequest, query: web::Query<RequestParameters>, body: web::Bytes, ) -> impl Responder {
    handler(req, query.into_inner(), body).await
}
