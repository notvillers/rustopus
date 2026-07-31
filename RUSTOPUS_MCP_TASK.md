# Task brief: add an MCP endpoint to Rustopus

**Hand this file to Claude Code running in the `rustopus` repo.** It is self-contained —
it assumes you have not seen the conversation that produced it.

---

## 0. Read before writing anything

1. `CLAUDE.md` in the repo root. Its conventions are binding and this brief assumes them.
2. `DOCKER_PLAN.md` — the deployment model this builds on.
3. `src/service/soap.rs`, `src/service/get_data.rs`, `src/routes/default.rs`,
   `src/routes/bulk.rs`. You will call into the first two and imitate the last two.

The file/function references below were taken from a read-only pass over the repo.
**Verify each signature before you rely on it** rather than trusting this document.

---

## 1. Context you do not have

Colleagues should be able to ask Claude about Orink product data. A local stdio MCP
server was prototyped in TypeScript and rejected: it needs Node.js installed on every
machine, and Claude Cowork fails silently without it. So MCP moves server-side into
Rustopus.

**The number driving the whole design**, measured against production:

```
GET https://api.orinkhungary.hu/get-product  →  200, 46.3 MB, 27.8 s
```

24,273 products, ~77 fields each. There is no cache in Rustopus today —
`get_response_shared` in `src/service/soap.rs` coalesces *concurrent* identical calls,
but two calls a minute apart both hit Octopus. A 28-second tool call is unusable in a
chat. Everything below exists to make that milliseconds **for MCP callers only**.

### Decisions already made — do not relitigate

| Decision | Choice |
| --- | --- |
| Where MCP lives | New endpoint inside Rustopus |
| Cache key | `sha256(authcode) + pid + url`. Colleagues sharing a combo share the entry |
| Cache budget | 2–3 GB global, weight-based LRU eviction |
| Cache scope | **MCP only.** The eight existing endpoints stay uncached |
| Precache | Configured in an admin dashboard, refreshes hourly |
| Partner context | One fixed `pid` per user, sent with their authcode |
| Deployment | Same image run twice; MCP in its own container |

---

## 2. What must NOT change

This is the most important section. A regression here breaks live consumers.

- **`src/service/soap.rs` gains no cache.** Caching there would silently change all eight
  existing endpoints. The new cache lives in `src/service/mcp/` and wraps
  `get_data.rs` at a higher level.
- **No behaviour change to** `routes/{product,stock,price,image,barcode,bulk,invoice,mat,order}.rs`.
  Read them; do not edit them.
- **`/post-order` keeps raw `get_response`**, never `get_response_shared` and never the
  new cache. Coalescing or caching a mutating call merges intentional submissions.
- **`RequestParameters` keeps working exactly as today**, including the `authcode`/`auth`
  alias and the `url` fallback to `soap.json`.
- When `[mcp] enabled = false` (the default), **none of the new code runs** — no routes
  registered, no background task spawned, no memory held.

Existing integration test `tests/get_test.rs` must still pass untouched.

---

## 3. Binding conventions (from `CLAUDE.md`)

- **`cargo check` after every code edit.** Not at the end — after each one.
- **No `.unwrap()` on real paths.** Use `match` / `if let` / `?` and surface a
  `RustopusError`. The panic hook in `main.rs` is a safety net, not the policy.
- **Route + alias pairing**: real logic in a private `handler`, exposed as `get` and
  `get_alias`; register **both** in `main.rs`. Each `_alias` must point at its own module.
- **Reuse `src/routes/default.rs` helpers** (`get_auth`, `get_url`, `get_xmlns`, `get_pid`,
  `get_date`, `get_i64`, `send_xml`, `send_csv`, `send_xlsx`) instead of reimplementing
  parameter plumbing.
- **Declare models through `src/macros/` wrappers**, not bare `#[derive]`.
- **Errors**: numeric `RustopusError` consts in `src/global/errors.rs`, human-readable
  messages in `src/errors/errors.json`. Existing codes include `GLOBAL_AUTH_ERROR` (201),
  `GLOBAL_URL_ERROR` (202), `GLOBAL_PID_ERROR` (203), `GLOBAL_MISSING_ERROR` (299).
  Add new MCP codes in a fresh block; do not renumber existing ones.
- **Request identity pattern**, in this order: `let uuid = get_uuid();` →
  `let ip_address = log_ip(req).await...;` → log before the external call → log after.
- **Do not clone `CallData` to read it.** `is_hu()`/`is_csv()` take `&self`.
- **The server resolves resources relative to CWD** (`Config.toml`, `soap.json`,
  `src/errors/errors.json`, `src/static/docs`, `log/`). Anything new must follow the same
  rule and be added to `DOCKER_PLAN.md`'s image layout.
- **CSP is `script-src 'self'` with no inline scripts** (`main.rs::security_headers()`).
  Dashboard JavaScript must be external files.
- **`Config.toml` additions must be `Option<T>` in the struct.** One missing required
  field fails the entire parse and silently falls back to the port-8080 defaults.
- Before `cargo test`: stop any running dev server. `tests/get_test.rs` spawns the real
  binary on the port from `Config.toml` (1140) and fails if it is occupied.

### One deliberate exception

`rmcp-actix-web` mounts a service scope. It **does not** follow the `get`/`get_alias`
pattern, because it is a protocol transport rather than a fetcher route. Add a short
comment at the mount site in `main.rs` saying so, or the next reader will treat it as a
convention slip.

---

## 4. Step 1 — Spike. This is a gate.

**Do not build anything else until this passes.** Two assumptions are unverified and both
can invalidate the design.

### Crates

```toml
rmcp = { version = "1.0", features = ["server", "macros"] }
rmcp-actix-web = "0.12"   # 0.12.16 depends on actix-web ^4 — matches this repo
```

### Do

1. Add `[mcp] enabled` to `Config.toml` and `ServerConfig`-adjacent settings (as `Option<bool>`,
   defaulting to `false`).
2. Mount `rmcp-actix-web`'s Streamable HTTP transport at `/mcp` in `main.rs`, gated on that
   flag, with `LocalSessionManager`.
3. Expose exactly one hard-coded tool, e.g. `ping` returning a fixed string.
4. Read headers `X-Authcode` and `X-Pid` in the handler and **log whether they arrived**
   (mask the authcode: first 4 + last 4 characters only).
5. Add the server to Claude as a custom connector with those two request headers set.

### Acceptance criteria

- `cargo check` and `cargo clippy --all-targets --all-features` clean.
- With `enabled = false`, `/mcp` returns 404 and startup logs are unchanged.
- With `enabled = true`, Claude lists the `ping` tool and calling it succeeds.
- **The server log shows `X-Authcode` and `X-Pid` actually arrived.**

### If headers do not arrive — STOP

[claude-ai-mcp#112](https://github.com/anthropics/claude-ai-mcp/issues/112) reports custom
headers not being configurable on remote connectors, while Anthropic's connector docs list
`static_headers` as supported. This contradiction is unresolved and is the single largest
risk in the plan.

If headers do not reach the server, **stop and report back**. The fallback is per-user
OAuth, which is a substantially larger piece of work and changes the phasing of everything
below. Do not improvise an alternative such as putting the authcode in the URL — it would
land in access logs.

### Security note carried from the crate docs

`rmcp-actix-web` forwards `Authorization` headers to MCP services, and its own docs warn
that passing those upstream (the proxy pattern) violates the MCP spec. **Use
`X-Authcode`/`X-Pid`, never `Authorization`**, for exactly this reason.

---

## 5. Step 2 — Config

`Config.toml`:

```toml
[mcp]
enabled = true
max_bytes = 2_500_000_000
ttl_secs = 21600
precache_interval_secs = 3600
admin_token = ""            # required when enabled; see step 7
```

Every field `Option<T>` in the struct, with defaults applied in code. Document each in
`CLAUDE.md`'s "Configuration files" section.

`cargo check`.

---

## 6. Step 3 — Cache module

New module. Nothing here is reachable when `[mcp] enabled = false`.

```
src/service/mcp/
  mod.rs
  cache.rs
  index.rs
  precache.rs
  tools.rs
  admin.rs
```

Add `moka = { version = "0.12", features = ["future"] }`.

### Key

```rust
#[derive(Hash, Eq, PartialEq, Clone)]
pub struct CacheKey {
    pub auth_hash: [u8; 32],   // sha256 of the authcode — never the code itself
    pub pid: i64,
    pub url: String,           // so a second Octopus instance cannot collide
}
```

The authcode must never appear in a key, a log line, a dashboard response, or an error
message. Provide one `mask_authcode(&str) -> String` helper (`FFD3…0E37`) and use it
everywhere.

### Store

```rust
Cache::builder()
    .max_capacity(cfg.max_bytes)
    .weigher(|_k, v: &Arc<CatalogSnapshot>| v.bytes.min(u32::MAX as u64) as u32)
    .time_to_live(Duration::from_secs(cfg.ttl_secs))
    .build()
```

**Eviction is LRU, not oldest-by-insertion.** This is deliberate and was chosen over the
originally requested age-ordering: with precaching on, the oldest entry is often one the
precache job just warmed and nobody has queried, so age-ordering would evict exactly what
was just paid for. Put that reasoning in a comment.

`moka` evicts asynchronously, so a burst of large inserts can briefly exceed the cap. The
configured budget must sit ~20 % below the container memory limit.

### Acceptance

Unit tests with synthetic snapshots proving: budget is enforced; a new entry that does not
fit evicts least-recently-used entries until it does; TTL expiry works; two different
authcodes produce different keys; the same authcode+pid produces one shared entry.

`cargo check`.

---

## 7. Step 4 — Snapshot / index

`src/service/mcp/index.rs` builds a `CatalogSnapshot` by calling the **existing** dispatch
in `src/service/get_data.rs` (`RequestGet::Product`, `::Price`, `::Stock`, … then
`.into_data().await`), exactly as the current routes do. Construct `CallData` the same way
`src/routes/bulk.rs` does.

Do not write new SOAP parsing. Do not touch `soap.rs`.

```rust
pub struct CatalogSnapshot {
    pub products: Vec<IndexedProduct>,
    pub by_sku: HashMap<String, u32>,
    pub folded: Vec<String>,      // accent-folded haystack, parallel to products
    pub fetched_at: DateTime<Utc>,
    pub bytes: u64,               // measured at build time, feeds the weigher
}
```

### Two sizing rules, both measured in the TypeScript prototype

- **Strip HTML from descriptions at ingest.** This cut the prototype's heap from 356 MB to
  108 MB. Nothing downstream renders HTML.
- **Drop empty fields instead of storing empty strings.** Octopus sends ~77 fields per
  product and most are blank.

Expect 40–70 MB per `(authcode, pid)` combo. Log the measured `bytes` per snapshot at
build time so the budget can be tuned against reality.

### Behaviour to port

Search ranking, accent folding (`szovegkiemelo` must match `Szövegkiemelő`), and Hungarian
decimal-comma parsing (`"0,0880000"` → `0.088`) are already solved and tested in the
`orink_web_mcp` repo — see its `src/catalog.ts`, `src/products.ts`, and the 21 tests in
`test/`. Port that logic rather than re-deriving it. One ranking rule matters and is easy
to get wrong: **the internal record id must not be searchable**, because it collides with
other products' article numbers and corrupts the ordering.

`cargo check`.

---

## 8. Step 5 — MCP tools

Four tools, shaped by user intent rather than by SOAP operation. Every tool definition
costs context on every request, so do not add one per endpoint.

| Tool | Returns |
| --- | --- |
| `search_products` | Accent-folded search over name / article number / brand / manufacturer part number, **with price and stock inline** |
| `get_product` | Full master data for one article number, plus price and stock |
| `list_categories` | Brands, main groups, product groups with counts |
| `catalog_status` | Snapshot age and product count, so Claude can state data freshness |

- `pid` comes from the `X-Pid` header, so **no tool takes a partner argument**.
- **No sync tool.** Precache handles refresh; a model-triggered 28-second sync is exactly
  what this design prevents.
- Return an MCP error result (not a protocol exception) for actionable failures — unknown
  article number, empty result set — so the model can retry differently. Include a "did you
  mean" list of near matches for a missing SKU.
- Clamp `limit` server-side and state in the response when results were truncated.
- Project down to the fields that matter. A 77-field record dumped as JSON buries the
  answer and burns the context window.

`cargo check`.

---

## 9. Step 6 — Precache

A `tokio::spawn` loop started in `main.rs` **only** when `[mcp] enabled`.

- **Refresh into a temp snapshot, then swap.** Never evict before the replacement is built,
  or a colleague asking a question mid-refresh gets a cold 28-second call.
- Use `from_date` for incremental refreshes where the operation supports it. Schedule a
  **full** pull weekly — incremental responses do not report products deleted in the ERP.
- Respect the existing `SOAP_GATE` semaphore (`soap_concurrency`, default 4) so a sweep
  cannot starve live API traffic. Consider a lower dedicated limit for precache.
- Stagger entries rather than firing all of them on the hour.
- Log start, duration, resulting size and outcome per entry, with the authcode masked.

State lives in `mcp_precache.toml` at the repo root — **gitignored**, mounted as a volume
like `soap.json`. Update `.gitignore` and `DOCKER_PLAN.md`.

`cargo check`.

---

## 10. Step 7 — Admin dashboard

Served at `/admin`, static files under `src/static/admin/`, **external JS only** (CSP is
`script-src 'self'`; `connect-src 'self'` already permits same-origin `fetch()`).

Functions: list precache entries (label, **masked** authcode, pid, last run, duration,
entry size, hit rate); add / edit / remove; refresh now; global cache usage against budget;
evict one entry.

### This turns Rustopus into a credential store — treat it accordingly

Precache needs real authcodes at rest, because the job runs with no user present. Hashing
is not an option. Therefore, non-negotiably:

- `/admin` gets its **own** authentication — `admin_token` from `Config.toml` or the
  environment. **Never** a SOAP authcode. Never unauthenticated.
- The UI displays authcodes **masked only**. The JSON API must never return a full code —
  not in a list response, not in an edit form, not in an error.
- `mcp_precache.toml` needs restricted file permissions. Flag it in `DOCKER_PLAN.md` as a
  secret-grade mount, not an ordinary config volume.
- Recommend in the docs that `/admin` be bound to an internal interface or placed behind
  the VPN rather than exposed on a public hostname.

If any of this cannot be satisfied, **stop and report back** — the fallback is dropping
precache entirely and accepting one cold call per combo per TTL, which leaves the rest of
the design intact.

`cargo check`.

---

## 11. Step 8 — Deployment

Same image, run twice:

```
api.orinkhungary.hu   → container A, [mcp] enabled = false   (unchanged, small heap)
mcp.orinkhungary.hu   → container B, [mcp] enabled = true    (2–3 GB budget)
```

A 2–3 GB cache in the process serving the live API would let an OOM take down
`/get-product` for every existing consumer. One binary, one pipeline, two instances.

Update `DOCKER_PLAN.md` with the second instance, the new mounts (`mcp_precache.toml`),
and the memory limit for container B. Set the cache budget ~20 % below that limit.

---

## 12. Documentation to update

Per `CLAUDE.md`, adding or renaming an endpoint means updating **both**:

- `src/static/docs/openapi.yaml` (Swagger picks it up automatically)
- the hand-written endpoint strip in `src/static/docs/index.html`, whose links deep-link as
  `swagger.html#/{Tag}/{method}_{path_with_underscores}`

Also update: `CLAUDE.md` (new module, new config section, the `[mcp]` gate), `README.md`,
`DOCKER_PLAN.md`, and `.gitignore`.

---

## 13. Final verification checklist

- [ ] `cargo check` clean
- [ ] `cargo clippy --all-targets --all-features` clean
- [ ] `cargo test` passes (stop any dev server on port 1140 first)
- [ ] With `[mcp] enabled = false`: `/mcp` and `/admin` return 404, no background task
      spawned, startup log unchanged, memory profile unchanged
- [ ] All eight existing endpoints return byte-identical responses to before the change
- [ ] No authcode appears in any log line, dashboard response, or error message
- [ ] Cache budget enforced under a synthetic overflow test
- [ ] `/admin` rejects requests without a valid admin token
- [ ] Claude can connect, list four tools, and answer a product question in under a second
      against a warmed cache

---

## 14. Report back on

1. Whether `X-Authcode` / `X-Pid` reached the server in step 1 (the gate).
2. Measured `bytes` per snapshot — decides whether 2 GB or 3 GB is the right budget.
3. Whether all authcodes see identical product **master data**, or only different
   *visibility* and prices. If only visibility differs, a two-tier cache (master data once
   globally, per-combo visibility lists) would cut memory from `N × 46 MB` to
   `46 MB + N × small` — a 500 MB service instead of 3 GB. Worth measuring before settling
   on the budget.
