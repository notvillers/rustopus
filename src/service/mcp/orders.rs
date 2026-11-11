//! The MCP `submit_order` tool: argument shape, validation, the duplicate
//! guard, and the summary handed back to the model.
//!
//! The SOAP call itself is `service::order::submit`, the same one `/post-order`
//! uses — this module only turns tool arguments into an [`Order`] and turns the
//! answer into something a model can relay line by line.
//!
//! Three rules shape it:
//!
//! - **The tool waits for Octopus's answer** and reports it per line: Octopus
//!   records some lines and refuses others (an unknown article comes back as
//!   "Nem rögzített"), so "the order went through" is never the whole story.
//! - **An unknown outcome is never reported as a failure.** A timeout may follow
//!   a recorded order, and Octopus does not reject duplicates, so the tool says
//!   "unknown — check before resubmitting", never "failed — try again".
//! - **The same order twice within [`DUPLICATE_WINDOW`] is refused** unless the
//!   caller explicitly confirms it. That is the guard against a model or a
//!   bridge retrying a call whose first attempt did go through.
//!
//! It does not read the catalog snapshot. An order must not depend on a cache
//! that may be cold (a cold build outlasts any client's patience) or stale (a
//! product added since the last refresh is still orderable); Octopus checks the
//! article numbers itself and the answer says which lines it refused.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use chrono::Utc;
use once_cell::sync::Lazy;
use rmcp::model::{CallToolResult, Content};
use serde_json::{Value, json};

use crate::{
    forms::{
        r#in::xml::{
            orders::{Address, Header, Item, Items, Order},
            orders_response::{Envelope, Valasz}
        },
        out::xml::defaults::Error as EnglishError
    },
    language::countries::COUNTRIES,
    macros::mcp::McpToolArgs,
    routes::default::derive_xmlns,
    service::{
        config::get_mcp_settings,
        log::{elogger, logger},
        mcp::{McpAuth, tools::lenient_count},
        order::{self, Outcome}
    }
};

/// Most lines one tool call may submit. A guard against a runaway argument, not
/// a business rule — Octopus sets no such limit that we know of.
pub const MAX_LINES: usize = 200;

/// How long an order is remembered for the duplicate guard.
pub const DUPLICATE_WINDOW: Duration = Duration::from_secs(600);

/// Delivery modes the tool accepts. `3` (direct to the end customer) needs a
/// whole block of end-user fields whose codes are not modelled here, so it stays
/// a `/post-order` feature.
const PICK_UP: u8 = 1;
const DELIVERY: u8 = 2;
const END_CUSTOMER: u8 = 3;


McpToolArgs! {
    pub struct SubmitOrderArgs {
        /// The order lines. Use the exact article number (`no`) from
        /// search_products or get_product.
        pub items: Vec<OrderLineArgs>,
        /// 2 (the default): Orink delivers to the partner — to `delivery_address`
        /// when given, otherwise to the partner's default address.
        /// 1: the partner picks the goods up at Orink.
        #[serde(default, deserialize_with = "lenient_count")]
        pub delivery_mode: Option<u32>,
        /// Only when it differs from the partner's default address, and only
        /// with delivery_mode 2.
        pub delivery_address: Option<DeliveryAddressArgs>,
        /// The partner the order is booked to. Omit it: by default the order goes
        /// to this connector's own partner. Set it only when the user explicitly
        /// names a different partner id to order for.
        #[serde(default, deserialize_with = "lenient_count")]
        pub partner_id: Option<u32>,
        /// The customer's own order reference, printed on Orink's documents.
        /// One is generated when omitted.
        pub reference: Option<String>,
        /// A note to Orink about the order.
        pub note: Option<String>,
        /// A note for the delivery (for the courier).
        pub delivery_note: Option<String>,
        /// Set to true ONLY when the user has explicitly confirmed that they want
        /// to place an order identical to one placed in the last 10 minutes.
        /// Never set it on a first attempt.
        #[serde(default, deserialize_with = "lenient_flag")]
        pub confirm_duplicate: Option<bool>
    }

    pub struct OrderLineArgs {
        /// Article number, exactly as the catalog lists it.
        pub no: String,
        /// How many to order, in the product's own unit.
        #[serde(deserialize_with = "lenient_quantity")]
        pub quantity: f64,
        /// A note for this line.
        pub note: Option<String>
    }

    pub struct DeliveryAddressArgs {
        /// Recipient / company name at the address.
        pub name: Option<String>,
        /// Two-letter ISO country code, e.g. "HU", "AT", "DE". Defaults to "HU".
        pub country: Option<String>,
        /// Postal code.
        pub zip: String,
        pub city: String,
        /// Street, house number and anything else that belongs on the line.
        pub street: String
    }
}


/// Accepts `true` and `"true"` — the same bridge problem `lenient_count` solves.
fn lenient_flag<'de, D>(deserializer: D) -> Result<Option<bool>, D::Error>
where
    D: serde::Deserializer<'de>
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Flag {
        Bool(bool),
        Text(String)
    }

    match Option::<Flag>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Flag::Bool(value)) => Ok(Some(value)),
        Some(Flag::Text(text)) => match text.trim().to_ascii_lowercase().as_str() {
            "" => Ok(None),
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            other => Err(serde::de::Error::custom(format!("'{}' is not true or false", other)))
        }
    }
}

/// Accepts `2`, `2.5`, `"2"` and `"2,5"` — a Hungarian decimal comma included,
/// since that is how a person in this catalog's market writes a quantity.
fn lenient_quantity<'de, D>(deserializer: D) -> Result<f64, D::Error>
where
    D: serde::Deserializer<'de>
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Quantity {
        Number(f64),
        Text(String)
    }

    match Quantity::deserialize(deserializer)? {
        Quantity::Number(value) => Ok(value),
        Quantity::Text(text) => text.trim().replace(',', ".").parse::<f64>()
            .map_err(|_| serde::de::Error::custom(format!("'{}' is not a quantity", text.trim())))
    }
}


/// A validated order, ready to send.
pub struct Prepared {
    pub order: Order,
    pub pid: u64,
    pub reference: String,
    pub lines: usize,
    /// Identity of this order for the duplicate guard: partner, mode and the
    /// sorted lines. Notes and references are left out on purpose — a retry
    /// with a fresh generated reference is still the same order.
    pub fingerprint: String
}


/// Trims an optional text argument; blank becomes absent.
fn text(value: Option<String>) -> Option<String> {
    value.map(|value| value.trim().to_string()).filter(|value| !value.is_empty())
}

/// A two-letter ISO code to the Hungarian country name Octopus understands.
///
/// Stricter than `country_to_hu`, which passes an unrecognised value through
/// untouched: that suits a REST caller who may already send Hungarian names, but
/// a model's guess at a country name would reach Octopus unchecked.
fn country_from_iso(code: &str) -> Option<String> {
    let code = code.trim();
    if code.len() != 2 {
        return None
    }
    COUNTRIES.iter()
        .find(|country| country.code.eq_ignore_ascii_case(code))
        .map(|country| country.hu_name.clone())
}

/// A reference for an order the caller did not name: dated, so it sorts and is
/// recognisable in Octopus, and random enough not to repeat.
fn generated_reference() -> String {
    let random = uuid::Uuid::new_v4().simple().to_string();
    format!("MCP-{}-{}", Utc::now().format("%Y%m%d"), random[..6].to_uppercase())
}


/// Turns tool arguments into an [`Order`], or says what to fix.
///
/// Everything refused here is refused before any call to Octopus. `own_pid` is
/// the caller's partner — the default when no `partner_id` is given.
pub fn prepare(args: SubmitOrderArgs, own_pid: i64) -> Result<Prepared, String> {
    if args.items.is_empty() {
        return Err("items must list at least one line.".into())
    }
    if args.items.len() > MAX_LINES {
        return Err(format!("An order can have at most {} lines; this one has {}.", MAX_LINES, args.items.len()))
    }

    let delivery_mode = match args.delivery_mode {
        None => DELIVERY,
        Some(mode) if mode == PICK_UP as u32 => PICK_UP,
        Some(mode) if mode == DELIVERY as u32 => DELIVERY,
        Some(mode) if mode == END_CUSTOMER as u32 => return Err(
            "delivery_mode 3 (direct to the end customer) is not available through this connector. \
             Use 2 for delivery to the partner or 1 for pick-up.".into()
        ),
        Some(other) => return Err(format!(
            "delivery_mode {} is not valid: use 2 (delivery to the partner) or 1 (pick-up at Orink).", other
        ))
    };

    let delivery_address = match args.delivery_address {
        None => None,
        Some(_) if delivery_mode == PICK_UP => return Err(
            "A delivery_address only applies to delivery_mode 2; a pick-up order (1) has none.".into()
        ),
        Some(address) => {
            let country_code = text(address.country).unwrap_or_else(|| "HU".into());
            let Some(country) = country_from_iso(&country_code) else {
                return Err(format!(
                    "'{}' is not a two-letter ISO country code. Use e.g. HU, AT, DE, SK, RO.", country_code
                ))
            };
            let zip = text(Some(address.zip));
            let city = text(Some(address.city));
            let street = text(Some(address.street));
            if zip.is_none() || city.is_none() || street.is_none() {
                return Err("delivery_address needs zip, city and street.".into())
            }
            Some(Address { name: text(address.name), country: Some(country), zip, city, street })
        }
    };

    let pid = match args.partner_id {
        None => match u64::try_from(own_pid) {
            Ok(pid) if pid > 0 => pid,
            _ => return Err("This connector has no valid partner id to order for.".into())
        },
        Some(0) => return Err("partner_id must be a positive number.".into()),
        Some(pid) => pid as u64
    };

    let mut items = Vec::with_capacity(args.items.len());
    let mut fingerprint_lines = Vec::with_capacity(args.items.len());
    for (index, line) in args.items.into_iter().enumerate() {
        let no = line.no.trim().to_string();
        if no.is_empty() {
            return Err(format!("Line {} has no article number.", index + 1))
        }
        if !line.quantity.is_finite() || line.quantity <= 0.0 {
            return Err(format!("Line {} ({}) needs a quantity above zero.", index + 1, no))
        }
        fingerprint_lines.push(format!("{}*{}", no.to_uppercase(), line.quantity));
        items.push(Item {
            lot_no: index as u64 + 1,
            no,
            qty: line.quantity,
            enduser_price: None,
            note: text(line.note)
        });
    }
    fingerprint_lines.sort();

    let reference = text(args.reference).unwrap_or_else(generated_reference);
    let lines = items.len();

    let order = Order {
        version: "1.0".into(),
        header: Header {
            pid,
            foreign_order_number: Some(reference.clone()),
            delivery_mode,
            delivery_note: text(args.delivery_note),
            enduser_id: None,
            enduser_name: None,
            enduser_contact_id: None,
            enduser_contact: None,
            contact_phone: None,
            contact_email: None,
            note: text(args.note),
            note_warehouse: None,
            note_hidden: None,
            invoice_address: None,
            delivery_address,
            enduser_delivery_mode: None,
            enduser_currency: None,
            enduser_payment_method: None,
            enduser_payment_deadline: None,
            enduser_vat_no: None,
            enduser_type: None,
            enduser_invoice: None
        },
        items: Items { items }
    };

    Ok(Prepared {
        order,
        pid,
        reference,
        lines,
        fingerprint: format!("{}|{}|{}", pid, delivery_mode, fingerprint_lines.join(","))
    })
}


/// Orders sent recently, by fingerprint: when, and under which reference.
static RECENT: Lazy<Mutex<HashMap<String, (Instant, String)>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// Claims `fingerprint` for this submission, or reports the earlier one.
///
/// Check and insert happen under one lock, so two identical calls arriving
/// together cannot both pass. `confirmed` skips the check (the user has said
/// they mean it) but still records the new submission.
fn reserve(fingerprint: &str, reference: &str, confirmed: bool) -> Result<(), (String, u64)> {
    let mut recent = RECENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    recent.retain(|_, (at, _)| at.elapsed() < DUPLICATE_WINDOW);
    if !confirmed && let Some((at, earlier)) = recent.get(fingerprint) {
        return Err((earlier.clone(), at.elapsed().as_secs()))
    }
    recent.insert(fingerprint.to_string(), (Instant::now(), reference.to_string()));
    Ok(())
}

/// Forgets a reservation whose order certainly was not recorded, so the same
/// order can be sent again at once.
fn release(fingerprint: &str) {
    let mut recent = RECENT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    recent.remove(fingerprint);
}


/// Places the order and describes the result. The whole tool, minus the
/// permission checks `tools.rs` makes first.
pub async fn run(args: SubmitOrderArgs, auth: &McpAuth, url: &str) -> CallToolResult {
    let confirmed = args.confirm_duplicate.unwrap_or(false);
    let prepared = match prepare(args, auth.pid) {
        Ok(prepared) => prepared,
        Err(message) => {
            logger(format!("MCP tool 'submit_order' by {}: refused before sending — {}", auth.masked(), message));
            return CallToolResult::error(vec![Content::text(message)])
        }
    };

    if let Err((earlier, secs_ago)) = reserve(&prepared.fingerprint, &prepared.reference, confirmed) {
        logger(format!(
            "MCP tool 'submit_order' by {}: refused as a duplicate of '{}' ({}s ago)",
            auth.masked(), earlier, secs_ago
        ));
        return CallToolResult::error(vec![Content::text(format!(
            "Not sent: an identical order (same partner, same lines) was submitted {} seconds ago \
             under reference '{}'. Do not resubmit on your own. Ask the user whether they really want \
             a second, identical order; only if they explicitly confirm, call again with \
             confirm_duplicate set to true.",
            secs_ago, earlier
        ))])
    }

    logger(format!(
        "MCP tool 'submit_order' by {}: sending '{}' to pid={} with {} line(s)",
        auth.masked(), prepared.reference, prepared.pid, prepared.lines
    ));

    let Prepared { order, pid, reference, lines, fingerprint } = prepared;
    let sent = Sent { pid, reference, lines };

    let timeout = Duration::from_secs(get_mcp_settings().order_timeout_secs());
    let label = format!("MCP ORDER '{}'", sent.reference);
    let log = |message: String| logger(format!("{}: {}", label, message));
    let outcome = order::submit(order, url, &derive_xmlns(url), &auth.authcode, Some(timeout), log).await;

    let (result, certainly_not_recorded) = describe(outcome, &sent);
    if certainly_not_recorded {
        release(&fingerprint);
    }
    result
}


/// What the answer is described against, once the order itself has been sent.
struct Sent {
    pid: u64,
    reference: String,
    lines: usize
}


/// Turns an outcome into the tool's answer, plus whether it is **certain** that
/// nothing was recorded (which frees the duplicate guard for an immediate retry).
fn describe(outcome: Outcome, sent: &Sent) -> (CallToolResult, bool) {
    match outcome {
        Outcome::Answered(envelope) => describe_answer(*envelope, sent),
        Outcome::NotSent(reason) => (
            CallToolResult::error(vec![Content::text(format!(
                "The order was NOT sent: Octopus could not be reached ({}). Nothing was recorded, \
                 so it is safe to submit it again once Octopus is reachable.",
                reason
            ))]),
            true
        ),
        Outcome::NoAnswer(reason) | Outcome::Unreadable(reason) => {
            elogger(format!("MCP ORDER '{}': outcome unknown — {}", sent.reference, reason));
            (
                CallToolResult::error(vec![Content::text(format!(
                    "OUTCOME UNKNOWN: the order was sent, but no usable answer came back ({}). It may \
                     or may not have been recorded. Do NOT submit it again on your own. Tell the user \
                     that the order with reference '{}' for partner {} needs checking in Orink's \
                     system before anything is resubmitted.",
                    reason, sent.reference, sent.pid
                ))]),
                false
            )
        }
    }
}

fn describe_answer(envelope: Envelope, sent: &Sent) -> (CallToolResult, bool) {
    let answer: Valasz = envelope.body.rendeles_feladas_auth_response.rendeles_feladas_auth_result.valasz;

    if let Some(hiba) = &answer.hiba {
        let error = EnglishError::from(hiba);
        logger(format!(
            "MCP ORDER '{}': refused by Octopus — {} ({})", sent.reference, error.description, error.code
        ));
        return (
            CallToolResult::error(vec![Content::text(format!(
                "Octopus refused the order: {} (error {}). Nothing was recorded.",
                error.description, error.code
            ))]),
            true
        )
    }

    let header = answer.fej.as_ref().filter(|fej| !fej.azonosito.trim().is_empty());

    let mut recorded_lines = 0usize;
    let mut total_net = 0.0;
    let mut total_gross = 0.0;
    let mut currency: Option<String> = None;
    let lines: Vec<Value> = answer.tetelek.tetel.iter().map(|line| {
        let recorded = line.rogzitett_tetelszam.trim() != "0"
            && line.mennyiseg.tipus.as_deref().map(str::trim) != Some("4");
        let mut row = json!({
            "line": line_number(&line.tetelszam),
            "no": line.cikkszam,
            "quantity": number(&line.mennyiseg.value, None),
            "recorded": recorded
        });
        if let Some(object) = row.as_object_mut() {
            if recorded {
                recorded_lines += 1;
                total_net += parse(line.ertek.as_deref()).unwrap_or(0.0);
                total_gross += parse(line.brertek.as_deref()).unwrap_or(0.0);
                if currency.is_none() {
                    currency = line.dnem.clone();
                }
                insert_money(object, "unit_price_net", line.egysegar.as_deref());
                insert_money(object, "unit_price_gross", line.bregysegar.as_deref());
                insert_money(object, "value_net", line.ertek.as_deref());
                insert_money(object, "value_gross", line.brertek.as_deref());
                if let Some(currency) = &line.dnem {
                    object.insert("currency".into(), json!(currency));
                }
                if let Some(date) = line.mennyiseg.datum.as_deref().filter(|date| !date.trim().is_empty()) {
                    object.insert("expected_date".into(), json!(date.get(..10).unwrap_or(date)));
                }
            } else {
                object.insert("reason".into(), json!(
                    line.mennyiseg.kenocs.as_deref().filter(|text| !text.trim().is_empty()).unwrap_or("not recorded")
                ));
            }
        }
        row
    }).collect();

    let status = match (header.is_some(), recorded_lines) {
        (false, _) | (true, 0) => "not_recorded",
        (true, n) if n == sent.lines && lines.len() == sent.lines => "recorded",
        _ => "partially_recorded"
    };

    logger(format!(
        "MCP ORDER '{}': {} — {}/{} line(s) recorded{}",
        sent.reference, status, recorded_lines, sent.lines,
        header.map(|fej| format!(", document {}", fej.bizonylatszam)).unwrap_or_default()
    ));

    let mut payload = json!({
        "status": status,
        "partner_id": sent.pid,
        "reference": sent.reference,
        "lines_sent": sent.lines,
        "lines_recorded": recorded_lines,
        "lines": lines
    });
    if let Some(object) = payload.as_object_mut() {
        if let Some(fej) = header {
            // Charges Octopus added on its own: delivery on a delivery order,
            // a deposit per DRS product, and so on. They are part of what the
            // partner pays, so they go into the totals.
            let mut charges_net = 0.0;
            let mut charges_gross = 0.0;
            let charges: Vec<Value> = [
                ("shipping", &answer.fuvarkoltseg),
                ("deposit", &answer.visszavaltasi_dij),
                ("cash_on_delivery", &answer.utanvet),
                ("extra_item", &answer.extratetelek),
                ("extra_service", &answer.extraszolg)
            ].into_iter().flat_map(|(kind, block)| block.tetel.iter().map(move |charge| (kind, charge))).map(|(kind, charge)| {
                charges_net += parse(charge.ertek.as_deref()).unwrap_or(0.0);
                charges_gross += parse(charge.brertek.as_deref()).unwrap_or(0.0);
                if currency.is_none() {
                    currency = charge.dnem.clone();
                }
                let mut row = json!({ "kind": kind });
                if let Some(row_object) = row.as_object_mut() {
                    if let Some(no) = &charge.cikkszam {
                        row_object.insert("no".into(), json!(no));
                    }
                    if let Some(name) = &charge.cikknev {
                        row_object.insert("name".into(), json!(name));
                    }
                    if let Some(quantity) = &charge.mennyiseg {
                        row_object.insert("quantity".into(), number(&quantity.value, None));
                    }
                    insert_money(row_object, "value_net", charge.ertek.as_deref());
                    insert_money(row_object, "value_gross", charge.brertek.as_deref());
                    // A deposit names the *recorded* line number of the
                    // product it is charged for.
                    if let Some(parent) = charge.vcapatszam.as_deref().filter(|parent| !parent.trim().is_empty()) {
                        row_object.insert("for_recorded_line".into(), line_number(parent));
                    }
                }
                row
            }).collect();

            object.insert("order_id".into(), json!(fej.azonosito));
            object.insert("document_number".into(), json!(fej.bizonylatszam));
            object.insert("web_id".into(), json!(fej.webazon));
            object.insert("delivery_date".into(), json!(fej.szalldatum));
            object.insert("items_net".into(), json!(round2(total_net)));
            object.insert("items_gross".into(), json!(round2(total_gross)));
            if !charges.is_empty() {
                object.insert("charges".into(), json!(charges));
            }
            object.insert("total_net".into(), json!(round2(total_net + charges_net)));
            object.insert("total_gross".into(), json!(round2(total_gross + charges_gross)));
            if let Some(currency) = currency {
                object.insert("currency".into(), json!(currency));
            }
        }
        object.insert("note".into(), json!(match status {
            "recorded" => "Every line was recorded. Give the user the document_number, and mention any \
                charges (delivery, deposit) — total_net and total_gross include them.",
            "partially_recorded" => "Only some lines were recorded — the order exists, but tell the user \
                exactly which lines were NOT recorded and why. Do not resubmit the recorded lines.",
            _ => "Nothing was recorded, usually because the article numbers are unknown. Check them with \
                search_products or get_product."
        }));
    }

    let text = serde_json::to_string(&payload).unwrap_or_else(|_| status.to_string());
    if status == "not_recorded" {
        (CallToolResult::error(vec![Content::text(text)]), true)
    } else {
        (CallToolResult::success(vec![Content::text(text)]), false)
    }
}


/// Octopus writes numbers with a decimal comma (`1150,6337850`).
fn parse(value: Option<&str>) -> Option<f64> {
    value?.trim().replace(',', ".").parse::<f64>().ok()
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

/// A number where Octopus sent one, the original text where it did not.
fn number(value: &str, decimals: Option<i32>) -> Value {
    match parse(Some(value)) {
        Some(parsed) => match decimals {
            Some(2) => json!(round2(parsed)),
            _ => json!(parsed)
        },
        None => json!(value.trim())
    }
}

/// A line number as an integer (`1`, not `1.0`), the original text where it is not one.
fn line_number(value: &str) -> Value {
    value.trim().parse::<u64>().map(|line| json!(line)).unwrap_or_else(|_| json!(value.trim()))
}

fn insert_money(object: &mut serde_json::Map<String, Value>, key: &str, value: Option<&str>) {
    if let Some(value) = value.filter(|value| !value.trim().is_empty()) {
        object.insert(key.into(), number(value, Some(2)));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn args(items: Vec<(&str, f64)>) -> SubmitOrderArgs {
        SubmitOrderArgs {
            items: items.into_iter().map(|(no, quantity)| OrderLineArgs { no: no.into(), quantity, note: None }).collect(),
            delivery_mode: None,
            delivery_address: None,
            partner_id: None,
            reference: None,
            note: None,
            delivery_note: None,
            confirm_duplicate: None
        }
    }

    fn address(country: Option<&str>) -> DeliveryAddressArgs {
        DeliveryAddressArgs {
            name: None,
            country: country.map(str::to_string),
            zip: "1111".into(),
            city: "Budapest".into(),
            street: "Fő utca 1.".into()
        }
    }

    /// What is refused before anything reaches Octopus, and what is filled in.
    #[test]
    fn prepare_validates_before_sending() {
        let refused: Vec<(&str, SubmitOrderArgs)> = vec![
            ("no lines", args(vec![])),
            ("blank article", args(vec![("  ", 1.0)])),
            ("zero quantity", args(vec![("A", 0.0)])),
            ("end-customer mode", SubmitOrderArgs { delivery_mode: Some(3), ..args(vec![("A", 1.0)]) }),
            ("unknown mode", SubmitOrderArgs { delivery_mode: Some(7), ..args(vec![("A", 1.0)]) }),
            ("address on pick-up", SubmitOrderArgs {
                delivery_mode: Some(1), delivery_address: Some(address(None)), ..args(vec![("A", 1.0)])
            }),
            ("country name, not ISO", SubmitOrderArgs { delivery_address: Some(address(Some("Germany"))), ..args(vec![("A", 1.0)]) }),
            ("zero partner", SubmitOrderArgs { partner_id: Some(0), ..args(vec![("A", 1.0)]) })
        ];
        for (case, refused) in refused {
            assert!(prepare(refused, 7824).is_err(), "{case} should be refused");
        }

        let prepared = prepare(
            SubmitOrderArgs { delivery_address: Some(address(Some("at"))), ..args(vec![("B", 2.0), ("A", 1.0)]) },
            7824
        ).expect("valid");
        assert_eq!(prepared.pid, 7824, "defaults to the caller's own partner");
        assert_eq!(prepared.order.header.delivery_mode, DELIVERY, "defaults to delivery");
        assert!(prepared.reference.starts_with("MCP-"), "a reference is generated");
        assert_eq!(prepared.order.items.items[1].lot_no, 2, "lines are numbered by the server");
        let country = prepared.order.header.delivery_address.and_then(|address| address.country);
        assert_eq!(country.as_deref(), Some("Ausztria"), "ISO code becomes the Hungarian name");

        let other = prepare(SubmitOrderArgs { partner_id: Some(9), ..args(vec![("A", 1.0), ("B", 2.0)]) }, 7824).expect("valid");
        assert_eq!(other.pid, 9, "an explicit partner wins");
        let same = prepare(args(vec![("b", 2.0), ("a", 1.0)]), 9).expect("valid");
        assert_eq!(same.fingerprint, other.fingerprint, "line order and letter case do not make a new order");
        assert_ne!(other.fingerprint, prepared.fingerprint, "a different partner is a different order");
    }

    /// Octopus's real answer to a delivery order of one DRS product: the order
    /// line plus a delivery charge and a deposit charge, both as `<tetel>`
    /// blocks. The first delivery order through the tool failed to parse on
    /// exactly this, and reported "outcome unknown" for an order that was fine.
    const DELIVERY_ORDER_ANSWER: &str = r#"<?xml version="1.0" encoding="utf-8"?><soap:Envelope xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema"><soap:Body><RendelesFeladasAuthResponse xmlns="https://orink.hu/services/"><RendelesFeladasAuthResult><valasz verzio="1.0" xmlns=""><fej><azonosito>2111994</azonosito><webazon>07f7e7c6-bdf5-4eb1-a2e1-63d857b4ae9d</webazon><bizonylatszam>XKBRE2630996</bizonylatszam><szalldatum>2026.11.12</szalldatum></fej><tetelek><tetel><tetelszam>1</tetelszam><rogzitett_tetelszam>1</rogzitett_tetelszam><cikkszam>62419</cikkszam><mennyiseg tipus="1" kenocs="{0:0.##} {1}" datum="2026-11-11T00:00:00">1,0000000</mennyiseg><egysegar>445,1154720</egysegar><bregysegar>565,29664944000</bregysegar><ertek>445,115472</ertek><brertek>565,296649</brertek><dnem>HUF</dnem></tetel></tetelek><extratetelek /><fuvarkoltseg><tetel><rogzitett_tetelszam>3</rogzitett_tetelszam><cikkid>67</cikkid><cikkszam>FUVARDIJKOZV</cikkszam><cikknev>Fuvar díj (közvetített szolgáltatás)</cikknev><menny>1,0000000</menny><mennyiseg tipus="1" kenocs="">1,0000000</mennyiseg><egysegar>2950,0000000</egysegar><bregysegar>3746,5000000</bregysegar><ertek>2950,0000000</ertek><brertek>3746,5000000</brertek><dnem>HUF</dnem></tetel></fuvarkoltseg><utanvet /><extraszolg /><visszavaltasi_dij><tetel><rogzitett_tetelszam>2</rogzitett_tetelszam><cikkid>57061</cikkid><cikkszam>EUTVISSZ</cikkszam><cikknev>Egyutas visszaváltási díj</cikknev><menny>1,0000000</menny><mennyiseg tipus="1" kenocs="">1,0000000</mennyiseg><egysegar>50,0000000</egysegar><bregysegar>50,0000000</bregysegar><ertek>50,0000000</ertek><brertek>50,0000000</brertek><dnem>HUF</dnem><vcapatszam>1</vcapatszam></tetel></visszavaltasi_dij></valasz></RendelesFeladasAuthResult></RendelesFeladasAuthResponse></soap:Body></soap:Envelope>"#;

    #[test]
    fn a_delivery_order_answer_with_charges_is_read_and_totalled() {
        let parse = || quick_xml::de::from_str::<Envelope>(DELIVERY_ORDER_ANSWER).expect("Octopus's answer parses");

        let sent = Sent { pid: 7824, reference: "ai_test_20261011_1".into(), lines: 1 };
        let (result, certainly_not_recorded) = describe_answer(parse(), &sent);
        assert!(!certainly_not_recorded);
        assert_ne!(result.is_error, Some(true));
        let text = result.content.first().and_then(|content| content.as_text()).map(|text| text.text.clone()).unwrap_or_default();
        let payload: Value = serde_json::from_str(&text).expect("the summary is JSON");
        assert_eq!(payload["status"], "recorded");
        assert_eq!(payload["document_number"], "XKBRE2630996");
        assert_eq!(payload["charges"].as_array().map(Vec::len), Some(2), "delivery and deposit");
        assert_eq!(payload["items_net"], 445.12);
        assert_eq!(payload["total_net"], 3445.12, "the charges are part of the total");
        assert_eq!(payload["charges"][1]["for_recorded_line"], json!(1), "a line number, not 1.0");

        // The REST answer is built from the same parse.
        let english = crate::service::get_data::to_xml_string(
            &crate::forms::out::xml::orders_response::Envelope::from(parse())
        );
        assert!(english.contains("<product_number>FUVARDIJKOZV</product_number>"), "{english}");
        assert!(english.contains("<parent_item_number>1</parent_item_number>"), "{english}");
    }

    #[test]
    fn duplicate_guard_refuses_a_repeat_until_released_or_confirmed() {
        let fingerprint = "test-guard|2|A*1";
        assert!(reserve(fingerprint, "first", false).is_ok());
        assert_eq!(reserve(fingerprint, "second", false).map_err(|(earlier, _)| earlier), Err("first".into()));
        assert!(reserve(fingerprint, "confirmed", true).is_ok(), "the user confirmed a second order");
        release(fingerprint);
        assert!(reserve(fingerprint, "after release", false).is_ok(), "nothing was recorded, so a retry is free");
        release(fingerprint);
    }
}
