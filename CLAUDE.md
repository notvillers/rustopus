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

- `src/main.rs` wires the `actix-web` server, registers all HTTP routes, and serves Swagger docs from `src/static/docs` at `/docs/` (root `/` redirects there). Installs a panic hook that writes to the error log instead of aborting. Note: the server's request/keep-alive timeouts are hardcoded to 1200s here, *not* read from `Config.toml`'s `timeout`.
- `src/routes/` is the HTTP layer. Each endpoint builds a `CallData` payload from query/body inputs, logs with IP + UUID, calls the service layer, then serializes XML/CSV responses. Each fetcher route file exposes a **canonical singular path plus a plural alias** that share one `handler` — e.g. `product.rs` registers `#[get("/get-product")]` (`get`) and `#[get("/get-products")]` (`get_alias`). Both variants are wired in `main.rs`. The GET fetchers are `get-product`, `get-stock`, `get-price`, `get-image`, `get-barcode`, `get-bulk`, `get-invoice`, `get-mat` (the "mathematican models" endpoint) — each with its plural alias — plus `get-test` (no alias) and the index `/`. The single POST endpoint is `/post-order` (alias `/post-orders`).
- `src/service/` is the integration layer:
  - `soap.rs` performs outbound SOAP POST requests through one process-wide, lazily-built `reqwest::Client` (`static CLIENT: Lazy<Client>`) so the connection pool / TLS sessions are reused across calls — never build a client per request. The server uses async reqwest only; the `blocking` feature/API belongs to the desktop `client` crate, not the server (calling it from the async runtime would panic).
  - `get_data.rs` dispatches typed requests (`RequestGet`) to endpoint-specific fetchers in `service/get/*.rs`.
  - `service/get/*.rs` deserializes Octopus XML envelopes, converts HU/EN representations, and returns typed data enums.
  - `service/get/bulk.rs` is an aggregator: it composes products/prices/stocks/images/barcodes calls and merges them into one response with per-subcall fallback errors.
- `src/forms/` holds schema/transform models:
  - `forms/in/xml/*`: incoming Octopus SOAP/request models (Hungarian tag names).
  - `forms/out/xml/*` and `forms/out/csv/*`: converted English-facing output models.
- `src/macros/` defines the `macro_rules!` wrappers that stamp the common `serde` derives onto every model, so the form files declare data shapes without repeating `#[derive(...)]`. Each is `pub(crate) use`-exported from its module:
  - `macros/in.rs`: `O8ModelDeriveOnly` (Debug + De/Serialize), `O8ModelLowercase`, `O8ModelPascalcase` (add `#[serde(rename_all = ...)]`) — for incoming Octopus models.
  - `macros/out.rs`: `OutModelDeriveOnly` (Debug + Serialize), `OutModelDeriveSerializeOnly` (Serialize only) — for English output models.
  - `macros/get.rs`: `get_models` — `#[serde(untagged)]` Serialize enums (the `ResponseGet`/`*Data` response dispatch enums).
  - `macros/service.rs`: `ConfigModelDerive` (Deserialize) — config/settings structs.
- Two separate doc trees exist: `src/static/docs/` is the served Swagger UI bundle (with `openapi.yaml`); `docs/api/<endpoint>/` holds hand-written consumer request examples (`request.{sh,py,js,cs,ps1}` + README) — keep these in sync when an endpoint's parameters change.
- Logging is hybrid Rust + C FFI:
  - `src/service/log.rs` wraps logging behavior.
  - `src/C/*.c` provides append/date helpers compiled by `build.rs`.
  - Logs are written to `log/YYYY.MM.DD.log`.

## Repository conventions

- **Reuse route helpers.** Route handlers should use `src/routes/default.rs` helpers (`get_auth`, `get_url`, `get_xmlns`, `get_pid`, `get_date`, `get_i64`, `send_xml`, `send_csv`) instead of reimplementing parameter/error plumbing.
- **Route + alias pairing.** Each fetcher file keeps the real logic in a private `handler` and exposes two thin wrappers — `get` (canonical singular path) and `get_alias` (plural path); `order.rs` uses `post`/`post_alias`. When adding or renaming a route, register **both** wrappers in `main.rs` and keep each `_alias` pointing at its own module (a copy-paste slip that registers the wrong module's alias silently drops one path).
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

- `Config.toml` (checked in) — server bind config: `[server] host, port, timeout, workers`. The committed config uses port `1140` (the code defaults to `8080` when absent); the desktop client and the integration test both assume `1140`. `get_settings()` parses this file once into a cached `static SETTINGS: Lazy<Settings>`, so edits take effect only on restart; `timeout` is applied to the outbound `reqwest` client (the actix request/keep-alive timeouts are the separate hardcoded 1200s noted under High-level architecture).
- `soap.json` (gitignored) — `{ "url": "<default wsdl url>" }`. Used as fallback for `url`/`xmlns` when a request doesn't supply them.
- `client_config.toml`, `crons.toml` (gitignored) — desktop-client state; not used by the server. See "Client config resolution" above for where the client looks for them.

`*.xml`, `*.log`, `*.csv`, `example/`, and `test/` are gitignored — treat the `example/` and `test/` XML files as scratch fixtures, not source of truth. (`tests/` — with an "s" — is the real integration-test directory and is tracked.)

## Important

At the end of every true code editing (so not markdowns, configs) should be followed with a `cargo check` to see if it compiles.
