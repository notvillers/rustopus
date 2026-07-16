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

/// This function handles the request to the given url with the given soap string, theoretically it can handle other requests too
pub async fn get_response(url: &str, soap_request: String) -> String {
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

    match CLIENT
        .post(url)
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send()
        .await {
            Ok(resp) => match resp.text().await {
                Ok(text) => return text,
                Err(error) => elogger(format!("Response error: {}", error))
            },
            Err(error) => elogger(format!("Response error: {}", error))
    }
    "<Envelope></Envelope>".into()
}

/// Singleflight variant of [`get_response`] for the read-only GET fetchers:
/// identical concurrent requests (same url + SOAP body) share one upstream
/// call and one response buffer instead of each fetching separately.
///
/// Do NOT use this for mutating calls (`/post-order` keeps the raw
/// `get_response`) — coalescing would silently merge two intentional
/// submissions into one.
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
