# Copilot Instructions for `rustopus`

## Build, test, and lint commands

This repository is a Rust binary crate (`edition = "2024"`) with a C build step in `build.rs`.

```bash
# Build
cargo build

# Run locally (reads Config.toml and soap.json from repo root)
cargo run

# Run all tests
cargo test

# Run a single test
cargo test <test_name> -- --exact

# Lint
cargo clippy --all-targets --all-features
```

Toolchain note: current dependencies (via `time 0.3.47`) require `rustc >= 1.88`.

There are currently no checked-in Rust tests (`#[test]`/`#[tokio::test]` or `tests/` integration tests), so `cargo test <test_name>` is the pattern to use when tests are added.

## High-level architecture

- `src/main.rs` wires an `actix-web` server, registers all HTTP routes, and serves static docs from `src/static/docs` at `/docs/` (root `/` redirects there).
- `src/routes/` is the HTTP layer. Each endpoint builds a `CallData` payload from query/body inputs, logs with IP + UUID, calls the service layer, then serializes XML/CSV responses.
- `src/service/` is the integration layer:
  - `soap.rs` performs outbound SOAP POST requests.
  - `get_data.rs` dispatches typed requests (`RequestGet`) to endpoint-specific fetchers in `service/get/*.rs`.
  - `service/get/*.rs` deserializes Octopus XML envelopes, converts HU/EN representations, and returns typed data enums.
- `src/forms/` holds schema/transform models:
  - `forms/in/xml/*`: incoming Octopus SOAP/request models (mostly Hungarian tag names).
  - `forms/out/xml/*` and `forms/out/csv/*`: converted English-facing output models.
- `src/service/get/bulk.rs` is an aggregator endpoint: it composes products/prices/stocks/images/barcodes calls and merges them into one response (with per-subcall fallback errors).
- Logging is hybrid Rust + C FFI:
  - `src/service/log.rs` wraps logging behavior.
  - `src/C/*.c` provides append/date helpers compiled by `build.rs`.
  - logs are written to `log/YYYY.MM.DD.log`.

## Key repository conventions

- Route handlers should reuse `src/routes/default.rs` helpers (`get_auth`, `get_url`, `get_xmlns`, `get_pid`, `get_date`, `get_i64`, `send_xml`, `send_csv`) instead of reimplementing parameter/error plumbing.
- Error responses use numeric `RustopusError` codes from `src/global/errors.rs`, and endpoint-specific XML error constructors (`error_struct_xml`) from `forms/out/xml/*`.
- Request identity/logging pattern is consistent across routes:
  1. `let uuid = get_uuid();`
  2. `let ip_address = log_ip(req).await...;`
  3. log before external call
  4. log after external call
- `RequestParameters` accepts both `authcode` and `auth`; `url` can fall back to `soap.json`; `xmlns` is derived from URL (`.../services/`) when omitted.
- Language/format behavior is centralized in `CallData`:
  - `language=hu|hun|hungary|hungarian` keeps Hungarian XML.
  - otherwise responses are translated to English models.
  - `data_type=csv` switches matching endpoints to semicolon-delimited CSV output.
- Keep envelope conversion style consistent: prefer `impl From<...>` mapping between `forms::in` and `forms::out` models instead of ad-hoc field transforms in route handlers.
