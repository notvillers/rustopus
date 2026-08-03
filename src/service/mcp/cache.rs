//! Weight-based snapshot cache.
//!
//! This is the whole reason MCP lives inside Rustopus rather than beside it: a
//! full catalog pull is ~46 MB and ~28 seconds against production, which is
//! unusable in a chat. `service/soap.rs` already coalesces *concurrent*
//! identical calls, but two calls a minute apart both hit Octopus. Here a built
//! snapshot is held until it ages out.
//!
//! Deliberately scoped to MCP. Caching inside `soap.rs` would silently change
//! all eight existing REST endpoints, whose consumers expect a live read.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use moka::future::Cache;
use moka::policy::EvictionPolicy;
use once_cell::sync::Lazy;
use sha2::{Digest, Sha256};

use crate::service::{
    config::get_mcp_settings,
    log::{elogger, logger},
    mcp::{
        index::{CatalogSnapshot, SnapshotError, build_snapshot},
        store
    }
};

/// Identity of one cached catalog.
///
/// The authcode is present only as a hash — the code itself must never reach a
/// key, a log line, a dashboard response or an error message.
///
/// `pid` is part of the key because prices are partner-specific. Note the cost:
/// of the three operations a snapshot merges, only `GetArlistaAuth` (prices)
/// takes a pid — `GetCikkekAuth` and `GetCikkekKeszletValtozasAuth` take an
/// authcode alone — so two pids under one authcode hold two copies of identical
/// master data and differ only in the price columns. That duplication is an
/// accepted trade for a key that is trivially correct; if the per-snapshot
/// `bytes` logged at build time shows it dominating the budget, splitting master
/// data into its own `(auth, url)` tier is the fix.
#[derive(Hash, Eq, PartialEq, Clone, Debug)]
pub struct CacheKey {
    pub auth_hash: [u8; 32],
    pub pid: i64,
    /// Included so a second Octopus instance cannot collide with the first.
    pub url: String
}

impl CacheKey {
    pub fn new(authcode: &str, pid: i64, url: &str) -> Self {
        Self {
            auth_hash: hash_authcode(authcode),
            pid,
            url: url.to_string()
        }
    }

}


/// SHA-256 of an authcode. One-way by construction: nothing in this service ever
/// needs to recover the code from a key.
pub fn hash_authcode(authcode: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(authcode.as_bytes());
    hasher.finalize().into()
}


/// The first four bytes of an authcode hash, hex-encoded — a stable identifier
/// that can safely appear in a URL, a log line or a dashboard row.
pub fn fingerprint(auth_hash: &[u8; 32]) -> String {
    auth_hash.iter()
        .take(4)
        .map(|byte| format!("{:02x}", byte))
        .collect()
}


/// Per-entry counters backing the dashboard's hit-rate column. moka does not
/// expose per-key statistics, so they are tracked alongside it.
#[derive(Debug, Default, Clone)]
pub struct EntryStats {
    pub hits: u64,
    pub misses: u64,
    /// Size of the snapshot as last inserted, in bytes.
    pub bytes: u64,
    /// How long the last build took, in milliseconds.
    pub last_build_ms: u64,
    pub last_built_at: Option<chrono::DateTime<chrono::Utc>>,
    pub products: usize
}

impl EntryStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}


/// The snapshot cache plus its bookkeeping.
pub struct SnapshotCache {
    entries: Cache<CacheKey, Arc<CatalogSnapshot>>,
    stats: Mutex<HashMap<CacheKey, EntryStats>>,
    budget_bytes: u64,
    /// Whether inserts are mirrored to the disk tier. Off in unit tests, which
    /// otherwise would write snapshot files into the working directory.
    mirror_to_disk: bool
}

impl SnapshotCache {
    fn new(budget_bytes: u64, ttl_secs: u64) -> Self {
        Self::build(budget_bytes, ttl_secs, true)
    }

    fn build(budget_bytes: u64, ttl_secs: u64, mirror_to_disk: bool) -> Self {
        let entries = Cache::builder()
            .max_capacity(budget_bytes)
            // Weigh entries by their measured size so the budget is in bytes,
            // not entry count: one combo's catalog is tens of megabytes. `min`
            // caps at u32 because moka's weigher is u32-wide; a single snapshot
            // that large would already be a bug worth noticing.
            .weigher(|_key, value: &Arc<CatalogSnapshot>| value.bytes.min(u32::MAX as u64) as u32)
            .time_to_live(std::time::Duration::from_secs(ttl_secs))
            // Explicitly LRU, not moka's default TinyLFU, and not oldest-by-
            // insertion. Both alternatives lose to precaching: TinyLFU's
            // admission filter can *reject* a freshly built snapshot nobody has
            // queried yet, and age-ordering evicts the entry the precache job
            // just spent ~28 seconds warming. Least-recently-*used* is the only
            // one of the three that keeps what people actually ask for.
            .eviction_policy(EvictionPolicy::lru())
            .build();

        Self {
            entries,
            stats: Mutex::new(HashMap::new()),
            budget_bytes,
            mirror_to_disk
        }
    }

    /// Configured budget in bytes.
    pub fn budget_bytes(&self) -> u64 {
        self.budget_bytes
    }

    /// Bytes currently held. moka evicts asynchronously, so this can briefly
    /// exceed the budget after a burst of large inserts — which is why the
    /// configured budget should sit ~20% below the container memory limit.
    pub fn used_bytes(&self) -> u64 {
        self.entries.weighted_size()
    }

    pub fn entry_count(&self) -> u64 {
        self.entries.entry_count()
    }

    /// A cached snapshot, or `None` on a miss. Never fetches.
    pub async fn peek(&self, key: &CacheKey) -> Option<Arc<CatalogSnapshot>> {
        let found = self.entries.get(key).await;
        self.record_lookup(key, found.is_some());
        found
    }

    /// Whether snapshots are held in memory at all.
    ///
    /// `[mcp] max_bytes = 0` turns the memory tier off entirely, so every query
    /// is served from disk. That does **not** lower peak memory — a snapshot
    /// still has to be materialized in RAM to be searched — it only makes the
    /// allocation transient rather than steady, at the cost of re-reading and
    /// re-indexing on every call.
    pub fn memory_enabled(&self) -> bool {
        self.budget_bytes > 0
    }

    /// A cached snapshot, from memory, then disk, then Octopus.
    ///
    /// Three tiers, each an order of magnitude dearer than the last:
    ///
    /// 1. **RAM** — microseconds. Bounded well below the host's memory because
    ///    one snapshot is ~46 MB and the host has ~1–1.5 GB. Skipped entirely
    ///    when `max_bytes` is 0.
    /// 2. **Disk** — well under a second. Where snapshots evicted from RAM, and
    ///    everything held across a restart, actually live.
    /// 3. **Octopus** — minutes. What this whole module exists to avoid.
    ///
    /// Concurrent callers for the same key share one trip through 2 and 3: moka's
    /// `try_get_with` admits a single initializer per key and hands the result to
    /// everyone waiting, so a cold start under load costs one fetch, not one per
    /// caller.
    pub async fn get_or_build(
        &self,
        authcode: &str,
        pid: i64,
        url: &str
    ) -> Result<Arc<CatalogSnapshot>, Arc<SnapshotError>> {
        let key = CacheKey::new(authcode, pid, url);

        if self.memory_enabled()
            && let Some(snapshot) = self.entries.get(&key).await {
                self.record_lookup(&key, true);
                return Ok(snapshot)
        }
        self.record_lookup(&key, false);

        // Memory tier off: read straight through, holding nothing afterwards.
        // The singleflight below lives in moka, so this path deliberately gives
        // up request coalescing — two simultaneous callers each load their own
        // copy, which is part of what the setting costs.
        if !self.memory_enabled() {
            let for_disk = key.clone();
            match actix_web::web::block(move || store::read(&for_disk)).await {
                Ok(Some(snapshot)) => return Ok(Arc::new(snapshot)),
                Ok(None) => (),
                Err(error) => elogger(format!("MCP cache: disk read failed to run: {}", error))
            }
            let snapshot = Arc::new(build_snapshot(authcode, pid, url, None).await?);
            write_through(key, snapshot.clone()).await;
            return Ok(snapshot)
        }

        let started = std::time::Instant::now();
        let for_disk = key.clone();
        let snapshot = self.entries
            .try_get_with(key.clone(), async move {
                // Reading a ~46 MB snapshot back is blocking, CPU-bound work.
                // Run it off the async workers so a cold query cannot stall the
                // REST endpoints sharing this runtime.
                let from_disk = actix_web::web::block(move || store::read(&for_disk)).await;
                match from_disk {
                    Ok(Some(snapshot)) => return Ok(Arc::new(snapshot)),
                    Ok(None) => (),
                    Err(error) => elogger(format!("MCP cache: disk read failed to run: {}", error))
                }

                let snapshot = Arc::new(build_snapshot(authcode, pid, url, None).await?);
                write_through(CacheKey::new(authcode, pid, url), snapshot.clone()).await;
                Ok(snapshot)
            })
            .await?;

        self.record_build(&key, &snapshot, started.elapsed().as_millis() as u64);
        Ok(snapshot)
    }

    /// Publishes an already-built snapshot to both tiers, replacing any previous
    /// one.
    ///
    /// Used by the precache job, which builds into a temporary snapshot and swaps
    /// it in here: the old entry stays queryable for the whole build, so nobody
    /// asking a question mid-refresh falls through to a cold fetch.
    pub async fn insert(&self, key: CacheKey, snapshot: Arc<CatalogSnapshot>, build_ms: u64) {
        self.record_build(&key, &snapshot, build_ms);
        if self.mirror_to_disk {
            write_through(key.clone(), snapshot.clone()).await;
        }
        if self.memory_enabled() {
            self.entries.insert(key, snapshot).await;
        }
    }

    /// Puts a snapshot into memory **without** mirroring it to disk.
    ///
    /// For snapshots that came *from* the disk tier: writing them back would
    /// re-serialize and re-compress tens of megabytes into a file that already
    /// holds exactly that content.
    pub async fn promote(&self, key: CacheKey, snapshot: Arc<CatalogSnapshot>) {
        self.record_build(&key, &snapshot, 0);
        if self.memory_enabled() {
            self.entries.insert(key, snapshot).await;
        }
    }

    /// Drops one entry from **both** tiers. Its statistics go too, so the
    /// dashboard does not show a hit rate for something no longer held.
    ///
    /// Evicting from memory alone would be surprising: the next query would
    /// silently reload the same data from disk, and an administrator who pressed
    /// "evict" to force fresh data would not get it.
    pub async fn invalidate(&self, key: &CacheKey) {
        self.entries.invalidate(key).await;
        if self.mirror_to_disk {
            store::remove(key);
        }
        if let Ok(mut stats) = self.stats.lock() {
            stats.remove(key);
        }
    }

    /// Snapshot of the per-entry counters, for the dashboard.
    pub fn stats(&self) -> HashMap<CacheKey, EntryStats> {
        self.stats.lock()
            .map(|stats| stats.clone())
            .unwrap_or_default()
    }

    fn record_lookup(&self, key: &CacheKey, hit: bool) {
        if let Ok(mut stats) = self.stats.lock() {
            let entry = stats.entry(key.clone()).or_default();
            if hit { entry.hits += 1 } else { entry.misses += 1 }
        }
    }

    fn record_build(&self, key: &CacheKey, snapshot: &CatalogSnapshot, build_ms: u64) {
        if let Ok(mut stats) = self.stats.lock() {
            let entry = stats.entry(key.clone()).or_default();
            entry.bytes = snapshot.bytes;
            entry.last_build_ms = build_ms;
            entry.last_built_at = Some(snapshot.fetched_at);
            entry.products = snapshot.products.len();
        }
    }

    /// Forces moka's deferred maintenance to run. Eviction is asynchronous, so
    /// tests (and the dashboard's usage figure) need this to see a settled state.
    pub async fn settle(&self) {
        self.entries.run_pending_tasks().await;
    }
}


/// Mirrors a snapshot to the disk tier.
///
/// Serializing and gzipping ~46 MB is seconds of CPU-bound work, so it runs on a
/// blocking thread. Doing it inline would park an async worker for the whole
/// write — and this runtime also serves the REST endpoints.
///
/// A disk failure is logged and swallowed: the snapshot is already in memory and
/// the query it was built for can be answered. Losing the write only costs a
/// rebuild later, which is not worth failing a live request over.
async fn write_through(key: CacheKey, snapshot: Arc<CatalogSnapshot>) {
    let products = snapshot.products.len();
    let started = std::time::Instant::now();

    match actix_web::web::block(move || store::write(&key, &snapshot)).await {
        Ok(Ok(size)) => logger(format!(
            "MCP cache: mirrored {} products to disk ({:.1} MB compressed, {:.1}s)",
            products,
            size as f64 / 1_048_576.0,
            started.elapsed().as_secs_f64()
        )),
        Ok(Err(error)) => elogger(format!("MCP cache: could not write snapshot to disk: {}", error)),
        Err(error) => elogger(format!("MCP cache: disk write failed to run: {}", error))
    }
}


/// Process-wide cache, built on first use from `[mcp]`.
///
/// Because it is lazy, a server started with `[mcp] enabled = false` never
/// touches it and holds no cache memory at all.
static CACHE: Lazy<SnapshotCache> = Lazy::new(|| {
    let config = get_mcp_settings();
    let cache = SnapshotCache::new(config.max_bytes(), config.ttl_secs());
    logger(format!(
        "MCP cache: budget {:.1} GB, ttl {}s",
        cache.budget_bytes as f64 / 1_000_000_000.0,
        config.ttl_secs()
    ));
    cache
});


/// Accessor for the process-wide cache.
pub fn cache() -> &'static SnapshotCache {
    &CACHE
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mcp::index::test_snapshot;

    const MB: u64 = 1_048_576;

    fn key(authcode: &str, pid: i64) -> CacheKey {
        CacheKey::new(authcode, pid, "https://example.test/services/vision.asmx")
    }

    #[test]
    fn different_authcodes_produce_different_keys() {
        assert_ne!(key("AAAA1111BBBB2222", 1), key("CCCC3333DDDD4444", 1));
    }

    #[test]
    fn same_authcode_and_pid_produce_one_shared_key() {
        // Two colleagues on the same combo must land on the same entry rather
        // than each paying for their own copy of the catalog.
        assert_eq!(key("AAAA1111BBBB2222", 7), key("AAAA1111BBBB2222", 7));
    }

    #[test]
    fn pid_and_url_are_part_of_the_identity() {
        assert_ne!(key("AAAA1111BBBB2222", 1), key("AAAA1111BBBB2222", 2));

        let other_url = CacheKey::new("AAAA1111BBBB2222", 1, "https://other.test/services/vision.asmx");
        assert_ne!(key("AAAA1111BBBB2222", 1), other_url);
    }

    #[test]
    fn the_key_never_carries_the_authcode() {
        let key = key("SUPERSECRETAUTHCODE", 1);
        let rendered = format!("{:?}", key);
        assert!(!rendered.contains("SUPERSECRETAUTHCODE"));
        assert!(!fingerprint(&key.auth_hash).contains("SUPERSECRET"));
    }

    #[test]
    fn the_fingerprint_is_stable_and_distinguishing() {
        let one = fingerprint(&hash_authcode("AAAA1111BBBB2222"));
        assert_eq!(one, fingerprint(&hash_authcode("AAAA1111BBBB2222")));
        assert_ne!(one, fingerprint(&hash_authcode("CCCC3333DDDD4444")));
        assert_eq!(one.len(), 8);
    }

    #[actix_web::test]
    async fn budget_is_enforced() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        for pid in 0..5 {
            cache.insert(key("AAAA1111BBBB2222", pid), Arc::new(test_snapshot(40 * MB)), 0).await;
        }
        cache.settle().await;

        // Five 40 MB entries against a 100 MB budget: at most two survive.
        assert!(cache.used_bytes() <= 100 * MB, "used {} > budget", cache.used_bytes());
        assert!(cache.entry_count() <= 2, "kept {} entries", cache.entry_count());
    }

    #[actix_web::test]
    async fn an_oversized_entry_evicts_until_it_fits() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        cache.insert(key("AAAA1111BBBB2222", 1), Arc::new(test_snapshot(60 * MB)), 0).await;
        cache.insert(key("AAAA1111BBBB2222", 2), Arc::new(test_snapshot(60 * MB)), 0).await;
        cache.settle().await;

        assert!(cache.used_bytes() <= 100 * MB);
        assert_eq!(cache.entry_count(), 1);
    }

    #[actix_web::test]
    async fn the_least_recently_used_entry_goes_first() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        let kept = key("AAAA1111BBBB2222", 1);
        let stale = key("AAAA1111BBBB2222", 2);

        cache.insert(kept.clone(), Arc::new(test_snapshot(40 * MB)), 0).await;
        cache.insert(stale.clone(), Arc::new(test_snapshot(40 * MB)), 0).await;
        cache.settle().await;

        // Touch the first so the second becomes least recently used.
        assert!(cache.peek(&kept).await.is_some());
        cache.settle().await;

        cache.insert(key("AAAA1111BBBB2222", 3), Arc::new(test_snapshot(40 * MB)), 0).await;
        cache.settle().await;

        assert!(cache.peek(&kept).await.is_some(), "the recently used entry was evicted");
        assert!(cache.peek(&stale).await.is_none(), "the least recently used entry survived");
    }

    #[actix_web::test]
    async fn entries_expire_after_their_ttl() {
        let cache = SnapshotCache::build(100 * MB, 1, false);
        let key = key("AAAA1111BBBB2222", 1);
        cache.insert(key.clone(), Arc::new(test_snapshot(MB)), 0).await;
        assert!(cache.peek(&key).await.is_some());

        tokio::time::sleep(std::time::Duration::from_millis(1_100)).await;
        cache.settle().await;

        assert!(cache.peek(&key).await.is_none(), "entry outlived its ttl");
    }

    #[actix_web::test]
    async fn invalidate_drops_the_entry_and_its_statistics() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        let key = key("AAAA1111BBBB2222", 1);
        cache.insert(key.clone(), Arc::new(test_snapshot(MB)), 0).await;

        cache.invalidate(&key).await;
        cache.settle().await;

        assert!(cache.peek(&key).await.is_none());
        // The peek above recorded a miss, so the entry reappears with a clean slate.
        assert_eq!(cache.stats().get(&key).map(|stats| stats.hits), Some(0));
    }

    #[actix_web::test]
    async fn a_zero_budget_holds_nothing_in_memory() {
        // `max_bytes = 0` is the shipped configuration: every query loads from
        // disk, so nothing may stay resident between calls.
        let cache = SnapshotCache::build(0, 3600, false);
        assert!(!cache.memory_enabled());

        let key = key("AAAA1111BBBB2222", 1);
        cache.insert(key.clone(), Arc::new(test_snapshot(40 * MB)), 0).await;
        cache.promote(key.clone(), Arc::new(test_snapshot(40 * MB))).await;
        cache.settle().await;

        assert_eq!(cache.entry_count(), 0, "a snapshot stayed resident with the memory tier off");
        assert_eq!(cache.used_bytes(), 0);
        assert!(cache.peek(&key).await.is_none());
    }

    #[actix_web::test]
    async fn a_positive_budget_keeps_the_memory_tier_on() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        assert!(cache.memory_enabled());

        let key = key("AAAA1111BBBB2222", 1);
        cache.insert(key.clone(), Arc::new(test_snapshot(MB)), 0).await;
        assert!(cache.peek(&key).await.is_some());
    }

    #[actix_web::test]
    async fn statistics_track_hits_and_misses() {
        let cache = SnapshotCache::build(100 * MB, 3600, false);
        let key = key("AAAA1111BBBB2222", 1);

        assert!(cache.peek(&key).await.is_none());
        cache.insert(key.clone(), Arc::new(test_snapshot(2 * MB)), 1234).await;
        assert!(cache.peek(&key).await.is_some());
        assert!(cache.peek(&key).await.is_some());

        let stats = cache.stats();
        let entry = stats.get(&key).expect("statistics recorded");
        assert_eq!(entry.hits, 2);
        assert_eq!(entry.misses, 1);
        assert_eq!(entry.bytes, 2 * MB);
        assert_eq!(entry.last_build_ms, 1234);
        assert!((entry.hit_rate() - 2.0 / 3.0).abs() < f64::EPSILON);
    }
}
