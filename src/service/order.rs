//! Order submission, shared by the `/post-order` route and (later) the MCP
//! `submit_order` tool.
//!
//! Callers build an [`Order`] however their input arrives — XML body, tool
//! arguments — and hand it here. This module owns everything from there on:
//! country translation, the Hungarian document, the SOAP call and the answer.
//! It never retries and never coalesces: an order is not idempotent, and
//! Octopus does not reject a duplicate.

use std::time::Duration;

use crate::{
    forms::{
        r#in::xml::{
            orders::Order,
            orders_response::Envelope
        },
        out::xml::orders::{Rendeles, get_request_string}
    },
    language::countries::order_country_to_hu,
    service::{
        get_data::to_xml_string,
        mcp::mask_authcode,
        soap::{self, SendError}
    }
};

/// What became of a submitted order, from the caller's point of view.
pub enum Outcome {
    /// Octopus answered. The answer may still be a refusal (`hiba`) or carry
    /// unrecorded lines — that is for the caller to read, not a failure here.
    Answered(Box<Envelope>),
    /// Octopus never saw the request. Safe to submit again.
    NotSent(String),
    /// The request may have reached Octopus, but no answer came back.
    /// Whether the order was recorded is unknown.
    NoAnswer(String),
    /// Octopus answered with something that is not an order response.
    /// Whether the order was recorded is unknown.
    Unreadable(String)
}

/// Collapses the whitespace between tags so a document fits one log line.
pub fn to_single_line(xml: &str) -> String {
    let re = match regex::Regex::new(r">\s+<") {
        Ok(regex) => regex,
        Err(e) => {
            eprintln!("`to_single_line`: {}", e);
            return xml.into()
        }
    };
    re.replace_all(xml.trim(), "> <").to_string()
}

/// Submits `order` and waits for Octopus's answer.
///
/// `timeout` overrides the client-wide `[server] timeout` for this call (`None`
/// keeps it). `log` receives each step; nothing passed to it contains the full
/// authcode — Octopus echoes an unknown code back in its error text, so the
/// raw answer is masked before it is logged.
pub async fn submit(
    order: Order,
    url: &str,
    xmlns: &str,
    authcode: &str,
    timeout: Option<Duration>,
    log: impl Fn(String)
) -> Outcome {
    let order_hu: Rendeles = order_country_to_hu(order).into();
    let order_hu_xml = to_xml_string(&order_hu);
    log(format!("formatted to: {}", to_single_line(&order_hu_xml)));

    let masked = mask_authcode(authcode);
    log(format!("sending to {url} as {masked}"));

    let request = get_request_string(xmlns, &order_hu_xml, authcode);
    let response = match soap::send(url, request, timeout).await {
        Ok(response) => response,
        Err(SendError::NotSent(reason)) => {
            log(format!("not sent: {reason}"));
            return Outcome::NotSent(reason)
        },
        Err(SendError::NoAnswer(reason)) => {
            log(format!("no answer, outcome unknown: {reason}"));
            return Outcome::NoAnswer(reason)
        }
    };

    log(format!("response: {}", response.replace(authcode, &masked)));

    match quick_xml::de::from_str::<Envelope>(&response) {
        Ok(envelope) => Outcome::Answered(Box::new(envelope)),
        Err(e) => {
            log(format!("response parse error, outcome unknown: {e}"));
            Outcome::Unreadable(e.to_string())
        }
    }
}
