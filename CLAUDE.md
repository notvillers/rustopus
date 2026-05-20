# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Rustopus is a web service that bridges the **Octopus 8 ERP** and clients. Octopus 8 exposes SOAP/XML with Hungarian tag names; Rustopus fetches, deserializes, and translates the payloads into English-tagged XML (or CSV) so non-Hungarian consumers can use them. It can also accept English-tagged input and forward it as Hungarian to Octopus.

## Workspace layout

This is a Cargo workspace with two crates:

- Root crate `rustopus` (binary) — the Actix-web HTTP service (`src/`). Edition 2024, requires `rustc >= 1.88` (transitively via `time 0.3.47`). Has a `build.rs` that compiles C helpers from `src/C/*.c` via the `cc` crate.
- `client/` crate `rustopus-client` — a desktop GUI (`eframe`/`egui`) used to exercise the server and manage cron-like scheduled requests (`client/src/cron.rs`, `scheduler.rs`).

## Build, run, test, lint

```bash
# Server
cargo build
cargo run                           # reads Config.toml + soap.json from repo root
cargo clippy --all-targets --all-features

# Tests (none checked in yet — use this pattern when adding)
cargo test
cargo test <test_name> -- --exact

# Desktop client
cargo run -p rustopus-client        # also: ./client.sh
```

`start.sh` / `start` run the prebuilt `./rustopus` binary; they are deploy scripts, not dev scripts.

There are currently no `#[test]` / `#[tokio::test]` or `tests/` integration tests in the repo.

## High-level architecture

- `src/main.rs` wires the `actix-web` server, registers all HTTP routes, and serves Swagger docs from `src/static/docs` at `/docs/` (root `/` redirects there). Installs a panic hook that writes to the error log instead of aborting.
- `src/routes/` is the HTTP layer. Each endpoint builds a `CallData` payload from query/body inputs, logs with IP + UUID, calls the service layer, then serializes XML/CSV responses.
- `src/service/` is the integration layer:
  - `soap.rs` performs outbound SOAP POST requests.
  - `get_data.rs` dispatches typed requests (`RequestGet`) to endpoint-specific fetchers in `service/get/*.rs`.
  - `service/get/*.rs` deserializes Octopus XML envelopes, converts HU/EN representations, and returns typed data enums.
  - `service/get/bulk.rs` is an aggregator: it composes products/prices/stocks/images/barcodes calls and merges them into one response with per-subcall fallback errors.
- `src/forms/` holds schema/transform models:
  - `forms/in/xml/*`: incoming Octopus SOAP/request models (Hungarian tag names).
  - `forms/out/xml/*` and `forms/out/csv/*`: converted English-facing output models.
- Logging is hybrid Rust + C FFI:
  - `src/service/log.rs` wraps logging behavior.
  - `src/C/*.c` provides append/date helpers compiled by `build.rs`.
  - Logs are written to `log/YYYY.MM.DD.log`.

## Repository conventions

- **Reuse route helpers.** Route handlers should use `src/routes/default.rs` helpers (`get_auth`, `get_url`, `get_xmlns`, `get_pid`, `get_date`, `get_i64`, `send_xml`, `send_csv`) instead of reimplementing parameter/error plumbing.
- **Errors.** Numeric `RustopusError` codes live in `src/global/errors.rs`; endpoint-specific XML error constructors (`error_struct_xml`) live in `forms/out/xml/*`. The catalog of human-readable messages is in `errors.json` (loaded by the error service).
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

## Configuration files (gitignored at runtime — see `.gitignore`)

- `Config.toml` — server bind config: `[server] host, port, timeout, workers`. Defaults: `0.0.0.0`, `8080`, `1200`s, `available_parallelism()`.
- `soap.json` — `{ "url": "<default wsdl url>" }`. Used as fallback for `url`/`xmlns` when a request doesn't supply them.
- `client_config.toml`, `crons.toml` — desktop-client state; not used by the server.

`*.xml`, `*.log`, `*.csv`, `example/`, and `test/` are gitignored — treat the `example/` and `test/` XML files as scratch fixtures, not source of truth.
