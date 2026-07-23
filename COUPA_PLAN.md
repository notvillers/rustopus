# Plan: Coupa data connection for Rustopus

## Context

Rustopus today is a **stateless, request/response** Actix-web bridge between the **Octopus 8 ERP** (SOAP/XML, Hungarian tags) and English-facing consumers. We want it to also connect to **Coupa** (a cloud spend/procurement SaaS) over its **REST API with OAuth 2.0**, in **both directions**, covering **purchase orders, invoices, items/catalog, and suppliers**.

This is genuinely new ground for the codebase. Exploration confirmed:
- **No JSON payload machinery** — all payloads are `quick-xml`; `reqwest` is pulled *without* its `json` feature (`Cargo.toml:16`).
- **No auth/token machinery of any kind** — an exhaustive grep for `oauth|bearer|access_token|api_key|Authorization` returned zero hits. The only credential concept is the Octopus `authcode`, injected as a **SOAP body element**, never an HTTP header.
- **No background jobs, no datastore** — the server is purely request-driven.

Everything else has a clean pattern to mirror (below). Decisions locked with the user:
- Direction: **both**. Objects: **all four**. Transport/auth: **REST + OAuth 2.0**.
- Trigger: **Hybrid** — triggerable HTTP endpoints *and* an optional internal scheduler.
- State: **Stateless full/upsert sync** — natural keys (item no, PO number, invoice number); no persistent store.

**Outcome:** Rustopus can push Octopus data into Coupa and pull Coupa documents into Octopus, on demand (HTTP) or on a schedule, reusing the existing outbound-HTTP, config, routing, conversion, and error patterns.

---

## What Coupa's API requires (informs the design)

- Base: `https://{instance}.coupahost.com/api/{resource}` — resources: `purchase_orders`, `invoices`, `items`, `suppliers` (+ `requisitions`, `accounts`, …).
- **OAuth 2.0 client-credentials**: `POST https://{instance}.coupahost.com/oauth2/token` with `grant_type=client_credentials`, `client_id`, `client_secret`, space-separated `scope` (e.g. `core.purchase_order.read core.invoice.write core.item.read_write core.supplier.read_write`). Returns `{access_token, token_type, expires_in, scope}` (token lives hours) → cache and refresh before expiry, then send `Authorization: Bearer <token>`.
- JSON via `Accept: application/json` + `Content-Type: application/json`. Pagination `limit`/`offset` (max ~50), sparse `fields`, filters like `updated-at[gt]`. Writes are POST (create) / PUT (update).

---

## Reused patterns (do NOT reinvent)

| Need | Reuse | Path |
|---|---|---|
| Outbound reqwest client (pooled, timeout) | `static CLIENT` | `src/service/soap.rs:25` |
| Concurrency cap | `SOAP_GATE` semaphore pattern | `src/service/soap.rs:41` |
| Secrets file loader (gitignored) | `SoapConfig`/`SOAP_URL` (OnceLock, cwd-relative) | `src/service/soap_config.rs` |
| Route + plural alias, private `handler` | `get`/`get_alias`, `post`/`post_alias` | `src/routes/product.rs`, `src/routes/order.rs` |
| Param/error plumbing helpers | `get_auth`, `get_url`, `send_xml`, … | `src/routes/default.rs` |
| Read Octopus data (for outbound push) | `RequestGet::to_data`, `get_bulk` `futures::join!` | `src/service/get_data.rs:86`, `src/service/get/bulk.rs:71` |
| Write to Octopus (for inbound) | SOAP push via raw `get_response` + `get_request_string` | `src/routes/order.rs:113`, `src/forms/out/xml/orders.rs:9` |
| Model derive macros | add a `CoupaModel` alongside existing | `src/macros/*.rs` |
| Numeric error codes + catalog | `RustopusError`, `errors.json` | `src/global/errors.rs`, `src/errors/errors.json` |
| Field mapping | chained `impl From<in> for out` | `src/forms/out/xml/products.rs:63-160` |
| External scheduler | desktop client cron | `client/src/cron.rs`, `scheduler.rs` |

---

## New components

### 1. Config & secrets — `coupa.json` (gitignored) + `src/service/coupa_config.rs`
Mirror `soap_config.rs`: an `OnceLock<Option<CoupaConfig>>` loaded once at startup in `src/main.rs` (next to the `SOAP_URL.set(...)` block, `main.rs:56-65`). `coupa.json` shape:
```json
{
  "instance_url": "https://acme.coupahost.com",
  "client_id": "...", "client_secret": "...",
  "scopes": "core.item.read_write core.supplier.read_write core.purchase_order.read core.invoice.read_write",
  "scheduler": { "enabled": false, "interval_secs": 3600, "jobs": ["push-items"] }
}
```
Secrets stay out of git (like `soap.json`). Add `coupa.json` to `.gitignore`. Optionally add an **optional** `[coupa] concurrency` to `Config.toml`/`ServerConfig` (keep it `Option`, per the existing `soap_concurrency` rule at `config.rs:22`).

### 2. OAuth2 token manager — `src/service/coupa/auth.rs`
- `struct CachedToken { access_token: String, expires_at: Instant }`.
- `static TOKEN: Lazy<tokio::sync::Mutex<Option<CachedToken>>>` (single-flight refresh — the Mutex prevents a token stampede).
- `async fn bearer() -> Result<String, RustopusError>`: return cached token if `expires_at - now > 60s`, else `POST /oauth2/token` (form body) via `CLIENT`, parse JSON, cache with `expires_at = now + expires_in - 60s`.
- Never panics; on failure logs via `elogger` and returns a new `COUPA_AUTH_ERROR`.

### 3. Coupa HTTP layer — `src/service/coupa/http.rs`
- Reuse `CLIENT` from `soap.rs` (add a `COUPA_GATE` semaphore analogous to `SOAP_GATE`).
- `async fn coupa_get(path, &query) -> Result<String, RustopusError>` and `coupa_send(method, path, json_body)`; each attaches `bearer()`, `Accept`/`Content-Type: application/json`, maps non-2xx → `RustopusError`.
- List helper handling `limit`/`offset` pagination.
- **Reads** may use a coalescing variant later; **writes use the raw path** (same rule as `order.rs` vs `get_response_shared`, warned at `soap.rs:92`).

### 4. JSON models — `src/forms/coupa/{items,suppliers,purchase_orders,invoices}.rs`
First JSON users in the repo. Add macro `CoupaModel` in `src/macros/coupa.rs` = `#[derive(Debug, Deserialize, Serialize)]` (Coupa JSON is snake_case = serde default, so usually no `rename_all`). Model only the fields we exchange per object.
- Enable JSON ergonomics: add `"json"` to `reqwest` features in `Cargo.toml:16` (server crate). *(Alternative: hand-serialize with the already-present `serde_json` + manual headers — but adding the feature is cleaner.)*

### 5. Mapping layer — `impl From` both ways
- Outbound: `impl From<forms::out Product/Partner/Invoice> for coupa::{Item,Supplier,Invoice}`.
- Inbound: `impl From<coupa::PurchaseOrder> for forms Order/Rendeles`, then the existing SOAP push.
- ⚠️ **This is the real domain work** — field-level business rules (units, currency, GL/account coding, supplier/item identity, tax) per object. Needs a reviewed mapping table per object before coding (see Open items).

### 6. Routes — `src/routes/coupa/*.rs`, registered in `src/main.rs:97-106`
Each `handler` + singular/plural alias, using `default.rs` helpers + uuid/`log_ip` logging pattern.
- `GET /coupa/ping` — verifies OAuth + connectivity (the Phase 0 vertical slice).
- Outbound: `POST /coupa/push-items`, `/coupa/push-suppliers`, `/coupa/push-invoices` — read Octopus via `RequestGet::to_data`, map, upsert to Coupa.
- Inbound: `POST /coupa/pull-orders`, `/coupa/pull-invoices` — GET from Coupa, map, push to Octopus SOAP.
(One pattern, repeated per object.)

### 7. Hybrid scheduler — optional internal poller in `src/main.rs`
If `coupa.json.scheduler.enabled`, `tokio::spawn` an interval loop (before `HttpServer::run`) that invokes the same service functions the routes call. Must be **panic-safe** (per CLAUDE.md "must never crash"): wrap each tick in error logging, never `.unwrap()`.

### 8. Errors — `src/global/errors.rs`
Add a Coupa code block (e.g. `COUPA_AUTH_ERROR 601`, `COUPA_HTTP_ERROR 602`, `COUPA_MAP_ERROR 603`, `COUPA_CONFIG_ERROR 604`). Reuse `RustopusError`/`error_struct` propagation.

---

## Phased implementation

- **Phase 0 — Foundation:** `coupa.json` + `coupa_config.rs`; `reqwest` `json` feature; `CoupaModel` macro; error codes; `auth.rs` (token cache) + `http.rs`; `GET /coupa/ping`. **Milestone:** ping returns a live token + Coupa reachability.
- **Phase 1 — Outbound (Octopus→Coupa):** items + suppliers, then invoices. Reuse existing Octopus reads.
- **Phase 2 — Inbound (Coupa→Octopus):** purchase orders, then invoices. Reuse existing SOAP push.
- **Phase 3 — Hybrid scheduler** (config-gated) + desktop-client cron entries for the new endpoints.
- **Phase 4 — Docs & hygiene:** `openapi.yaml`, `index.html` endpoint strip, `docs/api/coupa/*` examples, `.gitignore` (`coupa.json`).

Each object follows the identical vertical: JSON model → `impl From` mapping → service fn (`http.rs` call) → route (+alias in `main.rs`). `cargo check` after every code edit (project convention); `cargo clippy --all-targets` before finishing a phase.

---

## Verification

1. **Build/lint:** `cargo check` then `cargo clippy --all-targets --all-features` — clean.
2. **Unit:** token cache expiry/refresh logic; representative `impl From` mappings (Octopus product→Coupa item, Coupa PO→Octopus order).
3. **Live smoke (needs a Coupa sandbox + `coupa.json`):** run `cargo run` from repo root, `curl localhost:1140/coupa/ping` → 200 with token OK; then a dry-run `push-items` against the sandbox and a `pull-orders` verifying one PO maps to a valid Octopus order envelope.
4. **Regression:** `cargo test` (spawns the real binary on port 1140 — stop any running dev server first) to confirm existing endpoints still pass.
5. **Secrets:** confirm `git status` never shows `coupa.json`.

---

## Open items / needs from you (before/again during coding)

1. **Coupa credentials & instance:** the `instance_url`, a client_id/secret, and which **OAuth scopes** are provisioned — plus confirmation of a **sandbox** to test against.
2. **Field mappings per object** (the bulk of the domain work): a reviewed table mapping each Coupa field ↔ Octopus field for items, suppliers, POs, invoices — including identity keys, currency, tax, and GL/account coding. I can draft first-pass tables from the existing Octopus models for you to correct.
3. **Inbound orders:** confirm the Octopus write path for Coupa POs is the existing order SOAP call (`get_request_string`/`RendelesFeladasAuth`), or whether a different Octopus endpoint is required for procurement POs vs. sales orders.
