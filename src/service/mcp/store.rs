//! On-disk snapshot store — the second tier behind the in-memory cache.
//!
//! The host this runs on has ~1–1.5 GB of RAM and one catalog snapshot is
//! ~46 MB, so memory can only hold a handful at a time. Disk absorbs the rest:
//! a snapshot evicted from RAM, or lost to a restart, is reloaded from here in
//! well under a second instead of being rebuilt from Octopus in tens of seconds.
//!
//! ## These files are commercially sensitive
//!
//! A snapshot holds a partner's **own negotiated prices** and stock. Files are
//! therefore written `0600` and named by the authcode's hash fingerprint, never
//! by the code itself, so a directory listing reveals neither credentials nor
//! who the entry belongs to. Provision the directory like `mcp_precache.toml`,
//! not like a scratch volume.

use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use flate2::Compression;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;

use crate::service::{
    config::get_mcp_settings,
    log::{elogger, logger},
    mcp::{
        cache::{CacheKey, fingerprint},
        index::{CatalogSnapshot, PersistedSnapshot, SNAPSHOT_VERSION}
    },
    path::get_current_or_root_dir
};

/// Extension marking a stored snapshot: gzipped JSON.
const SNAPSHOT_EXTENSION: &str = "json.gz";

/// Buffer size for the compressed streams, both ways.
///
/// A snapshot is tens of megabytes written a JSON token at a time, so the cost
/// here is entirely in the number of calls, not in the size of any one of them.
const BUFFER_BYTES: usize = 256 * 1024;


/// The cache directory, resolved against the working directory like every other
/// runtime path in this service (`Config.toml`, `soap.json`, `log/`).
pub fn cache_dir() -> PathBuf {
    let configured = get_mcp_settings().disk_path();
    let path = PathBuf::from(&configured);
    if path.is_absolute() {
        return path
    }
    let mut base = get_current_or_root_dir();
    base.push(configured);
    base
}


/// File name for one cache key: `<auth fingerprint>-<pid>.json.gz`.
///
/// Derived from the authcode's hash, so neither the code nor the partner it
/// belongs to can be read off a directory listing. The url is not part of the
/// name — it would have to be sanitized into a path component, and a second
/// Octopus instance is better handled by a second cache directory.
fn file_name(key: &CacheKey) -> String {
    format!("{}-{}.{}", fingerprint(&key.auth_hash), key.pid, SNAPSHOT_EXTENSION)
}


/// Full path for one cache key.
fn path_for(key: &CacheKey) -> PathBuf {
    cache_dir().join(file_name(key))
}


/// Creates a cache directory if it does not exist yet.
fn ensure_dir(dir: &Path) -> Result<(), String> {
    if !dir.is_dir() {
        std::fs::create_dir_all(dir).map_err(|error| format!("cannot create '{:?}': {}", dir, error))?;
        restrict_dir(dir);
    }
    Ok(())
}


/// Narrows a file to owner read/write, a directory to owner-only.
///
/// A failure is logged rather than fatal: refusing to cache at all would be a
/// worse outcome than caching with the platform's default permissions, and the
/// deployment notes call for a directory that is already private.
fn restrict(path: &Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
            elogger(format!("MCP store: cannot restrict permissions on '{:?}': {}", path, error));
        }
    }
    #[cfg(not(unix))]
    {
        // Windows has no direct mode equivalent; inherited ACLs apply.
        let _ = (path, mode);
    }
}

fn restrict_file(path: &Path) {
    restrict(path, 0o600);
}

fn restrict_dir(path: &Path) {
    restrict(path, 0o700);
}


/// Writes a snapshot to disk, replacing any previous one for the same key.
///
/// Written to a temporary file and renamed, so a crash mid-write cannot leave a
/// truncated snapshot that would later deserialize into a partial catalog.
pub fn write(key: &CacheKey, snapshot: &CatalogSnapshot) -> Result<u64, String> {
    let size = write_to(&cache_dir(), key, snapshot)?;
    prune();
    Ok(size)
}


/// [`write`], against an explicit directory. Split out so tests can round-trip
/// through a temporary directory instead of the configured one.
fn write_to(dir: &Path, key: &CacheKey, snapshot: &CatalogSnapshot) -> Result<u64, String> {
    ensure_dir(dir)?;
    let path = dir.join(file_name(key));
    let temporary = path.with_extension("tmp");

    let persisted = PersistedSnapshot::from(snapshot);
    let file = std::fs::File::create(&temporary)
        .map_err(|error| format!("cannot create '{:?}': {}", temporary, error))?;
    restrict_file(&temporary);

    // Fast compression rather than best: this runs on every refresh, and the
    // difference in size is small next to the difference in CPU time.
    //
    // Both buffers matter. `serde_json` emits one write per token — per field
    // name, per string, per comma — so writing straight into the encoder means
    // tens of millions of deflate calls for a 45 MB catalog; the outer
    // `BufWriter` batches those into 256 KB blocks. The inner one does the same
    // for the encoder's output against the file, turning a syscall per
    // compressed chunk into one per block.
    let encoder = GzEncoder::new(BufWriter::with_capacity(BUFFER_BYTES, file), Compression::fast());
    let mut writer = BufWriter::with_capacity(BUFFER_BYTES, encoder);
    serde_json::to_writer(&mut writer, &persisted)
        .map_err(|error| format!("cannot serialize snapshot: {}", error))?;
    let encoder = writer.into_inner()
        .map_err(|error| format!("cannot flush '{:?}': {}", temporary, error))?;
    let mut file = encoder.finish().map_err(|error| format!("cannot finish '{:?}': {}", temporary, error))?;
    file.flush().map_err(|error| format!("cannot flush '{:?}': {}", temporary, error))?;
    drop(file);

    std::fs::rename(&temporary, &path)
        .map_err(|error| format!("cannot rename into '{:?}': {}", path, error))?;
    restrict_file(&path);

    Ok(std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0))
}


/// Reads a snapshot back, or `None` when nothing is stored for this key.
///
/// A corrupt or unreadable file is logged and removed rather than surfaced: the
/// caller can always rebuild from Octopus, and leaving a poison file in place
/// would make every future lookup fail the same way.
pub fn read(key: &CacheKey) -> Option<CatalogSnapshot> {
    read_from(&cache_dir(), key)
}


/// [`read`], against an explicit directory.
fn read_from(dir: &Path, key: &CacheKey) -> Option<CatalogSnapshot> {
    let path = dir.join(file_name(key));
    if !path.is_file() {
        return None
    }

    let started = std::time::Instant::now();
    let outcome = std::fs::File::open(&path)
        .map_err(|error| error.to_string())
        .and_then(|file| {
            let mut decoder = GzDecoder::new(BufReader::with_capacity(BUFFER_BYTES, file));
            let mut buffer = String::new();
            decoder.read_to_string(&mut buffer)
                .map_err(|error| error.to_string())
                .map(|_| buffer)
        })
        .and_then(|buffer| {
            serde_json::from_str::<PersistedSnapshot>(&buffer).map_err(|error| error.to_string())
        })
        .and_then(|persisted| {
            // A file from an older layout parses cleanly but means something
            // different — see `SNAPSHOT_VERSION`. Treat it like corruption: the
            // error arm below removes it and the caller rebuilds from Octopus.
            if persisted.version != SNAPSHOT_VERSION {
                return Err(format!(
                    "written by snapshot layout v{}, this build reads v{}",
                    persisted.version, SNAPSHOT_VERSION
                ))
            }
            Ok(persisted)
        });

    match outcome {
        Ok(persisted) => {
            let snapshot = CatalogSnapshot::from(persisted);
            logger(format!(
                "MCP store: loaded {} from disk — {} products, {:.1} MB, {:.2}s",
                file_name(key),
                snapshot.products.len(),
                snapshot.bytes as f64 / 1_048_576.0,
                started.elapsed().as_secs_f64()
            ));
            Some(snapshot)
        }
        Err(error) => {
            elogger(format!("MCP store: discarding unreadable '{:?}': {}", path, error));
            if let Err(error) = std::fs::remove_file(&path) {
                elogger(format!("MCP store: cannot remove '{:?}': {}", path, error));
            }
            None
        }
    }
}


/// Deletes one stored snapshot. Missing files are not an error.
pub fn remove(key: &CacheKey) {
    let path = path_for(key);
    if path.is_file()
        && let Err(error) = std::fs::remove_file(&path) {
            elogger(format!("MCP store: cannot remove '{:?}': {}", path, error));
    }
}


/// Whether a snapshot is stored for this key, without reading it.
pub fn contains(key: &CacheKey) -> bool {
    path_for(key).is_file()
}


/// How old the stored snapshot is, in seconds, from the file's modification
/// time — without decompressing tens of megabytes to find out.
///
/// The file is written once when the snapshot is built, so its mtime tracks the
/// snapshot's `fetched_at` closely enough to decide whether a refresh is due.
pub fn stored_age_secs(key: &CacheKey) -> Option<i64> {
    let modified = std::fs::metadata(path_for(key)).ok()?.modified().ok()?;
    let elapsed = std::time::SystemTime::now().duration_since(modified).ok()?;
    Some(elapsed.as_secs() as i64)
}


/// Every stored snapshot as `(path, size, modified)`, newest first.
fn stored_files() -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let Ok(entries) = std::fs::read_dir(cache_dir()) else {
        return Vec::new()
    };

    let mut files: Vec<(PathBuf, u64, std::time::SystemTime)> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_name().to_string_lossy().ends_with(SNAPSHOT_EXTENSION))
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            let modified = metadata.modified().ok()?;
            Some((entry.path(), metadata.len(), modified))
        })
        .collect();

    files.sort_by_key(|(_, _, modified)| std::cmp::Reverse(*modified));
    files
}


/// Total bytes currently stored, and how many snapshots that is.
pub fn usage() -> (u64, usize) {
    let files = stored_files();
    (files.iter().map(|(_, size, _)| size).sum(), files.len())
}


/// Drops the oldest snapshots until the directory fits its budget.
///
/// Oldest-first is right here, unlike in memory: the RAM tier evicts by
/// recency because it is answering live queries, while disk is a fallback whose
/// only job is to be cheaper than a rebuild. An old snapshot is the one closest
/// to being stale anyway.
fn prune() {
    let budget = get_mcp_settings().disk_max_bytes();
    let files = stored_files();
    let mut total: u64 = files.iter().map(|(_, size, _)| size).sum();

    if total <= budget {
        return
    }

    // `stored_files` is newest-first, so walk it backwards.
    for (path, size, _) in files.iter().rev() {
        if total <= budget {
            break
        }
        match std::fs::remove_file(path) {
            Ok(()) => {
                total = total.saturating_sub(*size);
                logger(format!("MCP store: pruned '{:?}' to stay inside the disk budget", path));
            }
            Err(error) => elogger(format!("MCP store: cannot prune '{:?}': {}", path, error))
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::service::mcp::index::test_snapshot_with_products;

    fn key(authcode: &str, pid: i64) -> CacheKey {
        CacheKey::new(authcode, pid, "https://example.test/services/vision.asmx")
    }

    #[test]
    fn file_names_reveal_neither_the_code_nor_the_partner_name() {
        let name = file_name(&key("SUPERSECRETAUTHCODE", 7824));
        assert!(!name.contains("SUPERSECRET"));
        assert!(name.ends_with("-7824.json.gz"));
        // Stable, so the same combination always maps to the same file.
        assert_eq!(name, file_name(&key("SUPERSECRETAUTHCODE", 7824)));
    }

    #[test]
    fn different_combinations_map_to_different_files() {
        assert_ne!(file_name(&key("AAAA1111BBBB2222", 1)), file_name(&key("AAAA1111BBBB2222", 2)));
        assert_ne!(file_name(&key("AAAA1111BBBB2222", 1)), file_name(&key("CCCC3333DDDD4444", 1)));
    }

    /// A scratch directory that cleans itself up, so tests never write into the
    /// configured cache directory.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("rustopus-store-{}-{}", name, std::process::id()));
            let _ = std::fs::remove_dir_all(&path);
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_snapshot_round_trips_through_the_filesystem() {
        let dir = TempDir::new("roundtrip");
        let key = key("AAAA1111BBBB2222", 42);
        let snapshot = test_snapshot_with_products(50);

        let size = write_to(&dir.0, &key, &snapshot).expect("writes");
        assert!(size > 0, "wrote an empty file");

        let restored = read_from(&dir.0, &key).expect("reads back");
        assert_eq!(restored.products.len(), 50);
        // Age is preserved rather than reset to "now" on load — otherwise every
        // restart would make stale data look fresh.
        assert_eq!(restored.fetched_at, snapshot.fetched_at);
        // Derived data is rebuilt on load rather than stored.
        assert_eq!(restored.by_sku.len(), 50);
        assert!(restored.bytes > 0);
        // And the reloaded snapshot is actually searchable.
        assert_eq!(restored.get_by_no("A-7").map(|p| p.no.as_str()), Some("A-7"));
    }

    #[test]
    fn a_snapshot_from_an_older_layout_is_discarded_rather_than_served() {
        let dir = TempDir::new("version");
        let key = key("AAAA1111BBBB2222", 1);
        ensure_dir(&dir.0).expect("creates dir");

        // Hand-written at the previous layout: parses fine, but its `price` held
        // Octopus's `ar` rather than `akcios_ar`, so serving it would quote a
        // retail figure as the partner's own.
        let stale = PersistedSnapshot {
            version: SNAPSHOT_VERSION - 1,
            products: test_snapshot_with_products(3).products,
            fetched_at: chrono::Utc::now()
        };
        let path = dir.0.join(file_name(&key));
        let file = std::fs::File::create(&path).expect("creates file");
        let mut encoder = flate2::write::GzEncoder::new(file, Compression::fast());
        serde_json::to_writer(&mut encoder, &stale).expect("writes");
        encoder.finish().expect("finishes");

        assert!(read_from(&dir.0, &key).is_none(), "a stale layout was served");
        assert!(!path.exists(), "the stale file was left to fail every future load");
    }

    #[test]
    fn a_missing_snapshot_reads_as_none() {
        let dir = TempDir::new("missing");
        assert!(read_from(&dir.0, &key("AAAA1111BBBB2222", 1)).is_none());
    }

    #[test]
    fn a_corrupt_file_is_discarded_rather_than_surfaced() {
        let dir = TempDir::new("corrupt");
        let key = key("AAAA1111BBBB2222", 1);
        ensure_dir(&dir.0).expect("creates dir");

        let path = dir.0.join(file_name(&key));
        std::fs::write(&path, b"this is not gzipped json").expect("writes garbage");

        // A poison file must not make every future lookup fail the same way.
        assert!(read_from(&dir.0, &key).is_none());
        assert!(!path.exists(), "the unreadable file was left in place");
    }

    #[test]
    fn stored_files_are_owner_only() {
        let dir = TempDir::new("perms");
        let key = key("AAAA1111BBBB2222", 1);
        write_to(&dir.0, &key, &test_snapshot_with_products(2)).expect("writes");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(dir.0.join(file_name(&key))).expect("stat").permissions().mode();
            // These files hold a partner's own negotiated prices.
            assert_eq!(mode & 0o777, 0o600, "snapshot file is group- or world-readable");

            let dir_mode = std::fs::metadata(&dir.0).expect("stat").permissions().mode();
            assert_eq!(dir_mode & 0o777, 0o700, "cache directory is group- or world-readable");
        }
    }
}
