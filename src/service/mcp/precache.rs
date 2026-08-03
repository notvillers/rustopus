//! Background precache: keeps configured `(authcode, pid)` combinations warm so
//! nobody's question is the one that waits ~28 seconds for a cold catalog.
//!
//! Started from `main.rs` **only** when `[mcp] enabled = true`; with the flag off
//! no task is spawned and this module is never touched.
//!
//! ## Why this file holds credentials
//!
//! The job runs with no user present, so it needs real authcodes at rest —
//! hashing is not an option. `mcp_precache.toml` is therefore a secret-grade
//! file: gitignored, written `0600`, and mounted like `soap.json` rather than
//! baked into the image. Nothing in this module ever logs, returns or renders a
//! full authcode; [`mask_authcode`] guards every path out.
//!
//! ## Why runtime state is not persisted here
//!
//! The brief put last-run bookkeeping in this file too. It lives in memory
//! instead, so a file containing credentials is rewritten only when an
//! administrator actually edits an entry, not once an hour. The cost is small
//! and lands the safe way: after a restart the first sweep has no `last_run` to
//! work from and does a **full** pull, which is the conservative choice anyway.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::service::{
    config::get_mcp_settings,
    log::{elogger, logger},
    mcp::{
        cache::{CacheKey, cache, fingerprint, hash_authcode},
        index::{build_snapshot, refresh_snapshot},
        mask_authcode, store
    },
    path::get_current_or_root_dir,
    soap_config::get_default_url
};

/// Gap between entries in one sweep, so a refresh cycle does not fire every
/// configured combination at the ERP on the hour.
const STAGGER_SECS: u64 = 30;

/// How often a full pull replaces the incremental ones. Incremental responses do
/// not report products **deleted** in the ERP, so without this a removed product
/// would linger in the snapshot indefinitely.
const FULL_PULL_INTERVAL_SECS: i64 = 7 * 24 * 3600;


/// One configured combination to keep warm.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrecacheEntry {
    /// Human label for the dashboard, e.g. a team or person's name.
    pub label: String,
    /// The Octopus authentication code. **Secret** — never leaves this process
    /// unmasked.
    pub authcode: String,
    pub pid: i64,
    /// Octopus endpoint. Falls back to `soap.json` when unset.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Set `false` to keep an entry configured but stop refreshing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>
}

impl PrecacheEntry {
    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    /// The Octopus endpoint this entry targets.
    pub fn url(&self) -> Option<String> {
        self.url.as_ref()
            .filter(|url| !url.trim().is_empty())
            .cloned()
            .or_else(get_default_url)
    }

    /// Stable, non-reversible identifier: the first bytes of the authcode hash
    /// plus the pid. Safe to put in a URL, a log line or a dashboard row.
    pub fn id(&self) -> String {
        format!("{}-{}", fingerprint(&hash_authcode(&self.authcode)), self.pid)
    }

    /// The cache key this entry warms, or `None` when no endpoint is configured.
    pub fn cache_key(&self) -> Option<CacheKey> {
        self.url().map(|url| CacheKey::new(&self.authcode, self.pid, &url))
    }

    /// How this entry may be shown outside the process.
    pub fn masked(&self) -> String {
        format!("{} pid={}", mask_authcode(&self.authcode), self.pid)
    }
}


/// On-disk shape of `mcp_precache.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PrecacheConfig {
    #[serde(default, rename = "entry")]
    pub entries: Vec<PrecacheEntry>
}


/// What happened to one entry on its last run. Held in memory only.
#[derive(Debug, Clone, Default)]
pub struct EntryRun {
    pub last_run: Option<DateTime<Utc>>,
    pub last_full_pull: Option<DateTime<Utc>>,
    pub last_duration_ms: u64,
    pub last_outcome: Option<String>,
    pub running: bool
}

/// Runtime state per entry id.
static RUNS: Lazy<Mutex<HashMap<String, EntryRun>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// The configured entries, cached in memory and rewritten on edit.
static ENTRIES: Lazy<Mutex<Vec<PrecacheEntry>>> = Lazy::new(|| Mutex::new(load().entries));


/// Path to `mcp_precache.toml`, resolved against the working directory like
/// `soap.json` and `Config.toml`.
pub fn get_precache_path() -> PathBuf {
    let mut path = get_current_or_root_dir();
    path.push("mcp_precache.toml");
    path
}


/// Reads `mcp_precache.toml`, or an empty configuration when it is absent or
/// unreadable. A missing file is normal — it means nothing is precached yet.
pub fn load() -> PrecacheConfig {
    let path = get_precache_path();
    if !path.is_file() {
        return PrecacheConfig::default()
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<PrecacheConfig>(&content) {
            Ok(config) => config,
            Err(error) => {
                elogger(format!("MCP precache: cannot parse '{:?}': {}", path, error));
                PrecacheConfig::default()
            }
        },
        Err(error) => {
            elogger(format!("MCP precache: cannot read '{:?}': {}", path, error));
            PrecacheConfig::default()
        }
    }
}


/// Writes `mcp_precache.toml` with owner-only permissions.
///
/// Written to a temporary file and renamed, so a crash mid-write cannot leave a
/// half-written credential file behind.
pub fn save(config: &PrecacheConfig) -> Result<(), String> {
    let path = get_precache_path();
    let body = toml::to_string_pretty(config).map_err(|error| error.to_string())?;
    let content = format!(
        "# Rustopus MCP precache entries.\n\
         #\n\
         # SECRET FILE: every entry holds a live Octopus authcode in plain text,\n\
         # because the precache job runs with no user present to supply one.\n\
         # Keep it gitignored, keep it 0600, and mount it like soap.json rather\n\
         # than baking it into an image.\n\
         #\n\
         # Managed by the /admin dashboard; hand edits are picked up on restart.\n\n{}",
        body
    );

    let temporary = path.with_extension("toml.tmp");
    std::fs::write(&temporary, content).map_err(|error| error.to_string())?;
    restrict_permissions(&temporary);
    std::fs::rename(&temporary, &path).map_err(|error| error.to_string())?;
    restrict_permissions(&path);
    Ok(())
}


/// Narrows a file to owner read/write. A failure is logged rather than fatal —
/// the alternative is refusing to save an administrator's change at all.
fn restrict_permissions(path: &PathBuf) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            elogger(format!("MCP precache: cannot restrict permissions on '{:?}': {}", path, error));
        }
    }
    #[cfg(not(unix))]
    {
        // Windows has no direct mode equivalent; inherited ACLs apply. Flagged in
        // DOCKER_PLAN.md so the mount is provisioned as a secret either way.
        let _ = path;
    }
}


/// The configured entries.
pub fn entries() -> Vec<PrecacheEntry> {
    ENTRIES.lock()
        .map(|entries| entries.clone())
        .unwrap_or_default()
}


/// Adds an entry, or replaces the one with the same id, and persists the result.
pub fn upsert(entry: PrecacheEntry) -> Result<(), String> {
    let mut entries = ENTRIES.lock().map_err(|_| "precache entries lock poisoned".to_string())?;
    let id = entry.id();
    match entries.iter().position(|existing| existing.id() == id) {
        Some(position) => entries[position] = entry,
        None => entries.push(entry)
    }
    save(&PrecacheConfig { entries: entries.clone() })
}


/// Removes an entry by id and persists the result. Returns whether it existed.
pub fn remove(id: &str) -> Result<bool, String> {
    let mut entries = ENTRIES.lock().map_err(|_| "precache entries lock poisoned".to_string())?;
    let before = entries.len();
    entries.retain(|entry| entry.id() != id);
    let removed = entries.len() != before;
    save(&PrecacheConfig { entries: entries.clone() })?;
    Ok(removed)
}


/// One entry by id.
pub fn find(id: &str) -> Option<PrecacheEntry> {
    entries().into_iter().find(|entry| entry.id() == id)
}


/// Runtime state per entry id, for the dashboard.
pub fn runs() -> HashMap<String, EntryRun> {
    RUNS.lock()
        .map(|runs| runs.clone())
        .unwrap_or_default()
}


/// Refreshes one entry now.
///
/// The replacement snapshot is built **before** anything is replaced, so the
/// previous one stays queryable for the whole build: a colleague asking a
/// question mid-refresh gets the slightly stale answer instantly rather than
/// waiting out a cold fetch.
pub async fn refresh(entry: &PrecacheEntry, force_full: bool) -> Result<(), String> {
    let Some(url) = entry.url() else {
        return Err("no Octopus url configured for this entry or in soap.json".into())
    };
    let key = CacheKey::new(&entry.authcode, entry.pid, &url);
    let id = entry.id();

    let previous_run = runs().get(&id).cloned().unwrap_or_default();
    mark_running(&id, true);
    let started = std::time::Instant::now();

    let interval = get_mcp_settings().precache_interval_secs() as i64;

    // Nothing to do when the stored snapshot is still young. Decided from the
    // file's timestamp rather than by reading it: this runs every sweep, and
    // decompressing tens of megabytes to learn a date would be absurd.
    //
    // With the memory tier off there is nowhere to promote it to either, so the
    // whole entry is a no-op until the snapshot ages.
    if !force_full
        && !cache().memory_enabled()
        && store::stored_age_secs(&key).is_some_and(|age| age < interval) {
            record(&id, 0, false, Ok(()));
            logger(format!(
                "MCP precache '{}' [{}]: stored snapshot still fresh, nothing to do",
                entry.label, entry.masked()
            ));
            return Ok(())
    }

    // Find a snapshot to work from: memory first, then disk. After a restart
    // memory is empty but the disk tier usually still holds a recent snapshot,
    // and merging a delta into that beats rebuilding the whole catalog by
    // minutes.
    let existing = if force_full {
        None
    } else {
        match cache().peek(&key).await {
            Some(snapshot) => Some(snapshot),
            None => {
                let for_disk = key.clone();
                match actix_web::web::block(move || store::read(&for_disk)).await {
                    Ok(snapshot) => snapshot.map(Arc::new),
                    Err(error) => {
                        elogger(format!("MCP precache: disk read failed to run: {}", error));
                        None
                    }
                }
            }
        }
    };

    // Whether to pull the whole catalog rather than a delta.
    //
    // Decided from the snapshot's own `fetched_at`, which survives a restart,
    // rather than from the in-memory run log, which does not. Keying this off
    // `last_full_pull` alone would force a full rebuild after every restart and
    // make the disk tier useless for the case it exists to serve.
    let needs_full = force_full
        || match &existing {
            // Incremental responses never report products deleted in the ERP,
            // so a snapshot has to be rebuilt from scratch periodically however
            // often it has been topped up.
            Some(previous) => previous.age_secs() >= FULL_PULL_INTERVAL_SECS,
            None => true
        };

    // A snapshot younger than one sweep interval needs no ERP call at all —
    // just put it back in memory. This is the ordinary path after a restart.
    if let Some(previous) = &existing
        && !needs_full
        && previous.age_secs() < interval {
            let age = previous.age_secs();
            let products = previous.products.len();
            // `promote`, not `insert`: this snapshot came off disk, so mirroring
            // it back would rewrite an identical file at real CPU cost.
            cache().promote(key, previous.clone()).await;
            record(&id, started.elapsed().as_millis() as u64, false, Ok(()));
            logger(format!(
                "MCP precache '{}' [{}]: reused a {}s-old snapshot from disk — {} products, no ERP call",
                entry.label, entry.masked(), age, products
            ));
            return Ok(())
    }

    let result = match (needs_full, existing) {
        (false, Some(previous)) => {
            let since = previous_run.last_run.unwrap_or(previous.fetched_at);
            refresh_snapshot(&previous, &entry.authcode, entry.pid, &url, since).await
        }
        _ => build_snapshot(&entry.authcode, entry.pid, &url, None).await
    };

    let elapsed_ms = started.elapsed().as_millis() as u64;

    match result {
        Ok(snapshot) => {
            let bytes = snapshot.bytes;
            let products = snapshot.products.len();
            cache().insert(key, Arc::new(snapshot), elapsed_ms).await;
            record(&id, elapsed_ms, needs_full, Ok(()));
            logger(format!(
                "MCP precache '{}' [{}]: {} refresh ok — {} products, {:.1} MB, {:.1}s",
                entry.label,
                entry.masked(),
                if needs_full { "full" } else { "incremental" },
                products,
                bytes as f64 / 1_048_576.0,
                elapsed_ms as f64 / 1000.0
            ));
            Ok(())
        }
        Err(error) => {
            let message = error.to_string();
            record(&id, elapsed_ms, needs_full, Err(message.clone()));
            elogger(format!(
                "MCP precache '{}' [{}]: refresh failed after {:.1}s — {}",
                entry.label,
                entry.masked(),
                elapsed_ms as f64 / 1000.0,
                message
            ));
            Err(message)
        }
    }
}


fn mark_running(id: &str, running: bool) {
    if let Ok(mut runs) = RUNS.lock() {
        runs.entry(id.to_string()).or_default().running = running;
    }
}


fn record(id: &str, elapsed_ms: u64, was_full: bool, outcome: Result<(), String>) {
    if let Ok(mut runs) = RUNS.lock() {
        let run = runs.entry(id.to_string()).or_default();
        run.running = false;
        run.last_duration_ms = elapsed_ms;
        match outcome {
            Ok(()) => {
                let now = Utc::now();
                run.last_run = Some(now);
                if was_full {
                    run.last_full_pull = Some(now);
                }
                run.last_outcome = Some("ok".into());
            }
            Err(message) => run.last_outcome = Some(message)
        }
    }
}


/// Refreshes every enabled entry, spaced out so one sweep cannot monopolize the
/// outbound SOAP budget.
///
/// Entries run one at a time. Combined with the [`STAGGER_SECS`] gap, a sweep
/// uses at most the two concurrent calls one snapshot build issues, well inside
/// the `soap_concurrency` gate that live API traffic shares — a sweep slows
/// itself down rather than starving `/get-product`.
async fn sweep() {
    let entries: Vec<PrecacheEntry> = entries().into_iter().filter(|entry| entry.is_enabled()).collect();
    if entries.is_empty() {
        return
    }

    logger(format!("MCP precache: sweep starting over {} entries", entries.len()));
    let started = std::time::Instant::now();

    for (position, entry) in entries.iter().enumerate() {
        if position > 0 {
            tokio::time::sleep(Duration::from_secs(STAGGER_SECS)).await;
        }
        // A failed entry is logged inside `refresh`; the sweep carries on so one
        // bad authcode cannot stop every other combination from refreshing.
        let _ = refresh(entry, false).await;
    }

    // moka's accounting is deferred, so `used_bytes` reads 0 straight after an
    // insert. Settle first or the sweep reports an empty cache it just filled.
    cache().settle().await;

    logger(format!(
        "MCP precache: sweep finished in {:.1}s, cache holding {:.1} MB of {:.1} MB",
        started.elapsed().as_secs_f64(),
        cache().used_bytes() as f64 / 1_048_576.0,
        cache().budget_bytes() as f64 / 1_048_576.0
    ));
}


/// Starts the refresh loop. Called from `main.rs` only when MCP is enabled.
pub fn spawn(interval_secs: u64) {
    let configured = entries().len();
    logger(format!(
        "MCP precache: loop started, every {}s over {} configured entr{}",
        interval_secs,
        configured,
        if configured == 1 { "y" } else { "ies" }
    ));

    tokio::spawn(async move {
        loop {
            sweep().await;
            tokio::time::sleep(Duration::from_secs(interval_secs)).await;
        }
    });
}


#[cfg(test)]
mod tests {
    use super::*;

    fn entry(authcode: &str, pid: i64) -> PrecacheEntry {
        PrecacheEntry {
            label: "Test".into(),
            authcode: authcode.into(),
            pid,
            url: Some("https://example.test/services/vision.asmx".into()),
            enabled: None
        }
    }

    #[test]
    fn entries_are_enabled_unless_switched_off() {
        assert!(entry("AAAA1111BBBB2222", 1).is_enabled());

        let mut disabled = entry("AAAA1111BBBB2222", 1);
        disabled.enabled = Some(false);
        assert!(!disabled.is_enabled());
    }

    #[test]
    fn the_id_is_derived_from_the_hash_not_the_code() {
        let entry = entry("SUPERSECRETAUTHCODE", 9);
        let id = entry.id();
        assert!(!id.contains("SUPERSECRET"));
        assert!(id.ends_with("-9"));
        // Stable across calls, and distinct per authcode and per pid.
        assert_eq!(id, entry.id());
        assert_ne!(id, self::entry("OTHERSECRETAUTHCODE", 9).id());
        assert_ne!(id, self::entry("SUPERSECRETAUTHCODE", 8).id());
    }

    #[test]
    fn masking_never_reveals_the_code() {
        assert_eq!(entry("FFD3ABCDEF120E37", 3).masked(), "FFD3…0E37 pid=3");
    }

    #[test]
    fn serialized_entries_round_trip() {
        let config = PrecacheConfig { entries: vec![entry("AAAA1111BBBB2222", 4)] };
        let text = toml::to_string_pretty(&config).expect("serializes");
        let parsed: PrecacheConfig = toml::from_str(&text).expect("parses");

        assert_eq!(parsed.entries.len(), 1);
        assert_eq!(parsed.entries[0].pid, 4);
        assert_eq!(parsed.entries[0].label, "Test");
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        // The common case on a fresh deployment: nothing is precached yet.
        let config = toml::from_str::<PrecacheConfig>("").expect("empty parses");
        assert!(config.entries.is_empty());
    }
}
