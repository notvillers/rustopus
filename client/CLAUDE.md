# CLAUDE.md — `rustopus-client`

Guidance for the desktop GUI crate. The repo-root `CLAUDE.md` covers workspace-wide concerns (packaging scripts, cross-compile prerequisites, icon pipeline, `windows_subsystem`, client config-file *resolution*) — this file covers what's specific to working *inside* `client/src`. Read both.

## What this crate is

A native `eframe`/`egui` desktop app that exercises the Rustopus server: a **Fetch** tab for one-off requests and a **Crons** tab for cron-like scheduled requests that save responses to disk. It talks to the server over HTTP exactly like any other consumer — it does **not** import the root crate or call Octopus directly.

## Module map (`client/src`)

- `main.rs` — entry point. Builds `eframe::NativeOptions`, applies the custom rust-orange `rust_theme()`, sets the window icon (`app_icon()`), and launches `RustopusApp`. `#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]` suppresses the console in release only.
- `app.rs` — `RustopusApp` (the `eframe::App`). Owns all UI: the left **Connection** side panel, the bottom status bar, and the two central tabs (`AppTab::Fetch` / `AppTab::Crons`). Holds shared state and the mpsc receivers. This is the big file.
- `api.rs` — the `Endpoint` enum (one variant per server route), its capability predicates, `EndpointParams`, and the blocking `fetch()` that builds the query string and performs the HTTP GET. **This is the single source of truth for which endpoints exist and what parameters each accepts.**
- `config.rs` — `ClientConfig` (server/Octopus URL, authcode, xmlns, pid, `start_minimized`) plus `data_path()` / `ensure_parent_dir()` file resolution. Load/save is TOML, `unwrap_or_default()` on parse failure.
- `cron.rs` — `CronJob`, `IntervalUnit`, `CronConfig`. Scheduling math (`is_due`, `seconds_until_next_run`, `next_run_label`), filename templating (`resolved_filename`, `sync_filename_extension`), and TOML persistence.
- `scheduler.rs` — background thread that wakes every `POLL_INTERVAL_SECS` (30s), finds due jobs, and runs each in its own thread (also the `Run now` path). Writes results to disk and reports back over an mpsc channel.
- `menubar.rs` — "minimize to menu bar (macOS) / system tray (Windows)" support, with a no-op fallback elsewhere. Heavily platform-`cfg`'d and full of windowing gotchas documented inline — read the existing comments before touching it.

## Adding or changing a server endpoint (the important one)

The whole UI is **data-driven off `api.rs::Endpoint`** — the Fetch tab, the cron form, and the scheduler all iterate `Endpoint::all()` and branch on the capability predicates. To add an endpoint you normally touch **only `api.rs`**:

1. Add the enum variant.
2. Add it to `all()` (controls dropdown/tab order).
3. Add arms to the two **exhaustive** matches: `label()` (UI text + the string persisted in `crons.toml` via `from_label`) and `path()` (the `/get-…` route).
4. Set the **capability predicates** to mirror the real server route:
   - `needs_pid`, `has_from_date`, `has_to_date`, `has_type_mod`, `has_unpaid` are *positive* `matches!` lists — opt in explicitly.
   - `has_language` and `has_data_type` are *negative* lists ("everything except …") — a new endpoint gets them **by default**; add it to the exclusion only if the route lacks that param.

Get the predicates right against the actual route in `../src/routes/*.rs` — they decide which form fields render and which query params `fetch()` sends. (e.g. `Mat` has `from_date`, language, and CSV, but no `pid`.) No edits to `app.rs`/`scheduler.rs` should be needed; if you find yourself special-casing a variant there, reconsider.

## Conventions & gotchas

- **Blocking HTTP, off the UI thread.** This crate uses `reqwest::blocking` (the server uses async — don't copy patterns across). Never call `fetch()` on the egui update thread; spawn a `std::thread`, send the result over an `mpsc::Sender`, then call `ctx.request_repaint()` so the UI wakes to consume it. `fetch()` sets a 600s timeout and `danger_accept_invalid_certs(true)`.
- **Shared state is `Arc<Mutex<…>>`.** `cron_jobs` and `config_arc` are shared with scheduler threads. `.lock().unwrap()` on these mutexes is the established style here (panic-on-poison is acceptable in this GUI; the server's "no `.unwrap()`" rule does **not** apply to this crate). `config_arc` is re-synced from the editable `config` once per frame in `update()`.
- **Persistence is implicit.** `ClientConfig` saves only via the "Save settings" button, but cron jobs persist on every mutation (`save_cron_jobs`) and `poll_scheduler` re-saves each frame to capture `last_run`/`last_status` written by scheduler threads. New cron fields need `#[serde(default)]` to stay backward-compatible with existing `crons.toml`.
- **Config/cron file locations** resolve through `config::data_path` (working dir in dev, else platform config dir, with one-time legacy copy) — see root `CLAUDE.md`. Don't hardcode paths.
- **egui is pinned to 0.31** — match the existing API (`from_id_salt`, `ViewportCommand`, etc.); newer egui snippets may not compile.
- **Response viewer truncates to 100 lines** in the UI; the full body only reaches disk via "Save to file" / cron output. Keep that in mind when something "looks empty".
- After any real code edit, run `cargo check -p rustopus-client` (per the root convention).
