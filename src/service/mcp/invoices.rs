//! Invoices over MCP: the data behind `list_invoices`, `get_invoice` and
//! `export_invoices`.
//!
//! ## Why this is not a plain call
//!
//! `GetSzamlakAuth` is slow in proportion to what it returns, and a busy
//! partner's year can take minutes to assemble. An MCP client gives up on a tool
//! call long before that (claude.ai at around a minute), and what the user then
//! sees is a bare failure — while Octopus keeps working on an answer nobody will
//! read. So the call is never made inside a tool call:
//!
//! 1. A tool call **starts a background job** (or joins the one already running
//!    for the same caller and question) and waits for it for at most
//!    `[mcp] invoice_wait_secs`.
//! 2. If the job finishes in that time, the tool answers normally. If not, it
//!    answers **"in progress"** with how far the job got, and the model calls the
//!    same tool again with the same arguments. The job is not restarted — the
//!    second call picks up where the first stopped waiting.
//! 3. A finished result is **kept for `[mcp] invoice_ttl_secs`**, so paging,
//!    `get_invoice` and an export of what was just listed cost no Octopus call.
//!    Not longer: an invoice's outstanding balance changes when it is paid.
//!
//! ## Why the period is fetched month by month
//!
//! A long period is split into calendar-month windows, fetched one after
//! another. That does not make the whole faster, and is not meant to. It keeps
//! **one response in memory at a time** — a busy partner's year as a single XML
//! string is tens of megabytes on a host with ~1 GB of RAM — and it makes
//! progress reportable and **failure partial**: if month 9 times out, months
//! 1–8 are kept and a retry fetches only what is missing. Windows are fetched
//! behind their own [`GATE`] of two, so however many partners ask at once,
//! invoice jobs never hold more than two of the outbound `SOAP_GATE` permits the
//! REST endpoints share.
//!
//! Adjacent windows **overlap at their boundary instant** and invoices are
//! de-duplicated by id. Whether Octopus compares `datumig` inclusively, or by
//! date or by time, is not documented; an overlap is harmless under every
//! reading, where a gap would silently drop the last day of each month under
//! one of them.
//!
//! ## Unpaid invoices
//!
//! "What do I owe" asks for every open invoice whatever its age, so it is a
//! different question, not a filter on a period: one call with
//! `osszes_fizetetlen = 1` over the whole history, filtered here to a non-zero
//! outstanding balance. A date range on that question narrows the result by
//! invoice date afterwards.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use chrono::{DateTime, Datelike, Months, NaiveDate, NaiveTime, TimeZone, Utc};
use once_cell::sync::Lazy;
use tokio::sync::{Semaphore, watch};

use crate::{
    forms::{r#in::xml::defaults::CallData, out::xml::invoices as p_invoices},
    macros::out::{OutModelDeriveOnly, OutModelDeriveSerializeOnly},
    routes::default::derive_xmlns,
    service::{
        config::get_mcp_settings,
        dates::get_first_date,
        get::invoices::{InvoicesData, InvoicesXML},
        get_data::{RequestGet, ResponseGet},
        log::{elogger, logger},
        mcp::{McpAuth, cache::hash_authcode, export::Cell, index::fold, mask_authcode}
    }
};

/// Longest period one request may cover. Two years is 25 monthly windows; past
/// that a single question is better asked as several.
pub const MAX_PERIOD_DAYS: i64 = 731;

/// Period used when the caller names no dates: the last 90 days.
const DEFAULT_PERIOD_DAYS: i64 = 90;

/// Windows fetched at once across **all** invoice jobs.
const CONCURRENT_WINDOWS: usize = 2;

/// Pause before the one automatic retry of a failed window. Long enough for a
/// momentary rate limit or a dropped connection to clear.
const WINDOW_RETRY_PAUSE: Duration = Duration::from_secs(10);

/// Finished results kept per caller, and in all. A result is a few megabytes at
/// most, but nothing else bounds how many a caller could accumulate.
const MAX_RESULTS_PER_CALLER: usize = 3;
const MAX_RESULTS: usize = 32;

/// Below this, an outstanding balance is rounding noise, not a debt.
const MONEY_EPSILON: f64 = 0.005;

/// Bounds outbound invoice calls across every job; see the module docs.
static GATE: Lazy<Semaphore> = Lazy::new(|| Semaphore::new(CONCURRENT_WINDOWS));


OutModelDeriveOnly! {
    /// One invoice, flattened from the English REST model. The internal line
    /// record id (`cikkid`) is left out: it means nothing to a reader, and its
    /// values collide with real article numbers.
    #[derive(Clone)]
    pub struct Invoice {
        pub id: i64,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub no: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub date: Option<NaiveDate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub completion_date: Option<NaiveDate>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub payment_deadline: Option<NaiveDate>,
        pub currency: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub net_total: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub gross_total: Option<f64>,
        /// What is still to be paid on it, in its own currency.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub outstanding: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cancellation_no: Option<String>,
        /// The partner's own order reference.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub customer_order_no: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub delivery_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub delivery_country: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub delivery_zip: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub delivery_city: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub delivery_street: Option<String>,
        pub lines: Vec<InvoiceLine>
    }

    #[derive(Clone)]
    pub struct InvoiceLine {
        pub line: u64,
        pub no: String,
        pub name: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub quantity: Option<f64>,
        pub unit: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub net_unit_price: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub gross_unit_price: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub net_value: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub gross_value: Option<f64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub order_no: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub customer_order_no: Option<String>
    }
}

OutModelDeriveSerializeOnly! {
    /// Sums for one currency over a selection of invoices.
    #[derive(Default)]
    pub struct Totals {
        pub invoices: usize,
        pub net_total: f64,
        pub gross_total: f64,
        pub outstanding: f64,
        pub overdue_invoices: usize,
        pub overdue_outstanding: f64
    }
}


impl From<p_invoices::Invoice> for Invoice {
    fn from(invoice: p_invoices::Invoice) -> Self {
        let head = invoice.head;
        Self {
            id: head.id,
            no: head.no,
            date: head.date,
            completion_date: head.completition_date,
            payment_deadline: head.payment_deadline,
            currency: head.currency,
            net_total: head.net_price,
            gross_total: head.price,
            outstanding: head.remaining,
            cancellation_no: head.cancellation_no,
            customer_order_no: head.foreign_order_no,
            delivery_name: head.delivery_name,
            delivery_country: head.delivery_country,
            delivery_zip: head.delivery_zip,
            delivery_city: head.delivery_city,
            delivery_street: head.delivery_street,
            lines: invoice.products.product.into_iter().map(InvoiceLine::from).collect()
        }
    }
}

impl From<p_invoices::Product> for InvoiceLine {
    fn from(product: p_invoices::Product) -> Self {
        Self {
            line: product.lot_no,
            no: product.no,
            name: product.name,
            quantity: product.qty,
            unit: product.unit,
            net_unit_price: product.net_unit_price,
            gross_unit_price: product.unit_price,
            net_value: product.net_price,
            gross_value: product.price,
            order_no: product.order_no,
            customer_order_no: product.order_foreign_no
        }
    }
}


impl Invoice {
    pub fn is_open(&self) -> bool {
        self.outstanding.is_some_and(|value| value.abs() > MONEY_EPSILON)
    }

    pub fn is_overdue(&self, today: NaiveDate) -> bool {
        self.outstanding.is_some_and(|value| value > MONEY_EPSILON)
            && self.payment_deadline.is_some_and(|deadline| deadline < today)
    }

    /// Whether `reference` names this invoice: its number, or its id.
    fn is_called(&self, reference: &str) -> bool {
        let reference = reference.trim();
        self.no.as_deref().is_some_and(|no| no.trim().eq_ignore_ascii_case(reference))
            || self.id.to_string() == reference
    }

    /// The invoice without its lines or delivery address — one row of a list.
    pub fn summary(&self, today: NaiveDate) -> serde_json::Value {
        let mut value = serde_json::to_value(self).unwrap_or_else(|_| serde_json::json!({}));
        if let Some(object) = value.as_object_mut() {
            object.retain(|key, _| key != "lines" && !key.starts_with("delivery_"));
            object.insert("line_count".into(), serde_json::json!(self.lines.len()));
            if self.is_overdue(today) {
                object.insert("overdue".into(), serde_json::json!(true));
            }
        }
        value
    }

    /// Every folded word of `words` must appear in the invoice: its numbers,
    /// its delivery name and city, or any line's article number, name or order
    /// reference — so "which invoice had the yellow highlighters" works.
    fn matches(&self, words: &[String]) -> bool {
        if words.is_empty() {
            return true
        }
        let mut haystack = String::new();
        for part in [&self.no, &self.customer_order_no, &self.cancellation_no, &self.delivery_name, &self.delivery_city].into_iter().flatten() {
            haystack.push_str(part);
            haystack.push(' ');
        }
        for line in &self.lines {
            haystack.push_str(&line.no);
            haystack.push(' ');
            haystack.push_str(&line.name);
            haystack.push(' ');
            for part in [&line.order_no, &line.customer_order_no].into_iter().flatten() {
                haystack.push_str(part);
                haystack.push(' ');
            }
        }
        let haystack = fold(&haystack);
        words.iter().all(|word| haystack.contains(word.as_str()))
    }
}


/// What a caller asked Octopus for. Together with the caller, the key a job and
/// its result are found under.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Scope {
    /// Every invoice dated within the period, fetched month by month.
    Period { from: NaiveDate, to: NaiveDate },
    /// Every invoice with an outstanding balance, whatever its date.
    Unpaid
}

impl Scope {
    pub fn describe(&self) -> String {
        match self {
            Scope::Period { from, to } => format!("invoices dated {} to {}", from, to),
            Scope::Unpaid => "all unpaid invoices".to_string()
        }
    }

    /// The calls this scope is fetched in.
    fn windows(&self, today: NaiveDate) -> Vec<Window> {
        match self {
            Scope::Period { from, to } => month_windows(*from, *to),
            Scope::Unpaid => vec![Window { from: get_first_date(), to: end_of_day(today) }]
        }
    }

    fn is_unpaid(&self) -> bool {
        matches!(self, Scope::Unpaid)
    }
}


/// One `GetSzamlakAuth` call's date range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Window {
    from: DateTime<Utc>,
    to: DateTime<Utc>
}

fn start_of_day(date: NaiveDate) -> DateTime<Utc> {
    Utc.from_utc_datetime(&date.and_time(NaiveTime::MIN))
}

fn end_of_day(date: NaiveDate) -> DateTime<Utc> {
    let last_second = NaiveTime::from_hms_opt(23, 59, 59).unwrap_or(NaiveTime::MIN);
    Utc.from_utc_datetime(&date.and_time(last_second))
}

/// Splits a period into calendar-month windows. Each inner window ends at the
/// instant the next one starts (see the module docs); the outer ends are
/// exactly the period the caller asked for, as `/get-invoice` would send them.
fn month_windows(from: NaiveDate, to: NaiveDate) -> Vec<Window> {
    let mut windows = Vec::new();
    let mut start = from;
    loop {
        let next = start.with_day(1).and_then(|first| first.checked_add_months(Months::new(1)));
        match next {
            Some(next) if next <= to => {
                windows.push(Window { from: start_of_day(start), to: start_of_day(next) });
                start = next;
            }
            _ => {
                windows.push(Window { from: start_of_day(start), to: end_of_day(to) });
                return windows
            }
        }
    }
}


/// The invoices one window returned. Kept on a failed job so a retry fetches
/// only the windows that are missing.
#[derive(Clone)]
struct Fetched {
    window: Window,
    invoices: Vec<Invoice>
}

/// A finished, de-duplicated result.
pub struct InvoiceSet {
    pub invoices: Vec<Invoice>,
    pub fetched_at: DateTime<Utc>
}

impl InvoiceSet {
    fn assemble(scope: Scope, fetched: Vec<Fetched>) -> Self {
        let mut seen = HashSet::new();
        let mut invoices: Vec<Invoice> = fetched.into_iter()
            .flat_map(|part| part.invoices)
            .filter(|invoice| seen.insert(invoice.id))
            .filter(|invoice| !scope.is_unpaid() || invoice.is_open())
            .collect();

        match scope {
            // Newest first: "my invoices last year" reads from the latest.
            Scope::Period { .. } => invoices.sort_by(|a, b| b.date.cmp(&a.date).then(b.id.cmp(&a.id))),
            // Earliest deadline first: what is most overdue comes first. An
            // invoice with no deadline cannot be overdue, so it goes last.
            Scope::Unpaid => invoices.sort_by(|a, b| {
                match (a.payment_deadline, b.payment_deadline) {
                    (Some(x), Some(y)) => x.cmp(&y),
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (None, None) => std::cmp::Ordering::Equal
                }.then(a.id.cmp(&b.id))
            })
        }

        Self { invoices, fetched_at: Utc::now() }
    }

    pub fn age_secs(&self) -> i64 {
        (Utc::now() - self.fetched_at).num_seconds().max(0)
    }

    /// The invoices that pass `filter`, in the set's own order.
    pub fn select(&self, filter: &Filter, today: NaiveDate) -> Vec<&Invoice> {
        let words: Vec<String> = filter.query.as_deref()
            .map(|query| fold(query).split_whitespace().map(str::to_string).collect())
            .unwrap_or_default();
        self.invoices.iter()
            .filter(|invoice| !filter.overdue_only || invoice.is_overdue(today))
            .filter(|invoice| filter.from.is_none_or(|from| invoice.date.is_some_and(|date| date >= from)))
            .filter(|invoice| filter.to.is_none_or(|to| invoice.date.is_some_and(|date| date <= to)))
            .filter(|invoice| invoice.matches(&words))
            .collect()
    }

    pub fn find(&self, reference: &str) -> Option<&Invoice> {
        self.invoices.iter().find(|invoice| invoice.is_called(reference))
    }
}


/// Sums per currency. Rounded to cents, so a total does not read 1234.5600000001.
pub fn totals(invoices: &[&Invoice], today: NaiveDate) -> BTreeMap<String, Totals> {
    let mut sums: BTreeMap<String, Totals> = BTreeMap::new();
    for invoice in invoices {
        let entry = sums.entry(invoice.currency.clone()).or_default();
        entry.invoices += 1;
        entry.net_total += invoice.net_total.unwrap_or(0.0);
        entry.gross_total += invoice.gross_total.unwrap_or(0.0);
        entry.outstanding += invoice.outstanding.unwrap_or(0.0);
        if invoice.is_overdue(today) {
            entry.overdue_invoices += 1;
            entry.overdue_outstanding += invoice.outstanding.unwrap_or(0.0);
        }
    }
    for entry in sums.values_mut() {
        entry.net_total = round_cents(entry.net_total);
        entry.gross_total = round_cents(entry.gross_total);
        entry.outstanding = round_cents(entry.outstanding);
        entry.overdue_outstanding = round_cents(entry.overdue_outstanding);
    }
    sums
}

fn round_cents(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}


/// Narrowing applied to a finished result. Never part of the job key: filtering
/// a result that is already here costs nothing, so two questions about the same
/// period share one fetch.
#[derive(Debug, Clone, Default)]
pub struct Filter {
    pub query: Option<String>,
    /// By invoice date. Only set for the unpaid scope — a period was already
    /// narrowed by Octopus.
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    pub overdue_only: bool
}

/// A tool's arguments, resolved into what to fetch and how to narrow it.
#[derive(Debug, Clone)]
pub struct Question {
    pub scope: Scope,
    pub filter: Filter
}

/// Resolves the shared invoice-tool arguments.
///
/// Without dates a period means the last 90 days; an end date in the future is
/// brought back to today, since there is nothing to fetch past it.
pub fn resolve(
    from: Option<&str>,
    to: Option<&str>,
    unpaid_only: bool,
    overdue_only: bool,
    query: Option<&str>,
    today: NaiveDate
) -> Result<Question, String> {
    let from = from.map(|text| parse_date(text, "from_date")).transpose()?;
    let to = to.map(|text| parse_date(text, "to_date")).transpose()?;
    if let (Some(from), Some(to)) = (from, to) && from > to {
        return Err(format!("from_date ({}) is after to_date ({}).", from, to))
    }

    let query = query.map(str::trim).filter(|query| !query.is_empty()).map(str::to_string);

    if unpaid_only || overdue_only {
        return Ok(Question {
            scope: Scope::Unpaid,
            filter: Filter { query, from, to, overdue_only }
        })
    }

    let to = to.unwrap_or(today).min(today);
    let from = from.unwrap_or(to - chrono::Duration::days(DEFAULT_PERIOD_DAYS - 1));
    if from > today {
        return Err(format!("from_date ({}) is in the future, so there are no invoices to read yet.", from))
    }
    if (to - from).num_days() >= MAX_PERIOD_DAYS {
        return Err(format!(
            "{} to {} is longer than two years. Ask for at most {} days at a time — for example one year per call.",
            from, to, MAX_PERIOD_DAYS
        ))
    }

    Ok(Question {
        scope: Scope::Period { from, to },
        filter: Filter { query, from: None, to: None, overdue_only: false }
    })
}

/// Accepts `2025-03-01`, `2025.03.01`, `2025/03/01` and a full timestamp, whose
/// date part is taken. Anything else is refused with the expected shape.
pub fn parse_date(text: &str, field: &str) -> Result<NaiveDate, String> {
    let trimmed = text.trim().trim_end_matches('.');
    let normalized: String = trimmed.chars()
        .take(10)
        .map(|c| if c == '.' || c == '/' { '-' } else { c })
        .collect();
    NaiveDate::parse_from_str(&normalized, "%Y-%m-%d")
        .map_err(|_| format!("{} '{}' is not a date; use YYYY-MM-DD, e.g. 2025-01-31.", field, text))
}

/// Today, in the server's local time — the calendar day Octopus dates its
/// invoices by, which is not UTC's day for the first hours after midnight.
pub fn today() -> NaiveDate {
    chrono::Local::now().date_naive()
}


// ---------------------------------------------------------------------------
// Jobs
// ---------------------------------------------------------------------------

/// Who is asking. The authcode is held hashed, as everywhere a caller is a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Caller {
    auth_hash: [u8; 32],
    pid: i64
}

#[derive(Clone)]
enum Progress {
    Running { done: usize, total: usize, invoices: usize },
    Done { set: Arc<InvoiceSet>, finished: Instant },
    Failed { message: String, kept: Arc<Vec<Fetched>>, finished: Instant }
}

impl Progress {
    fn is_running(&self) -> bool {
        matches!(self, Progress::Running { .. })
    }

    fn finished(&self) -> Option<Instant> {
        match self {
            Progress::Running { .. } => None,
            Progress::Done { finished, .. } | Progress::Failed { finished, .. } => Some(*finished)
        }
    }
}

struct Job {
    started: Instant,
    progress: watch::Receiver<Progress>,
    /// Whether a caller has been told this job failed. The first call after a
    /// failure reports it; the one after that retries.
    reported: bool
}

static JOBS: Lazy<Mutex<HashMap<(Caller, Scope), Job>>> = Lazy::new(|| Mutex::new(HashMap::new()));

fn jobs() -> MutexGuard<'static, HashMap<(Caller, Scope), Job>> {
    // The map holds plain values and channel handles, so a poisoned guard's
    // state is still consistent; recover it rather than panic.
    JOBS.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}


/// How a fetch ended, from the tool's point of view.
pub enum Fetch {
    Ready(Arc<InvoiceSet>),
    /// Still running; ask again with the same arguments.
    Pending(Pending),
    /// Octopus refused or failed. A further call retries what is missing.
    Failed(String),
    /// Another question is still being fetched for this caller.
    Busy(String)
}

pub struct Pending {
    pub scope: Scope,
    pub done: usize,
    pub total: usize,
    pub invoices_so_far: usize,
    pub elapsed_secs: u64
}

enum Claim {
    Ready(Arc<InvoiceSet>),
    Failed(String),
    Busy(String),
    Wait(watch::Receiver<Progress>, Instant)
}


/// Answers `scope` for this caller: from a kept result, from a running job, or
/// by starting one — and waits up to `[mcp] invoice_wait_secs` for it.
pub async fn fetch(auth: &McpAuth, url: &str, scope: Scope) -> Fetch {
    let key = (Caller { auth_hash: hash_authcode(&auth.authcode), pid: auth.pid }, scope);

    let (mut receiver, started) = match claim(auth, url, key) {
        Claim::Ready(set) => return Fetch::Ready(set),
        Claim::Failed(message) => return Fetch::Failed(message),
        Claim::Busy(message) => return Fetch::Busy(message),
        Claim::Wait(receiver, started) => (receiver, started)
    };

    let wait = Duration::from_secs(get_mcp_settings().invoice_wait_secs());
    // Cloned out inside the timeout so the channel's read guard never outlives
    // the wait. `None` inside means the sender is gone; `Err` means time ran out.
    let waited = tokio::time::timeout(wait, async {
        receiver.wait_for(|progress| !progress.is_running()).await.ok().map(|progress| progress.clone())
    }).await;
    let settled = match waited {
        Ok(settled) => settled,
        Err(_) => {
            let progress = receiver.borrow().clone();
            return pending(scope, &progress, started)
        }
    };

    match settled {
        Some(Progress::Done { set, .. }) => Fetch::Ready(set),
        Some(Progress::Failed { message, .. }) => {
            if let Some(job) = jobs().get_mut(&key) {
                job.reported = true;
            }
            Fetch::Failed(message)
        }
        // The sender is gone without a final state: the job's task ended
        // abnormally. Forget it, so the next call starts afresh.
        Some(Progress::Running { .. }) | None => {
            jobs().remove(&key);
            Fetch::Failed("the invoice fetch stopped unexpectedly".to_string())
        }
    }
}

fn pending(scope: Scope, progress: &Progress, started: Instant) -> Fetch {
    let (done, total, invoices_so_far) = match progress {
        Progress::Running { done, total, invoices } => (*done, *total, *invoices),
        _ => (0, 0, 0)
    };
    Fetch::Pending(Pending { scope, done, total, invoices_so_far, elapsed_secs: started.elapsed().as_secs() })
}


/// Decides, under the lock, whether this question is answered, running, failed
/// or must start — and starts it.
fn claim(auth: &McpAuth, url: &str, key: (Caller, Scope)) -> Claim {
    let (caller, scope) = key;
    let mut jobs = jobs();
    sweep(&mut jobs);

    let mut kept = Arc::new(Vec::new());
    if let Some(job) = jobs.get_mut(&key) {
        let progress = job.progress.borrow().clone();
        match progress {
            Progress::Running { .. } => return Claim::Wait(job.progress.clone(), job.started),
            Progress::Done { set, .. } => return Claim::Ready(set),
            Progress::Failed { message, kept: fetched, .. } => {
                if !job.reported {
                    job.reported = true;
                    return Claim::Failed(message)
                }
                // Told once already: this call is the retry. Start again from
                // what did arrive.
                kept = fetched;
                jobs.remove(&key);
            }
        }
    }

    // One running job per caller. Without this a model asking five periods in
    // a row would queue five jobs, each pinning a gate permit in turn.
    if let Some(((_, other), job)) = jobs.iter().find(|((c, s), job)| *c == caller && *s != scope && job.progress.borrow().is_running()) {
        let progress = match &*job.progress.borrow() {
            Progress::Running { done, total, .. } if *total > 1 => format!(", {} of {} months fetched", done, total),
            _ => String::new()
        };
        return Claim::Busy(format!(
            "Another invoice request for this partner is still running ({}{}, started {}s ago). \
             Only one runs at a time: ask for that one again to collect it, or retry this one when it is done.",
            other.describe(), progress, job.started.elapsed().as_secs()
        ))
    }

    let total = scope.windows(today()).len();
    let (sender, receiver) = watch::channel(Progress::Running { done: 0, total, invoices: 0 });
    let started = Instant::now();
    jobs.insert(key, Job { started, progress: receiver.clone(), reported: false });
    trim(&mut jobs, caller);
    drop(jobs);

    logger(format!(
        "MCP invoices: job started for {} pid={} — {} in {} call(s){}",
        mask_authcode(&auth.authcode), auth.pid, scope.describe(), total,
        if kept.is_empty() { String::new() } else { format!(", {} kept from the failed attempt", kept.len()) }
    ));
    tokio::spawn(run(auth.authcode.clone(), auth.pid, url.to_string(), scope, kept, sender));

    Claim::Wait(receiver, started)
}


/// Drops finished jobs past their time to live.
fn sweep(jobs: &mut HashMap<(Caller, Scope), Job>) {
    let ttl = Duration::from_secs(get_mcp_settings().invoice_ttl_secs());
    jobs.retain(|_, job| job.progress.borrow().finished().is_none_or(|finished| finished.elapsed() < ttl));
}

/// Keeps finished results within [`MAX_RESULTS_PER_CALLER`] and [`MAX_RESULTS`],
/// oldest first out. Running jobs are never dropped.
fn trim(jobs: &mut HashMap<(Caller, Scope), Job>, caller: Caller) {
    let finished = |jobs: &HashMap<(Caller, Scope), Job>, only: Option<Caller>| {
        let mut list: Vec<((Caller, Scope), Instant)> = jobs.iter()
            .filter(|((c, _), _)| only.is_none_or(|only| *c == only))
            .filter_map(|(key, job)| job.progress.borrow().finished().map(|at| (*key, at)))
            .collect();
        list.sort_by_key(|(_, at)| *at);
        list
    };

    let mine = finished(jobs, Some(caller));
    for (key, _) in mine.iter().take(mine.len().saturating_sub(MAX_RESULTS_PER_CALLER)) {
        jobs.remove(key);
    }
    let all = finished(jobs, None);
    for (key, _) in all.iter().take(all.len().saturating_sub(MAX_RESULTS)) {
        jobs.remove(key);
    }
}


/// Looks an invoice up in this caller's kept results, newest result first.
pub fn find_kept(auth: &McpAuth, reference: &str) -> Option<(Invoice, Arc<InvoiceSet>)> {
    let caller = Caller { auth_hash: hash_authcode(&auth.authcode), pid: auth.pid };
    let mut sets: Vec<Arc<InvoiceSet>> = jobs().iter()
        .filter(|((c, _), _)| *c == caller)
        .filter_map(|(_, job)| match &*job.progress.borrow() {
            Progress::Done { set, .. } => Some(Arc::clone(set)),
            _ => None
        })
        .collect();
    sets.sort_by_key(|set| std::cmp::Reverse(set.fetched_at));
    sets.into_iter().find_map(|set| set.find(reference).cloned().map(|invoice| (invoice, set)))
}


/// The job itself: every missing window in turn, then the assembled result.
async fn run(
    authcode: String,
    pid: i64,
    url: String,
    scope: Scope,
    kept: Arc<Vec<Fetched>>,
    sender: watch::Sender<Progress>
) {
    let started = Instant::now();
    let windows = scope.windows(today());
    let total = windows.len();
    let mut fetched: Vec<Fetched> = kept.iter().filter(|part| windows.contains(&part.window)).cloned().collect();
    let count = |fetched: &[Fetched]| fetched.iter().map(|part| part.invoices.len()).sum::<usize>();
    sender.send_replace(Progress::Running { done: fetched.len(), total, invoices: count(&fetched) });

    for window in windows {
        if fetched.iter().any(|part| part.window == window) {
            continue
        }
        match fetch_window_with_retry(&authcode, pid, &url, window, scope.is_unpaid()).await {
            Ok(invoices) => {
                fetched.push(Fetched { window, invoices });
                sender.send_replace(Progress::Running { done: fetched.len(), total, invoices: count(&fetched) });
            }
            Err(message) => {
                elogger(format!(
                    "MCP invoices: job failed for {} pid={} — {} at {}..{} after {:.1}s ({} of {} call(s) done): {}",
                    mask_authcode(&authcode), pid, scope.describe(), window.from.date_naive(), window.to.date_naive(),
                    started.elapsed().as_secs_f64(), fetched.len(), total, message
                ));
                sender.send_replace(Progress::Failed {
                    message,
                    kept: Arc::new(fetched),
                    finished: Instant::now()
                });
                return
            }
        }
    }

    let set = InvoiceSet::assemble(scope, fetched);
    logger(format!(
        "MCP invoices: job finished for {} pid={} — {}: {} invoice(s) in {:.1}s",
        mask_authcode(&authcode), pid, scope.describe(), set.invoices.len(), started.elapsed().as_secs_f64()
    ));
    sender.send_replace(Progress::Done { set: Arc::new(set), finished: Instant::now() });
}

/// One window, retried once after a pause: a single dropped connection or a
/// momentary rate limit should not cost the caller a round trip.
async fn fetch_window_with_retry(authcode: &str, pid: i64, url: &str, window: Window, unpaid: bool) -> Result<Vec<Invoice>, String> {
    match fetch_window(authcode, pid, url, window, unpaid).await {
        Ok(invoices) => Ok(invoices),
        Err(first) => {
            elogger(format!(
                "MCP invoices: {}..{} failed for {} pid={}, retrying once: {}",
                window.from.date_naive(), window.to.date_naive(), mask_authcode(authcode), pid, first
            ));
            tokio::time::sleep(WINDOW_RETRY_PAUSE).await;
            fetch_window(authcode, pid, url, window, unpaid).await
        }
    }
}

/// One `GetSzamlakAuth` call through the existing dispatch — the same parsing,
/// `<hiba>` handling and error translation `/get-invoice` uses.
async fn fetch_window(authcode: &str, pid: i64, url: &str, window: Window, unpaid: bool) -> Result<Vec<Invoice>, String> {
    let Ok(_permit) = GATE.acquire().await else {
        return Err("the invoice gate is closed".to_string())
    };

    let call_data = CallData {
        authcode: authcode.to_string(),
        url: url.to_string(),
        xmlns: derive_xmlns(url),
        pid: Some(pid),
        type_mod: Some(1),
        from_date: Some(window.from),
        to_date: Some(window.to),
        unpaid: Some(if unpaid { 1 } else { 0 }),
        ..Default::default()
    };

    let response = RequestGet::Invoices(call_data).into_data().await;
    let ResponseGet::Invoices(InvoicesData::Xml(InvoicesXML::En(envelope))) = response else {
        return Err("Octopus answered in an unexpected shape".to_string())
    };
    let answer = envelope.body.response.result.answer;
    if let Some(error) = answer.error {
        return Err(format!("{} ({})", error.description.trim(), error.code))
    }
    Ok(answer.invoices.invoice.into_iter().map(Invoice::from).collect())
}


// ---------------------------------------------------------------------------
// Export
// ---------------------------------------------------------------------------

/// What one export row stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detail {
    /// One row per invoice line, carrying its invoice's number and dates.
    Lines,
    /// One row per invoice, with its totals.
    Invoices
}

impl Detail {
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.map(str::trim).map(str::to_lowercase).as_deref() {
            None | Some("") | Some("lines") | Some("line") | Some("items") => Some(Detail::Lines),
            Some("invoices") | Some("invoice") | Some("headers") => Some(Detail::Invoices),
            _ => None
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Detail::Lines => "lines",
            Detail::Invoices => "invoices"
        }
    }
}

const INVOICE_HEADERS: [&str; 17] = [
    "Invoice number", "Invoice date", "Completion date", "Payment deadline", "Currency",
    "Net total", "Gross total", "Outstanding", "Overdue", "Customer order number",
    "Cancellation number", "Delivery name", "Delivery country", "Delivery zip",
    "Delivery city", "Delivery street", "Lines"
];

const LINE_HEADERS: [&str; 16] = [
    "Invoice number", "Invoice date", "Payment deadline", "Currency", "Customer order number",
    "Line", "Article number", "Name", "Quantity", "Unit", "Net unit price", "Gross unit price",
    "Net value", "Gross value", "Order number", "Line customer order number"
];

fn invoice_row(invoice: &Invoice, today: NaiveDate) -> Vec<Cell> {
    vec![
        Cell::text(invoice.no.as_ref()),
        Cell::text(invoice.date),
        Cell::text(invoice.completion_date),
        Cell::text(invoice.payment_deadline),
        Cell::Text(invoice.currency.clone()),
        Cell::number(invoice.net_total),
        Cell::number(invoice.gross_total),
        Cell::number(invoice.outstanding),
        Cell::Text(if invoice.is_overdue(today) { "yes".into() } else { String::new() }),
        Cell::text(invoice.customer_order_no.as_ref()),
        Cell::text(invoice.cancellation_no.as_ref()),
        Cell::text(invoice.delivery_name.as_ref()),
        Cell::text(invoice.delivery_country.as_ref()),
        Cell::text(invoice.delivery_zip.as_ref()),
        Cell::text(invoice.delivery_city.as_ref()),
        Cell::text(invoice.delivery_street.as_ref()),
        Cell::Number(invoice.lines.len() as f64)
    ]
}

fn line_row(invoice: &Invoice, line: &InvoiceLine) -> Vec<Cell> {
    vec![
        Cell::text(invoice.no.as_ref()),
        Cell::text(invoice.date),
        Cell::text(invoice.payment_deadline),
        Cell::Text(invoice.currency.clone()),
        Cell::text(invoice.customer_order_no.as_ref()),
        Cell::Number(line.line as f64),
        // Text, never a number: article numbers carry leading zeros.
        Cell::Text(line.no.clone()),
        Cell::Text(line.name.clone()),
        Cell::number(line.quantity),
        Cell::Text(line.unit.clone()),
        Cell::number(line.net_unit_price),
        Cell::number(line.gross_unit_price),
        Cell::number(line.net_value),
        Cell::number(line.gross_value),
        Cell::text(line.order_no.as_ref()),
        Cell::text(line.customer_order_no.as_ref())
    ]
}

/// Writes the selected invoices to a file. Blocking — run it through
/// `web::block`. Returns the file and the number of rows written.
pub fn write_export(
    set: &InvoiceSet,
    filter: &Filter,
    detail: Detail,
    format: super::export::Format
) -> Result<(super::export::Prepared, usize), String> {
    let today = today();
    let selected = set.select(filter, today);
    match detail {
        Detail::Invoices => {
            let rows = selected.len();
            let prepared = super::export::write_table(
                "orink-invoices", &INVOICE_HEADERS,
                selected.iter().map(|invoice| invoice_row(invoice, today)), format
            )?;
            Ok((prepared, rows))
        }
        Detail::Lines => {
            let rows = selected.iter().map(|invoice| invoice.lines.len()).sum();
            let prepared = super::export::write_table(
                "orink-invoice-lines", &LINE_HEADERS,
                selected.iter().flat_map(|invoice| invoice.lines.iter().map(move |line| line_row(invoice, line))), format
            )?;
            Ok((prepared, rows))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn date(text: &str) -> NaiveDate {
        NaiveDate::parse_from_str(text, "%Y-%m-%d").expect("test date")
    }

    fn invoice(id: i64, day: &str, outstanding: f64, deadline: &str) -> Invoice {
        Invoice {
            id,
            no: Some(format!("SZ-{}", id)),
            date: Some(date(day)),
            completion_date: None,
            payment_deadline: Some(date(deadline)),
            currency: "HUF".into(),
            net_total: Some(1000.0),
            gross_total: Some(1270.0),
            outstanding: Some(outstanding),
            cancellation_no: None,
            customer_order_no: None,
            delivery_name: None,
            delivery_country: None,
            delivery_zip: None,
            delivery_city: None,
            delivery_street: None,
            lines: vec![InvoiceLine {
                line: 1,
                no: "A-1".into(),
                name: "Szövegkiemelő sárga".into(),
                quantity: Some(2.0),
                unit: "db".into(),
                net_unit_price: Some(500.0),
                gross_unit_price: Some(635.0),
                net_value: Some(1000.0),
                gross_value: Some(1270.0),
                order_no: None,
                customer_order_no: None
            }]
        }
    }

    #[test]
    fn a_period_is_split_into_months_that_overlap_only_at_their_boundaries() {
        let cases = [
            // Mid-month to mid-month: partial first and last month.
            ("2025-01-15", "2025-03-10", vec![
                ("2025-01-15T00:00:00", "2025-02-01T00:00:00"),
                ("2025-02-01T00:00:00", "2025-03-01T00:00:00"),
                ("2025-03-01T00:00:00", "2025-03-10T23:59:59")
            ]),
            // A single day is one call.
            ("2025-06-30", "2025-06-30", vec![("2025-06-30T00:00:00", "2025-06-30T23:59:59")]),
            // Ending on the first of a month: that day is still covered.
            ("2025-01-01", "2025-02-01", vec![
                ("2025-01-01T00:00:00", "2025-02-01T00:00:00"),
                ("2025-02-01T00:00:00", "2025-02-01T23:59:59")
            ]),
            // Across a year end.
            ("2024-12-20", "2025-01-05", vec![
                ("2024-12-20T00:00:00", "2025-01-01T00:00:00"),
                ("2025-01-01T00:00:00", "2025-01-05T23:59:59")
            ])
        ];
        for (from, to, expected) in cases {
            let windows: Vec<(String, String)> = month_windows(date(from), date(to)).into_iter()
                .map(|w| (w.from.format("%Y-%m-%dT%H:%M:%S").to_string(), w.to.format("%Y-%m-%dT%H:%M:%S").to_string()))
                .collect();
            let expected: Vec<(String, String)> = expected.into_iter().map(|(a, b)| (a.to_string(), b.to_string())).collect();
            assert_eq!(windows, expected, "{} .. {}", from, to);
        }
        // A full year is twelve calls, not thirteen.
        assert_eq!(month_windows(date("2025-01-01"), date("2025-12-31")).len(), 12);
    }

    #[test]
    fn assembling_drops_boundary_duplicates_and_paid_invoices_when_asked_for_unpaid() {
        let window = Window { from: start_of_day(date("2025-01-01")), to: end_of_day(date("2025-01-31")) };
        let parts = vec![
            Fetched { window, invoices: vec![invoice(1, "2025-01-31", 0.0, "2025-02-10"), invoice(2, "2025-02-01", 500.0, "2025-02-11")] },
            // Invoice 2 again: the next window starts at the instant this one ended.
            Fetched { window, invoices: vec![invoice(2, "2025-02-01", 500.0, "2025-02-11"), invoice(3, "2025-02-03", 100.0, "2025-01-20")] }
        ];

        let period = InvoiceSet::assemble(Scope::Period { from: date("2025-01-01"), to: date("2025-02-28") }, parts.clone());
        let ids: Vec<i64> = period.invoices.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![3, 2, 1], "each invoice once, newest first");

        let unpaid = InvoiceSet::assemble(Scope::Unpaid, parts);
        let ids: Vec<i64> = unpaid.invoices.iter().map(|i| i.id).collect();
        assert_eq!(ids, vec![3, 2], "paid invoice dropped, earliest deadline first");
    }

    #[test]
    fn selection_filters_by_overdue_date_and_words_in_the_lines() {
        let today = date("2025-03-01");
        let set = InvoiceSet::assemble(Scope::Unpaid, vec![Fetched {
            window: Window { from: start_of_day(today), to: end_of_day(today) },
            invoices: vec![invoice(1, "2025-01-10", 100.0, "2025-02-01"), invoice(2, "2025-02-20", 100.0, "2025-03-20")]
        }]);

        let overdue = Filter { overdue_only: true, ..Default::default() };
        assert_eq!(set.select(&overdue, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![1]);

        let february = Filter { from: Some(date("2025-02-01")), ..Default::default() };
        assert_eq!(set.select(&february, today).iter().map(|i| i.id).collect::<Vec<_>>(), vec![2]);

        // Accent-insensitive, and matched against line names.
        let words = Filter { query: Some("szovegkiemelo SARGA".into()), ..Default::default() };
        assert_eq!(set.select(&words, today).len(), 2);
        let none = Filter { query: Some("radir".into()), ..Default::default() };
        assert!(set.select(&none, today).is_empty());

        let totals = totals(&set.select(&Filter::default(), today), today);
        let huf = totals.get("HUF").expect("HUF totals");
        assert_eq!((huf.invoices, huf.outstanding, huf.overdue_invoices, huf.overdue_outstanding), (2, 200.0, 1, 100.0));
    }

    #[test]
    fn arguments_resolve_to_a_bounded_period_or_the_unpaid_question() {
        let today = date("2025-11-16");

        let default = resolve(None, None, false, false, None, today).expect("default");
        assert_eq!(default.scope, Scope::Period { from: date("2025-08-19"), to: today });

        // A future end is brought back to today.
        let future = resolve(Some("2025-01-01"), Some("2026-06-30"), false, false, None, today).expect("future");
        assert_eq!(future.scope, Scope::Period { from: date("2025-01-01"), to: today });

        // Dates narrow an unpaid question instead of defining a period.
        let unpaid = resolve(Some("2025.01.01"), None, true, false, Some("  "), today).expect("unpaid");
        assert_eq!(unpaid.scope, Scope::Unpaid);
        assert_eq!(unpaid.filter.from, Some(date("2025-01-01")));
        assert_eq!(unpaid.filter.query, None);

        assert!(resolve(Some("2025-03-01"), Some("2025-02-01"), false, false, None, today).is_err(), "reversed");
        assert!(resolve(Some("2022-01-01"), None, false, false, None, today).is_err(), "over two years");
        assert!(resolve(Some("tegnap"), None, false, false, None, today).is_err(), "not a date");
        assert_eq!(parse_date("2025-01-31T00:00:00Z", "from_date"), Ok(date("2025-01-31")));
    }
}
