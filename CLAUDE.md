# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustopus is a web service that bridges the **Octopus 8 ERP** and clients. Octopus 8 exposes SOAP/XML with Hungarian tag names; Rustopus fetches, deserializes, and translates the payloads into English-tagged XML (or CSV) so non-Hungarian consumers can use them. It can also accept English-tagged input and forward it as Hungarian to Octopus.

## Workspace layout

This is a Cargo workspace with two crates:

- Root crate `rustopus` (binary) — the Actix-web HTTP service (`src/`). Edition 2024, requires `rustc >= 1.88` (transitively via `time 0.3.47`). Has a `build.rs` that compiles C helpers from `src/C/*.c` via the `cc` crate.
- `client/` crate `rustopus-client` — a desktop GUI (`eframe`/`egui`) used to exercise the server and manage cron-like scheduled requests (`client/src/cron.rs`, `scheduler.rs`). Its `client/build.rs` embeds the app icon (`winresource`) into Windows builds.

## Build, run, test, lint

```bash
# Server
cargo build
cargo run                           # reads Config.toml + soap.json from repo root
cargo clippy --all-targets --all-features

# Tests
cargo test                          # runs tests/get_test.rs (integration)
cargo test get_test_returns_envelope -- --exact

# Desktop client (native, dev)
cargo run -p rustopus-client        # also: ./client.sh / client.bat
```

`tests/get_test.rs` builds and **spawns the real server binary** on the port from `Config.toml` (fallback 1140) and polls `/get-test` — it fails if something else is already bound to that port, so stop a running dev server first.

`start.sh` / `start` run the prebuilt `./rustopus` binary; they are deploy scripts, not dev scripts.

## Client packaging (release builds)

```bash
./build_client_win.sh   # Windows x64 exe → target/x86_64-pc-windows-gnu/release/rustopus-client.exe
./build_client_mac.sh   # macOS bundle  → target/release/Rustopus Client.app
./zip_mac_app.sh        # zips the .app for distribution (ditto)
```

- **Windows cross-compile prerequisites** (one-time): `brew install mingw-w64` and `rustup target add x86_64-pc-windows-gnu`. The linker is configured in the checked-in `.cargo/config.toml`. The GNU-target exe is self-contained (rustls, no extra DLLs).
- **No console/terminal windows**: the Windows exe uses `windows_subsystem = "windows"` (release builds only — debug builds keep the console for `println!`); on macOS the windowless launch comes from packaging as a `.app` bundle, so distribute the bundle, not the bare binary.
- **Icons** all derive from `client/src/assets/images/octopus.png` (64×64): `client/build.rs` embeds `octopus.ico` into the exe, `build_client_mac.sh` generates the `.icns` via `sips`/`iconutil`, and `main.rs::app_icon()` sets the runtime window icon.
- **Client config resolution** (`client/src/config.rs::data_path`): `client_config.toml` / `crons.toml` are read from the working directory if present (dev runs from repo root), otherwise from the platform config directory — `~/Library/Application Support/Rustopus Client` (macOS), `%APPDATA%\Rustopus Client` (Windows), `~/.config/rustopus-client` (Linux) — so settings survive app updates. Legacy files next to the executable (the pre-config-dir location) are copied into the config directory on first lookup.

## High-level architecture

- `src/main.rs` wires the `actix-web` server, registers all HTTP routes, and serves the static docs tree `src/static/docs` at `/docs/`. Root `/` (`src/routes/index.rs`) serves the docs landing page (`index.html`) directly via `NamedFile`, falling back to a `/docs/` redirect if the file can't be opened — one more reason the server must run from the repo root. Installs a panic hook that writes to the error log instead of aborting. Note: the server's request/keep-alive timeouts are hardcoded to 1200s here, *not* read from `Config.toml`'s `timeout`.
- `src/routes/` is the HTTP layer. Each endpoint builds a `CallData` payload from query/body inputs, logs with IP + UUID, calls the service layer, then serializes XML/CSV responses. Each fetcher route file exposes a **canonical singular path plus a plural alias** that share one `handler` — e.g. `product.rs` registers `#[get("/get-product")]` (`get`) and `#[get("/get-products")]` (`get_alias`). Both variants are wired in `main.rs`. The GET fetchers are `get-product`, `get-stock`, `get-price`, `get-image`, `get-barcode`, `get-bulk`, `get-invoice`, `get-mat` (the "mathematican models" endpoint) — each with its plural alias — plus `get-test` (no alias) and the index `/`. The single POST endpoint is `/post-order` (alias `/post-orders`).
- `src/service/` is the integration layer:
  - `soap.rs` performs outbound SOAP POST requests through one process-wide, lazily-built `reqwest::Client` (`static CLIENT: Lazy<Client>`) so the connection pool / TLS sessions are reused across calls — never build a client per request. The server uses async reqwest only; the `blocking` feature/API belongs to the desktop `client` crate, not the server (calling it from the async runtime would panic). Two protections live here: `SOAP_GATE` (a `tokio::sync::Semaphore`, size from `Config.toml`'s optional `soap_concurrency`, default 4) caps concurrent outbound calls, and `get_response_shared` coalesces identical concurrent requests (singleflight keyed on `(url, soap_body)`, returns `Arc<String>`). The 7 GET fetchers use `get_response_shared`; **`/post-order` must keep the raw `get_response`** — coalescing a mutating call would merge two intentional submissions.
  - `get_data.rs` dispatches typed requests (`RequestGet`) to endpoint-specific fetchers in `service/get/*.rs`.
  - `service/get/*.rs` deserializes Octopus XML envelopes, converts HU/EN representations, and returns typed data enums.
  - `service/get/bulk.rs` is an aggregator: it composes products/prices/stocks/images/barcodes calls and merges them into one response with per-subcall fallback errors.
- `src/service/mcp/` is the **MCP endpoint**, and is entirely gated behind `[mcp] enabled` (default `false`). With the flag off, `main.rs` registers no route, spawns no task and never constructs the cache, so the nine REST endpoints keep their original behaviour and memory profile.
  - `mod.rs` — the `X-Authcode`/`X-Pid` header names, the `McpAuth` request identity, and `mask_authcode` (`FFD3…0E37`). **Every** path that identifies a caller in a log, a key, a dashboard row or an error goes through that helper; a full authcode must never appear in any of them.
  - `cache.rs` — `moka` cache of `Arc<CatalogSnapshot>` keyed on `sha256(authcode) + pid + url`, weighed by measured snapshot bytes, budget from `[mcp] max_bytes`, explicit **LRU** eviction. LRU is chosen over moka's default TinyLFU *and* over age-ordering because both would discard a snapshot the precache job just spent minutes warming. Lookups go **memory → disk → Octopus**, each tier an order of magnitude dearer than the last; `insert` writes through to disk, `promote` puts a disk-loaded snapshot into memory *without* rewriting the file it came from, and `invalidate` clears both tiers. **`max_bytes = 0` turns the memory tier off** (`memory_enabled()`), serving every query from disk — measured at ~90 ms and ~12 MB idle against ~103 MB holding one snapshot, which is the shipped configuration.
  - `store.rs` — the disk tier: snapshots as gzipped JSON under `[mcp] disk_path`. Exists because the target host has ~1–1.5 GB of RAM and one snapshot is ~46 MB, so memory holds only a handful. A measured reload is ~5.6 MB on disk, 0.2s to decompress and parse, against ~260s to rebuild from Octopus. Only `products` and `fetched_at` are stored — `folded`, `by_sku` and `bytes` are rebuilt by `assemble()` on load, and `fetched_at` is restored verbatim so a reloaded snapshot keeps its real age. **Reads and writes go through `actix_web::web::block`**: serializing tens of megabytes is CPU-bound and would otherwise park an async worker that also serves the REST endpoints.
  - `index.rs` — builds a `CatalogSnapshot` by calling the **existing** `RequestGet` dispatch in `get_data.rs`; it writes no new SOAP parsing and does not touch `soap.rs`. Holds the accent folding, the ported search ranking, HTML stripping and the incremental-merge logic. **The internal record id (`cikkid`) must never enter a search haystack** — its values collide with other products' article numbers and corrupt the ranking.
  - `tools.rs` — the five tools (`search_products`, `get_product`, `list_categories`, `catalog_status`, `export_products`) plus the `rmcp-actix-web` transport. No tool takes a partner argument (the pid comes from the header) and there is deliberately **no sync tool**. Numeric arguments go through `lenient_count`, which accepts `20` *and* `"20"`: the published schema stays `integer`, but bridges such as `mcp-remote` lose type information in transit and an `invalid type: string` error reads like a server bug and blocks the call.
  - `export.rs` — bulk export to `.xlsx`/`.csv` plus the `/export/{token}` download route. Exists because **paging is not a bulk mechanism**: 24,000 rows cannot cross a model's context at any page size, so `search_products` pages for browsing only and anything larger goes through a file. Rows are selected from the cached snapshot behind an `Arc` and written inside `web::block` — copying them out first would put a second catalog in memory. The download link carries an unguessable token, never an authcode (which would land in access logs); files are `0600` in a `0700` directory, expire with their token, and `purge_orphans()` clears leftovers at startup because tokens live in memory and a restart makes old files unreachable but still readable on disk.
  - `precache.rs` — the background refresh loop and `mcp_precache.toml`.
  - `admin.rs` — the `/admin` dashboard API, behind its own `admin_token`.
- **The MCP cache is MCP-only.** Do not add caching to `src/service/soap.rs`: that would silently change all nine existing endpoints, whose consumers expect a live read on every call. `get_response_shared` coalescing *concurrent* identical calls is a different thing and stays as it is.
- `src/forms/` holds schema/transform models:
  - `forms/in/xml/*`: incoming Octopus SOAP/request models (Hungarian tag names).
  - `forms/out/xml/*` and `forms/out/csv/*`: converted English-facing output models.
- `src/macros/` defines the `macro_rules!` wrappers that stamp the common `serde` derives onto every model, so the form files declare data shapes without repeating `#[derive(...)]`. Each is `pub(crate) use`-exported from its module:
  - `macros/in.rs`: `O8ModelDeriveOnly` (Debug + De/Serialize), `O8ModelLowercase`, `O8ModelPascalcase` (add `#[serde(rename_all = ...)]`) — for incoming Octopus models.
  - `macros/out.rs`: `OutModelDeriveOnly` (Debug + Serialize), `OutModelDeriveSerializeOnly` (Serialize only) — for English output models.
  - `macros/get.rs`: `get_models` — `#[serde(untagged)]` Serialize enums (the `ResponseGet`/`*Data` response dispatch enums).
  - `macros/service.rs`: `ConfigModelDerive` (Deserialize) — config/settings structs.
  - `macros/mcp.rs`: `McpToolArgs` (Debug + Deserialize + `JsonSchema`) — MCP tool-argument
    models. MCP publishes a JSON Schema per tool, so these need a derive the other layers don't;
    `schemars` is reached through rmcp's re-export, hence the `#[schemars(crate = ...)]` inside
    the macro.
- Two separate doc trees exist: `src/static/docs/` is the served docs bundle; `docs/api/<endpoint>/` holds hand-written consumer request examples (`request.{sh,py,js,cs,ps1}` + README) — keep these in sync when an endpoint's parameters change.
- The served docs bundle (`src/static/docs/`) has two pages:
  - `index.html` — static landing page (Hermes-style design), served at both `/` and `/docs/`, so all its asset/link paths are absolute (`/docs/...`). Styled by `landing.css` (never touch `index.css` / `swagger-ui.css` for landing work — those are the stock Swagger UI dist files). `landing.js` fills the `CALL VIA TERMINAL` example's domain from `landing-config.js`; scripts must be external files because the CSP in `main.rs::security_headers()` is `script-src 'self'` (no inline scripts).
  - `swagger.html` — Swagger UI (stock look) rendering `openapi.yaml`, served at `/docs/swagger.html`; it reuses `landing.css` only for the blue hero band on top.
  - **Adding/renaming an endpoint**: update `openapi.yaml` (Swagger picks it up automatically) *and* the hand-written endpoint strip on `index.html`, whose links deep-link into operations as `swagger.html#/{Tag}/{method}_{path_with_underscores}` (e.g. `#/Products/get_get_product`). Note the strip is currently HTML-commented out; the visible landing content is the numbered `.feature` cards, so a new endpoint worth advertising needs a card too. `/mcp` and `/admin` are in `openapi.yaml` under the `MCP` tag, each flagged in its description as a non-REST entry documented for completeness.
- `src/static/admin/` is the `/admin` dashboard's own `index.html` / `admin.css` / `admin.js`, served by handlers in `service/mcp/admin.rs` (not by `Files`, so the token check covers the page as well as the API). **External JS only** — the CSP is `script-src 'self'`; `connect-src 'self'` already permits the same-origin `fetch()` calls. Build rows with `textContent`, never `innerHTML`: entry labels are operator input.
- Logging is hybrid Rust + C FFI:
  - `src/service/log.rs` wraps logging behavior.
  - `src/C/*.c` provides append/date helpers compiled by `build.rs`.
  - Logs are written to `log/YYYY.MM.DD.log`.

## Repository conventions

- **Reuse route helpers.** Route handlers should use `src/routes/default.rs` helpers (`get_auth`, `get_url`, `get_xmlns`, `get_pid`, `get_date`, `get_i64`, `send_xml`, `send_csv`) instead of reimplementing parameter/error plumbing.
- **Route + alias pairing.** Each fetcher file keeps the real logic in a private `handler` and exposes two thin wrappers — `get` (canonical singular path) and `get_alias` (plural path); `order.rs` uses `post`/`post_alias`. When adding or renaming a route, register **both** wrappers in `main.rs` and keep each `_alias` pointing at its own module (a copy-paste slip that registers the wrong module's alias silently drops one path).
  - **Three deliberate exceptions**, all mounted as scopes rather than route pairs, and all commented as such at the mount site: `/mcp` (a protocol transport — POST/GET/DELETE on one path, session-managed), `/admin` (a small internal app with its own authentication) and `/export/{token}` (a token-authenticated file download with a path parameter). None is a fetcher, so none has a plural alias.
- **Errors.** Numeric `RustopusError` codes live in `src/global/errors.rs`; endpoint-specific XML error constructors (`error_struct_xml`) live in `forms/out/xml/*`. The catalog of human-readable messages is `src/errors/errors.json`, loaded at runtime relative to the working directory — another reason the server must run from the repo root.
- **No `.unwrap()` on real paths.** This service must never crash — prefer `match` / `if let` / `?` and surface a `RustopusError`. The panic hook in `main.rs` is a safety net, not the policy.
- **Request identity / logging pattern** (consistent across routes):
  1. `let uuid = get_uuid();`
  2. `let ip_address = log_ip(req).await...;`
  3. log before the external call
  4. log after the external call
- **`RequestParameters` flexibility.** Accepts both `authcode` and `auth`; `url` falls back to `soap.json`; `xmlns` is derived from the URL (`.../services/`) when omitted.
- **Language / format toggles** are centralized in `CallData`:
  - `language=hu|hun|hungary|hungarian` → keep Hungarian XML.
  - otherwise → translate to English models.
  - `data_type=csv` → semicolon-delimited CSV output on endpoints that support it.
- **Conversion style.** Prefer `impl From<...>` mappings between `forms::in` and `forms::out` models over ad-hoc field transforms in route handlers.
- **Don't clone `CallData` to read it.** `is_hu()`/`is_csv()` take `&self`, and fields (`language`, `data_type`, …) can be read by reference — call them on the borrow instead of `call_data.clone().is_hu()`. Clone only when you need an owned copy, e.g. the concurrent `futures::join!` fan-out in `service/get/bulk.rs`.
- **Declare models through the `src/macros/` wrappers, not bare `#[derive]`.** New form/response models should reuse the existing macro for that layer (see "High-level architecture") so the derive set stays uniform. Two call styles are in use: the function-like form wrapping a block of definitions (`OutModelDeriveSerializeOnly! { pub struct A {..} pub struct B {..} }`), and the attribute form on a single item via `macro_rules_attribute::apply` (`#[apply(O8ModelLowercase)] pub struct C {..}`). `impl` blocks and `error_struct(_xml)` constructors stay outside the macro block.

## Configuration files

- `Config.toml` (checked in) — server bind config in `[server]`, MCP config in the optional `[mcp]` table.
  - `[server] host, port, timeout, workers, soap_concurrency` (the last is optional — it must stay `Option` in `ServerConfig`, since a missing required field fails the whole parse and silently falls back to the port-8080 defaults). The committed config uses port `1140` (the code defaults to `8080` when absent); the desktop client and the integration test both assume `1140`. `get_settings()` parses this file once into a cached `static SETTINGS: Lazy<Settings>`, so edits take effect only on restart; `timeout` is applied to the outbound `reqwest` client (the actix request/keep-alive timeouts are the separate hardcoded 1200s noted under High-level architecture).
  - `[mcp] enabled, max_bytes, disk_path, disk_max_bytes, export_path, export_ttl_secs, public_url, ttl_secs, precache_interval_secs, admin_token` — **every key is `Option<T>`** in `McpConfig` and every default lives in code (`is_enabled()`, `max_bytes()`, …), so a partial or absent table never fails the parse. `enabled` defaults to `false` and is `false` in the committed file: only the dedicated MCP instance turns it on (see `DOCKER_PLAN.md`). `admin_token()` reads `RUSTOPUS_ADMIN_TOKEN` **first** and falls back to the file — this file is tracked in git, so a token written here gets committed; with neither set, `/admin` is not registered at all.
- `.env` / `.ENV` (gitignored, **secret-grade**) — optional environment file in the working directory, loaded via `dotenv` as the first line of `main()` in `src/main.rs` (`.env` tried first, `.ENV` as a fallback for this deploy's history) before anything reads `std::env`. This is where `RUSTOPUS_ADMIN_TOKEN` should live instead of `Config.toml`, so it never gets committed. Only takes effect on the binary that's actually running — copying a new build to `./rustopus` does not affect an already-running process (`ETXTBSY` on in-place overwrite; deploy via copy-to-temp-then-rename) and a **service restart is required** to load it.
- `soap.json` (gitignored) — `{ "url": "<default wsdl url>" }`. Used as fallback for `url`/`xmlns` when a request doesn't supply them. Also the url the MCP snapshot builder uses, since MCP callers pass no `url`.
- `mcp_cache/` (gitignored, **secret-grade**) — the disk tier's snapshot files, one gzipped JSON per `(authcode, pid)`, written `0600` inside a `0700` directory. Named by the authcode's hash fingerprint plus the pid, never by the code, so a directory listing reveals neither credentials nor who an entry belongs to. The contents are a partner's **own negotiated prices** and stock, so treat the volume as sensitive, not scratch. Path comes from `[mcp] disk_path`.
- `mcp_exports/` (gitignored, **secret-grade**) — generated `.xlsx`/`.csv` exports awaiting download, named by their download token and written `0600` in a `0700` directory. Deleted when the token expires (`export_ttl_secs`) and wiped wholesale at startup. Holds the same partner prices as the snapshots, in a form that is trivially readable. Path comes from `[mcp] export_path`. **`public_url` must be set in any real deployment** — download links are built from it, and the MCP transport gives a tool no view of the request's `Host` header to derive it from.
- `mcp_precache.toml` (gitignored, **secret-grade**) — which `(authcode, pid)` combinations the precache job keeps warm. Holds **live authcodes in plain text**, because the job runs with no user present; written `0600` via a temp-file-and-rename, managed by `/admin`, and mounted as a secret rather than baked into the image. It holds configuration only — last-run bookkeeping stays in memory so a credential file is not rewritten hourly, which means the first sweep after a restart does a full pull.
- `client_config.toml`, `crons.toml` (gitignored) — desktop-client state; not used by the server. See "Client config resolution" above for where the client looks for them.
- `src/static/docs/landing-config.js` (checked in) — per-deployment docs setting: `RUSTOPUS_API_BASE`, the base URL shown in the landing page's terminal example (empty string → falls back to `window.location.origin`).

`*.xml`, `*.log`, `*.csv`, `example/`, and `test/` are gitignored — treat the `example/` and `test/` XML files as scratch fixtures, not source of truth. (`tests/` — with an "s" — is the real integration-test directory and is tracked.)

## Important

At the end of every true code editing (so not markdowns, configs) should be followed with a `cargo check` to see if it compiles.
