use actix_web::{
    post, HttpRequest, HttpResponse, Responder,
    web::{Query, Bytes}
};
use quick_xml::escape::escape;

use crate::{
    routes::default::{
        RequestParameters, GetStringResponse,
        get_url, get_xmlns, get_auth,
        send_xml
    },
    forms::{
        r#in::xml::orders::Order,
        out::xml::{
            orders::error_struct_xml,
            orders_response::Envelope as p_Envelope
        }
    },
    service::{
        slave::get_uuid,
        log::log_with_ip_uuid,
        ipv4::log_ip,
        get_data::to_xml_string,
        order::{self, Outcome, to_single_line}
    }
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


/// An `<error>` document for the cases where Octopus gave no order answer.
fn error_response(mut response: actix_web::HttpResponseBuilder, message: String) -> HttpResponse {
    response
        .content_type("application/xml")
        .body(format!("<error><message>{}</message></error>", escape(&message)))
}

const REQUEST_NAME: &str = "ORDER SUBMISSION";

async fn handler(req: HttpRequest, params: RequestParameters, body: Bytes) -> impl Responder {
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

    // Deriving XMLNS from the url; the parameter is only a fallback
    let xmlns = get_xmlns(REQUEST_NAME, &ip_address, &uuid, &params, &url);

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
            return error_response(HttpResponse::BadRequest(), format!("Invalid XML: {e}"))
        }
    };

    // 3. Submit and wait for Octopus. `None` keeps the client-wide timeout,
    //    which is what REST callers have always had.
    let log = |message: String| log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: {message}"));
    let envelope = match order::submit(order, &url, &xmlns, &authcode, None, log).await {
        Outcome::Answered(envelope) => *envelope,
        Outcome::NotSent(reason) => return error_response(
            HttpResponse::BadGateway(),
            format!("The order was not sent to Octopus ({reason}). It is safe to submit it again.")
        ),
        Outcome::NoAnswer(reason) => return error_response(
            HttpResponse::GatewayTimeout(),
            format!("No answer from Octopus ({reason}). The order may have been recorded: check before submitting it again.")
        ),
        Outcome::Unreadable(reason) => return error_response(
            HttpResponse::InternalServerError(),
            format!("Failed to parse Octopus response ({reason}). The order may have been recorded: check before submitting it again.")
        )
    };

    // 4. Convert response to English and send it back
    let response_trans: p_Envelope = envelope.into();
    let response_xml = to_xml_string(&response_trans);

    log_with_ip_uuid(&ip_address, &uuid, format!("{REQUEST_NAME}: converted to English response: {}", to_single_line(&response_xml)));

    send_xml(response_xml)
}


/// POST handler
#[post("/post-order")]
pub async fn post(req: HttpRequest, query: Query<RequestParameters>, body: Bytes, ) -> impl Responder {
    handler(req, query.into_inner(), body).await
}


/// POST handler alias
#[post("/post-orders")]
pub async fn post_alias(req: HttpRequest, query: Query<RequestParameters>, body: Bytes, ) -> impl Responder {
    handler(req, query.into_inner(), body).await
}
