use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use once_cell::sync::Lazy;
use futures::FutureExt;
use futures::future::{BoxFuture, Shared};
use tokio::sync::Semaphore;
use reqwest::{
    Client,
    header::CONTENT_TYPE
};

use crate::service::{
    config,
    log::{logger, elogger}
};

/// Default cap on concurrent outbound SOAP calls when `Config.toml` doesn't
/// set `[server] soap_concurrency`.
const DEFAULT_SOAP_CONCURRENCY: usize = 4;

/// Process-wide reqwest client, built once so the connection pool (and TLS
/// sessions) are reused across every outbound Octopus call.
static CLIENT: Lazy<Client> = Lazy::new(|| {
    match Client::builder()
        .timeout(Duration::from_secs(config::get_settings().server.timeout))
        .build() {
            Ok(client) => client,
            Err(error) => {
                elogger(format!("Error creating reqwest client: {error}"));
                Client::new()
        }
    }
});

/// Caps how many outbound SOAP calls run at once; excess requests wait here
/// (an idle async task is nearly free) instead of stacking response buffers
/// in memory. Coalesced waiters (see `get_response_shared`) never consume a
/// permit — the gate sits inside the one real fetch.
static SOAP_GATE: Lazy<Semaphore> = Lazy::new(|| {
    Semaphore::new(config::get_settings().server.soap_concurrency.unwrap_or(DEFAULT_SOAP_CONCURRENCY))
});

/// One in-flight upstream fetch that identical concurrent requests attach to.
/// The `id` guards cleanup: an entry is only removed by the caller that
/// created it, so a newer future under the same key is never deleted early.
struct InFlight {
    id: u64,
    fut: Shared<BoxFuture<'static, Arc<String>>>
}

/// Identical concurrent GET fetches share one upstream call through this map,
/// keyed by `(url, soap_request)` — the SOAP body already encodes the full
/// request identity (endpoint, xmlns, authcode, dates).
static IN_FLIGHT: Lazy<Mutex<HashMap<(String, String), InFlight>>> = Lazy::new(|| Mutex::new(HashMap::new()));

static NEXT_IN_FLIGHT_ID: AtomicU64 = AtomicU64::new(0);

/// Why an outbound SOAP call produced no response body.
///
/// The split exists for mutating calls: a read can simply be retried, but an
/// order that "failed" may or may not have been recorded, and the caller has to
/// be told which.
#[derive(Debug)]
pub enum SendError {
    /// The connection was never established, so Octopus never saw the request.
    /// Safe to send again.
    NotSent(String),
    /// The request may have reached Octopus — it timed out, or the connection
    /// broke after sending. Whether it took effect is unknown.
    NoAnswer(String)
}

impl std::fmt::Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotSent(reason) => write!(f, "not sent: {reason}"),
            Self::NoAnswer(reason) => write!(f, "no answer: {reason}")
        }
    }
}

/// Sends one SOAP request and reports *how* it failed, not just that it did.
///
/// `timeout` overrides the client-wide `[server] timeout` for this one call;
/// `None` keeps it. Never coalesced, so this is the entry point for mutating
/// calls (`/post-order`); the read-only fetchers go through
/// [`get_response_shared`].
pub async fn send(url: &str, soap_request: String, timeout: Option<Duration>) -> Result<String, SendError> {
    // Wait for a free slot; the permit is held only for the duration of this
    // one HTTP round-trip. `acquire` can only fail if the semaphore is closed
    // (never done here) — on that impossible error, log and fetch ungated
    // rather than fail the request.
    let _permit = match SOAP_GATE.acquire().await {
        Ok(permit) => Some(permit),
        Err(error) => {
            elogger(format!("SOAP gate error (continuing without permit): {}", error));
            None
        }
    };

    let mut request = CLIENT
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request);
    if let Some(timeout) = timeout {
        request = request.timeout(timeout);
    }

    let response = match request.send().await {
        Ok(response) => response,
        // A connect error (refused, DNS, TLS handshake — including one that
        // timed out) happens before a byte of the body is written. Anything
        // else, a timeout above all, may have been sent already.
        Err(error) if error.is_connect() || error.is_builder() => return Err(SendError::NotSent(error.to_string())),
        Err(error) => return Err(SendError::NoAnswer(error.to_string()))
    };

    response.text().await.map_err(|error| SendError::NoAnswer(error.to_string()))
}

/// This function handles the request to the given url with the given soap string, theoretically it can handle other requests too
pub async fn get_response(url: &str, soap_request: String) -> String {
    match send(url, soap_request, None).await {
        Ok(text) => text,
        Err(error) => {
            elogger(format!("Response error: {}", error));
            "<Envelope></Envelope>".into()
        }
    }
}

/// Singleflight variant of [`get_response`] for the read-only GET fetchers:
/// identical concurrent requests (same url + SOAP body) share one upstream
/// call and one response buffer instead of each fetching separately.
///
/// Do NOT use this for mutating calls (`/post-order` uses [`send`]) —
/// coalescing would silently merge two intentional submissions into one.
pub async fn get_response_shared(url: &str, soap_request: String) -> Arc<String> {
    let key = (url.to_string(), soap_request.clone());

    let (fut, my_id) = {
        // On mutex poisoning, recover the guard instead of panicking — the
        // map only ever holds cloneable handles, so its state stays valid.
        let mut in_flight = IN_FLIGHT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        match in_flight.get(&key) {
            Some(entry) => {
                logger(format!("Joining in-flight identical SOAP request to '{}'", url));
                (entry.fut.clone(), None)
            }
            None => {
                let owned_url = url.to_string();
                let fut = async move { Arc::new(get_response(&owned_url, soap_request).await) }
                    .boxed()
                    .shared();
                let id = NEXT_IN_FLIGHT_ID.fetch_add(1, Ordering::Relaxed);
                in_flight.insert(key.clone(), InFlight { id, fut: fut.clone() });
                (fut, Some(id))
            }
        }
    };

    let response = fut.await;

    // Only the caller that inserted the entry removes it, and only while it
    // still holds the same id — a newer in-flight request that reused the key
    // in the meantime must not be evicted.
    if let Some(my_id) = my_id {
        let mut in_flight = IN_FLIGHT.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        if in_flight.get(&key).is_some_and(|entry| entry.id == my_id) {
            in_flight.remove(&key);
        }
    }

    response
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::{io::Read, net::TcpListener, thread};

    /// The line between "never sent" and "sent, no answer" is what tells an
    /// order caller whether resubmitting can duplicate an order.
    #[actix_web::test]
    async fn send_separates_never_sent_from_no_answer() {
        // Nothing listens on a port that was just released: refused at connect.
        let closed = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).expect("bind");
        let refused = send(&format!("http://{closed}/"), String::new(), Some(Duration::from_secs(2))).await;
        assert!(matches!(refused, Err(SendError::NotSent(_))), "{refused:?}");

        // A server that accepts and reads the request but never replies.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let silent = listener.local_addr().expect("addr");
        thread::spawn(move || {
            if let Ok((mut stream, _)) = listener.accept() {
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer);
                thread::sleep(Duration::from_secs(3));
            }
        });
        let unanswered = send(&format!("http://{silent}/"), String::new(), Some(Duration::from_millis(300))).await;
        assert!(matches!(unanswered, Err(SendError::NoAnswer(_))), "{unanswered:?}");
    }
}
