//! Access blocklist: IP addresses and authcodes refused before a request ever
//! reaches a route.
//!
//! ## Why this lives outside `service/mcp/`
//!
//! Abuse is not an MCP problem. The same client can hammer `/get-bulk` and
//! `/mcp`, and the REST-only instance (see `DOCKER_PLAN.md`) runs with
//! `[mcp] enabled = false`, where nothing under `service/mcp/` is wired up. So
//! the rules, the matching and the middleware live here, and `/admin` — which
//! manages them — is registered on any instance that has an admin token, MCP or
//! not.
//!
//! ## Why this is not a credential file
//!
//! An authcode rule stores the **SHA-256 of the code**, never the code, plus its
//! `FFD3…0E37` mask for display. Blocking therefore costs one hash per request
//! and `blocklist.toml` leaks nothing if it is read — unlike `mcp_precache.toml`,
//! which has to hold live codes because its job runs unattended. It is still
//! written `0600` through a temp-file-and-rename: nothing but this server needs
//! to read it, and a crash mid-write must not leave half a ruleset behind.
//!
//! ## One choke point
//!
//! Enforcement is a single `from_fn` middleware wrapping the whole app, so the
//! nine REST endpoints, `/mcp` and `/export/{token}` are all covered by the same
//! rule set without a line of per-route plumbing. `/admin` is deliberately
//! exempt — an administrator who blocks their own address must not lock
//! themselves out of the page that would undo it.

use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, RwLock};

use actix_web::body::{BoxBody, MessageBody};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::middleware::Next;
use actix_web::{Error, HttpResponse, web};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::{
    global::errors::GLOBAL_BLOCKED_ERROR,
    service::{
        log::{elog_with_ip, elogger, logger},
        mcp::{
            cache::{fingerprint, hash_authcode},
            mask_authcode
        },
        path::get_current_or_root_dir
    }
};

/// Query parameters carrying an authcode on the REST endpoints. Mirrors
/// [`crate::routes::default::get_auth`], which accepts either spelling.
const AUTH_PARAMS: [&str; 2] = ["authcode", "auth"];

/// Path prefix that is never blocked. See the module note: the dashboard is the
/// only way back from a rule that matches the administrator's own address.
const EXEMPT_PREFIX: &str = "/admin";


/// What a rule matches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockKind {
    /// A single address, or a CIDR range such as `203.0.113.0/24`.
    Ip,
    /// One Octopus authcode, held as a hash.
    Authcode
}

impl BlockKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockKind::Ip => "ip",
            BlockKind::Authcode => "authcode"
        }
    }
}


/// Which surface a rule applies to.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BlockScope {
    /// Everything this server exposes except `/admin`.
    #[default]
    All,
    /// The REST fetchers and `/post-order` only.
    Rest,
    /// `/mcp` and the export downloads it hands out.
    Mcp
}

impl BlockScope {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockScope::All => "all",
            BlockScope::Rest => "rest",
            BlockScope::Mcp => "mcp"
        }
    }

    fn covers(&self, surface: Surface) -> bool {
        match self {
            BlockScope::All => true,
            BlockScope::Rest => surface == Surface::Rest,
            BlockScope::Mcp => surface == Surface::Mcp
        }
    }
}


/// Which surface a request arrived on, derived from its path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    Rest,
    Mcp
}

/// `/mcp` is the protocol transport; `/export` exists only to hand a generated
/// file to someone an MCP tool sent there, so it belongs to the same surface.
/// Everything else is the REST API and its docs.
fn surface_of(path: &str) -> Surface {
    if path.starts_with("/mcp") || path.starts_with("/export") {
        return Surface::Mcp
    }
    Surface::Rest
}


/// One blocking rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockRule {
    pub kind: BlockKind,
    /// For an IP rule: the address or CIDR range, verbatim.
    /// For an authcode rule: the **SHA-256 hex of the code** — never the code.
    pub value: String,
    /// How the rule is shown: the range for an IP, `FFD3…0E37` for an authcode.
    pub label: String,
    /// Free-text reason, for whoever reads this file in six months.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<BlockScope>,
    /// Set `false` to keep a rule on file but stop enforcing it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateTime<Utc>>
}

impl BlockRule {
    /// Builds an IP rule, or explains why the value is not an address or range.
    pub fn ip(value: &str, note: Option<String>, scope: Option<BlockScope>) -> Result<Self, String> {
        let value = value.trim().to_string();
        if matches!(compile_ip(&value), Matcher::Invalid) {
            return Err(format!("'{}' is not an IP address or CIDR range", value))
        }
        Ok(Self {
            kind: BlockKind::Ip,
            label: value.clone(),
            value,
            note,
            scope,
            enabled: Some(true),
            created_at: Some(Utc::now())
        })
    }

    /// Builds an authcode rule from a full code. The code is hashed here and
    /// **not retained**: what is stored is the hash and the mask.
    pub fn authcode(code: &str, note: Option<String>, scope: Option<BlockScope>) -> Result<Self, String> {
        let code = code.trim();
        if code.is_empty() {
            return Err("authcode is required".into())
        }
        Ok(Self {
            kind: BlockKind::Authcode,
            value: hash_hex(code),
            label: mask_authcode(code),
            note,
            scope,
            enabled: Some(true),
            created_at: Some(Utc::now())
        })
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.unwrap_or(true)
    }

    pub fn scope(&self) -> BlockScope {
        self.scope.unwrap_or_default()
    }

    /// Stable identifier derived from what the rule matches on. Safe in a URL,
    /// a log line or a dashboard row: for an authcode rule the seed is already
    /// a hash, so this reveals nothing the label does not.
    pub fn id(&self) -> String {
        let seed = format!("{}:{}", self.kind.as_str(), self.value.to_lowercase());
        fingerprint(&hash_authcode(&seed))
    }
}


/// Full SHA-256 hex of an authcode. Uses the same one-way hash as the MCP cache
/// key rather than a second construction of its own.
fn hash_hex(authcode: &str) -> String {
    hash_authcode(authcode).iter()
        .map(|byte| format!("{:02x}", byte))
        .collect()
}


/// A rule with its match test resolved once, at load time, instead of parsing an
/// address on every request.
#[derive(Debug, Clone)]
struct Compiled {
    rule: BlockRule,
    matcher: Matcher
}

#[derive(Debug, Clone)]
enum Matcher {
    Address(IpAddr),
    Network(IpAddr, u32),
    AuthHash(String),
    /// A rule that cannot be parsed. Kept visible in the dashboard rather than
    /// dropped silently, but it never matches anything.
    Invalid
}


fn compile(rule: &BlockRule) -> Compiled {
    let matcher = match rule.kind {
        BlockKind::Ip => compile_ip(&rule.value),
        BlockKind::Authcode => {
            let hash = rule.value.trim().to_lowercase();
            // A hex SHA-256 is 64 characters; anything else was hand-edited into
            // the file and would match nothing anyway.
            if hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit()) {
                Matcher::AuthHash(hash)
            } else {
                Matcher::Invalid
            }
        }
    };
    Compiled { rule: rule.clone(), matcher }
}


/// Parses `203.0.113.7` or `203.0.113.0/24` (v6 equally).
fn compile_ip(value: &str) -> Matcher {
    let value = value.trim();
    let Some((address, prefix)) = value.split_once('/') else {
        return match value.parse::<IpAddr>() {
            Ok(address) => Matcher::Address(normalize(address)),
            Err(_) => Matcher::Invalid
        }
    };

    let Ok(address) = address.trim().parse::<IpAddr>() else {
        return Matcher::Invalid
    };
    let Ok(prefix) = prefix.trim().parse::<u32>() else {
        return Matcher::Invalid
    };
    let bits = if address.is_ipv4() { 32 } else { 128 };
    if prefix > bits {
        return Matcher::Invalid
    }
    Matcher::Network(address, prefix)
}


/// Collapses an IPv4-mapped IPv6 address (`::ffff:203.0.113.7`) to its IPv4 form,
/// so a rule written the way an operator reads the address in a log still
/// matches when the socket reports the mapped shape.
fn normalize(address: IpAddr) -> IpAddr {
    match address {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map(IpAddr::V4).unwrap_or(IpAddr::V6(v6)),
        other => other
    }
}


/// Whether `candidate` falls inside `network/prefix`.
fn in_network(candidate: IpAddr, network: IpAddr, prefix: u32) -> bool {
    let (left, right): (Vec<u8>, Vec<u8>) = match (normalize(candidate), normalize(network)) {
        (IpAddr::V4(a), IpAddr::V4(b)) => (a.octets().into(), b.octets().into()),
        (IpAddr::V6(a), IpAddr::V6(b)) => (a.octets().into(), b.octets().into()),
        // A v4 rule never matches a v6 caller, or the other way round.
        _ => return false
    };

    let mut remaining = prefix;
    for (a, b) in left.iter().zip(right.iter()) {
        if remaining == 0 {
            return true
        }
        if remaining >= 8 {
            if a != b {
                return false
            }
            remaining -= 8;
            continue
        }
        let mask = 0xFFu8 << (8 - remaining);
        return a & mask == b & mask
    }
    true
}


impl Compiled {
    fn matches_ip(&self, candidate: IpAddr) -> bool {
        match &self.matcher {
            Matcher::Address(address) => normalize(candidate) == *address,
            Matcher::Network(network, prefix) => in_network(candidate, *network, *prefix),
            _ => false
        }
    }

    fn matches_authcode(&self, hash: &str) -> bool {
        match &self.matcher {
            Matcher::AuthHash(expected) => expected == hash,
            _ => false
        }
    }
}


/// On-disk shape of `blocklist.toml`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BlocklistConfig {
    #[serde(default, rename = "rule")]
    pub rules: Vec<BlockRule>
}


/// What a rule has actually stopped. In memory only, like the precache run log:
/// a counter is not worth rewriting a file for on every blocked request.
#[derive(Debug, Clone, Default)]
pub struct RuleHits {
    pub count: u64,
    pub last_at: Option<DateTime<Utc>>,
    pub last_ip: Option<String>
}

static HITS: Lazy<Mutex<HashMap<String, RuleHits>>> = Lazy::new(|| Mutex::new(HashMap::new()));

/// The compiled rule set, rebuilt on every edit.
static RULES: Lazy<RwLock<Vec<Compiled>>> = Lazy::new(|| RwLock::new(compile_all(&load().rules)));

/// How many rules are currently enforced.
///
/// Read before anything else on every request, so an instance with no rules
/// pays one relaxed atomic load and never touches the lock, the query string or
/// the headers. [`init`] primes it at startup; without that call this would read
/// zero until something else forced `RULES`.
static ARMED: AtomicUsize = AtomicUsize::new(0);


fn compile_all(rules: &[BlockRule]) -> Vec<Compiled> {
    let compiled: Vec<Compiled> = rules.iter().map(compile).collect();
    ARMED.store(compiled.iter().filter(|entry| entry.rule.is_enabled()).count(), Ordering::Relaxed);
    compiled
}


/// Path to `blocklist.toml`, resolved against the working directory like
/// `soap.json`, `Config.toml` and `mcp_precache.toml`.
pub fn get_blocklist_path() -> PathBuf {
    let mut path = get_current_or_root_dir();
    path.push("blocklist.toml");
    path
}


/// Reads `blocklist.toml`, or an empty rule set when it is absent or unreadable.
/// A missing file is the normal case: nobody has been blocked yet.
pub fn load() -> BlocklistConfig {
    let path = get_blocklist_path();
    if !path.is_file() {
        return BlocklistConfig::default()
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<BlocklistConfig>(&content) {
            Ok(config) => config,
            Err(error) => {
                elogger(format!("Blocklist: cannot parse '{:?}': {}", path, error));
                BlocklistConfig::default()
            }
        },
        Err(error) => {
            elogger(format!("Blocklist: cannot read '{:?}': {}", path, error));
            BlocklistConfig::default()
        }
    }
}


/// Writes `blocklist.toml` through a temp file and a rename, owner-only.
pub fn save(config: &BlocklistConfig) -> Result<(), String> {
    let path = get_blocklist_path();
    let body = toml::to_string_pretty(config).map_err(|error| error.to_string())?;
    let content = format!(
        "# Rustopus access blocklist.\n\
         #\n\
         # Requests matching an enabled rule are refused before they reach a route.\n\
         # Authcode rules hold the SHA-256 of the code, never the code itself, so\n\
         # this file is not a credential store — but nothing except this server\n\
         # needs to read it, so it is written 0600 all the same.\n\
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


/// Narrows a file to owner read/write. Logged rather than fatal — the
/// alternative is refusing to save an administrator's rule at all.
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)) {
            elogger(format!("Blocklist: cannot restrict permissions on '{:?}': {}", path, error));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
}


/// Loads the rule set and reports it. Called from `main.rs` at startup so the
/// armed count is primed before the first request arrives.
pub fn init() {
    let rules = rules();
    let active = rules.iter().filter(|rule| rule.is_enabled()).count();
    if rules.is_empty() {
        logger("Blocklist: no rules configured");
        return
    }
    logger(format!(
        "Blocklist: {} rule{} loaded, {} enforced",
        rules.len(),
        if rules.len() == 1 { "" } else { "s" },
        active
    ));
}


/// Every configured rule, enabled or not.
pub fn rules() -> Vec<BlockRule> {
    RULES.read()
        .map(|rules| rules.iter().map(|entry| entry.rule.clone()).collect())
        .unwrap_or_default()
}


/// One rule by id.
pub fn find(id: &str) -> Option<BlockRule> {
    rules().into_iter().find(|rule| rule.id() == id)
}


/// Hit counters per rule id, for the dashboard.
pub fn hits() -> HashMap<String, RuleHits> {
    HITS.lock()
        .map(|hits| hits.clone())
        .unwrap_or_default()
}


/// Adds a rule, or replaces the one matching the same thing, and persists it.
pub fn upsert(rule: BlockRule) -> Result<(), String> {
    let mut current = rules();
    let id = rule.id();
    match current.iter().position(|existing| existing.id() == id) {
        // Keep the original creation time: this is the same rule being edited,
        // not a new one.
        Some(position) => {
            let created_at = current[position].created_at.or(rule.created_at);
            current[position] = BlockRule { created_at, ..rule };
        }
        None => current.push(rule)
    }
    commit(current)
}


/// Removes a rule by id. Returns whether it existed.
pub fn remove(id: &str) -> Result<bool, String> {
    let mut current = rules();
    let before = current.len();
    current.retain(|rule| rule.id() != id);
    let removed = current.len() != before;
    commit(current)?;
    if removed && let Ok(mut hits) = HITS.lock() {
        hits.remove(id);
    }
    Ok(removed)
}


/// Persists a new rule set and swaps in its compiled form.
fn commit(rules: Vec<BlockRule>) -> Result<(), String> {
    save(&BlocklistConfig { rules: rules.clone() })?;
    let compiled = compile_all(&rules);
    match RULES.write() {
        Ok(mut current) => *current = compiled,
        Err(_) => return Err("blocklist lock poisoned".into())
    }
    Ok(())
}


/// What matched, for the log line and the counter.
#[derive(Debug, Clone)]
pub struct BlockMatch {
    pub id: String,
    pub kind: BlockKind,
    pub label: String
}


/// Tests one request's identity against the rule set.
///
/// `addresses` holds every address the request can be attributed to — both the
/// forwarded one and the peer — because a client talking to this server directly
/// could otherwise send a fabricated `X-Forwarded-For` and walk past its own
/// block.
pub fn check(addresses: &[IpAddr], authcode: Option<&str>, surface: Surface) -> Option<BlockMatch> {
    let rules = RULES.read().ok()?;
    let hash = authcode.map(hash_hex);

    for entry in rules.iter() {
        if !entry.rule.is_enabled() || !entry.rule.scope().covers(surface) {
            continue
        }
        let hit = match entry.rule.kind {
            BlockKind::Ip => addresses.iter().any(|address| entry.matches_ip(*address)),
            BlockKind::Authcode => hash.as_deref().is_some_and(|hash| entry.matches_authcode(hash))
        };
        if hit {
            return Some(BlockMatch {
                id: entry.rule.id(),
                kind: entry.rule.kind,
                label: entry.rule.label.clone()
            })
        }
    }
    None
}


fn record_hit(id: &str, address: Option<&str>) {
    if let Ok(mut hits) = HITS.lock() {
        let hit = hits.entry(id.to_string()).or_default();
        hit.count += 1;
        hit.last_at = Some(Utc::now());
        hit.last_ip = address.map(str::to_string);
    }
}


/// Addresses a request can be attributed to: the first `X-Forwarded-For` hop
/// (what the reverse proxy saw) and the socket peer.
fn request_addresses(request: &ServiceRequest) -> (Vec<IpAddr>, Option<String>) {
    let mut addresses = Vec::new();
    let mut display = None;

    if let Some(forwarded) = request.headers()
        .get("X-Forwarded-For")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(',').next())
        .map(str::trim)
        .filter(|value| !value.is_empty()) {
            display = Some(forwarded.to_string());
            if let Ok(address) = forwarded.parse::<IpAddr>() {
                addresses.push(address);
            }
    }

    if let Some(peer) = request.peer_addr() {
        let address = peer.ip();
        if !addresses.contains(&address) {
            addresses.push(address);
        }
        if display.is_none() {
            display = Some(address.to_string());
        }
    }

    (addresses, display)
}


/// The authcode a request presents: the MCP header, or either spelling of the
/// REST query parameter.
fn request_authcode(request: &ServiceRequest) -> Option<String> {
    if let Some(header) = request.headers()
        .get(crate::service::mcp::AUTHCODE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty()) {
            return Some(header.to_string())
    }

    let query = request.query_string();
    if query.is_empty() {
        return None
    }
    let parsed = web::Query::<HashMap<String, String>>::from_query(query).ok()?;
    AUTH_PARAMS.iter()
        .find_map(|name| parsed.get(*name))
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}


/// The response a blocked caller gets: `403` with the house error shape, so a
/// partner blocked by mistake sees a code they can quote rather than a bare
/// status line.
fn forbidden() -> HttpResponse {
    let error = GLOBAL_BLOCKED_ERROR;
    HttpResponse::Forbidden()
        .content_type("application/xml")
        .body(format!(
            "<?xml version=\"1.0\" encoding=\"utf-8\"?><error><code>{}</code><description>{}</description></error>",
            error.code, error.description
        ))
}


/// The middleware. Wraps the whole app, so it covers the REST fetchers, `/mcp`,
/// `/export/{token}` and the docs alike — everything but `/admin`.
pub async fn guard(
    request: ServiceRequest,
    next: Next<impl MessageBody + 'static>
) -> Result<ServiceResponse<BoxBody>, Error> {
    // Nothing configured: one atomic load, then straight through.
    if ARMED.load(Ordering::Relaxed) == 0 || request.path().starts_with(EXEMPT_PREFIX) {
        return next.call(request).await.map(ServiceResponse::map_into_boxed_body)
    }

    let (addresses, display) = request_addresses(&request);
    let authcode = request_authcode(&request);
    let surface = surface_of(request.path());

    if let Some(hit) = check(&addresses, authcode.as_deref(), surface) {
        record_hit(&hit.id, display.as_deref());
        let address = display.unwrap_or_else(|| "unknown IP address".into());
        elog_with_ip(&address, format!(
            "{}: {} — blocked by {} rule '{}' on {} {}",
            GLOBAL_BLOCKED_ERROR.code,
            GLOBAL_BLOCKED_ERROR.description,
            hit.kind.as_str(),
            hit.label,
            request.method(),
            request.path()
        ));
        return Ok(request.into_response(forbidden()))
    }

    next.call(request).await.map(ServiceResponse::map_into_boxed_body)
}


#[cfg(test)]
mod tests {
    use super::*;

    fn ip(value: &str) -> IpAddr {
        value.parse().expect("test address parses")
    }

    #[test]
    fn a_single_address_matches_only_itself() {
        let rule = BlockRule::ip("203.0.113.7", None, None).expect("valid");
        let compiled = compile(&rule);
        assert!(compiled.matches_ip(ip("203.0.113.7")));
        assert!(!compiled.matches_ip(ip("203.0.113.8")));
    }

    #[test]
    fn a_cidr_range_matches_its_whole_network() {
        let rule = BlockRule::ip("203.0.113.0/24", None, None).expect("valid");
        let compiled = compile(&rule);
        assert!(compiled.matches_ip(ip("203.0.113.0")));
        assert!(compiled.matches_ip(ip("203.0.113.255")));
        assert!(!compiled.matches_ip(ip("203.0.114.1")));
    }

    #[test]
    fn a_partial_byte_prefix_is_masked_not_rounded() {
        // /20 covers 198.51.48.0 – 198.51.63.255.
        let compiled = compile(&BlockRule::ip("198.51.48.0/20", None, None).expect("valid"));
        assert!(compiled.matches_ip(ip("198.51.63.255")));
        assert!(!compiled.matches_ip(ip("198.51.64.0")));
    }

    #[test]
    fn a_mapped_v6_peer_matches_the_v4_rule_an_operator_wrote() {
        let compiled = compile(&BlockRule::ip("203.0.113.7", None, None).expect("valid"));
        assert!(compiled.matches_ip(ip("::ffff:203.0.113.7")));
    }

    #[test]
    fn v4_and_v6_never_match_each_other() {
        let compiled = compile(&BlockRule::ip("2001:db8::/32", None, None).expect("valid"));
        assert!(compiled.matches_ip(ip("2001:db8::1")));
        assert!(!compiled.matches_ip(ip("203.0.113.7")));
    }

    #[test]
    fn nonsense_addresses_are_refused_at_the_door() {
        assert!(BlockRule::ip("not-an-address", None, None).is_err());
        assert!(BlockRule::ip("203.0.113.0/40", None, None).is_err());
        assert!(BlockRule::ip("", None, None).is_err());
    }

    #[test]
    fn an_authcode_rule_stores_the_hash_and_the_mask_only() {
        let rule = BlockRule::authcode("FFD3ABCDEF120E37", None, None).expect("valid");
        assert_eq!(rule.label, "FFD3…0E37");
        assert_eq!(rule.value.len(), 64);
        assert!(!rule.value.contains("ABCDEF"));

        let compiled = compile(&rule);
        assert!(compiled.matches_authcode(&hash_hex("FFD3ABCDEF120E37")));
        assert!(!compiled.matches_authcode(&hash_hex("OTHERCODE1234567")));
    }

    #[test]
    fn the_id_is_stable_and_distinguishes_rules() {
        let one = BlockRule::ip("203.0.113.7", None, None).expect("valid");
        assert_eq!(one.id(), one.id());
        assert_ne!(one.id(), BlockRule::ip("203.0.113.8", None, None).expect("valid").id());
        // Same value, different kind: still a different rule.
        assert_ne!(one.id(), BlockRule::authcode("203.0.113.7", None, None).expect("valid").id());
    }

    #[test]
    fn scopes_select_the_surface_they_name() {
        assert!(BlockScope::All.covers(Surface::Rest));
        assert!(BlockScope::All.covers(Surface::Mcp));
        assert!(BlockScope::Rest.covers(Surface::Rest));
        assert!(!BlockScope::Rest.covers(Surface::Mcp));
        assert!(BlockScope::Mcp.covers(Surface::Mcp));
        assert!(!BlockScope::Mcp.covers(Surface::Rest));
    }

    #[test]
    fn exports_count_as_the_mcp_surface() {
        assert_eq!(surface_of("/mcp"), Surface::Mcp);
        assert_eq!(surface_of("/export/abc123"), Surface::Mcp);
        assert_eq!(surface_of("/get-product"), Surface::Rest);
        assert_eq!(surface_of("/"), Surface::Rest);
    }

    #[test]
    fn a_disabled_rule_is_kept_but_not_enforced() {
        let mut rule = BlockRule::ip("203.0.113.7", None, None).expect("valid");
        assert!(rule.is_enabled());
        rule.enabled = Some(false);
        assert!(!rule.is_enabled());
    }

    #[test]
    fn serialized_rules_round_trip() {
        let config = BlocklistConfig {
            rules: vec![
                BlockRule::ip("203.0.113.0/24", Some("scraper".into()), Some(BlockScope::Rest)).expect("valid"),
                BlockRule::authcode("FFD3ABCDEF120E37", None, None).expect("valid")
            ]
        };
        let text = toml::to_string_pretty(&config).expect("serializes");
        // The plain code never reaches the file.
        assert!(!text.contains("FFD3ABCDEF120E37"));

        let parsed: BlocklistConfig = toml::from_str(&text).expect("parses");
        assert_eq!(parsed.rules.len(), 2);
        assert_eq!(parsed.rules[0].scope(), BlockScope::Rest);
        assert_eq!(parsed.rules[1].kind, BlockKind::Authcode);
    }

    #[test]
    fn a_missing_file_is_not_an_error() {
        let config = toml::from_str::<BlocklistConfig>("").expect("empty parses");
        assert!(config.rules.is_empty());
    }
}
