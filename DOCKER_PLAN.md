# Containerize Rustopus (Docker/OCI)

## Context

The user wants to run the Rustopus server "as a microVM or smth." We settled on a
**Docker/OCI container** (runs anywhere with Docker/Podman/K8s; the usual practical
"microVM"), with **runtime config supplied via mounted volumes** so the SOAP url / port
can change without rebuilding and logs persist outside the image.

The one hard constraint discovered during exploration: the binary resolves **everything
relative to the current working directory** via `env::current_dir()`:

- `Config.toml` — loaded by name through the `config` crate (CWD search) — `src/service/config.rs:26`
- `soap.json` — `current_dir()/soap.json` — `src/service/soap_config.rs:13` + `src/service/path.rs:22`
- `errors.json` — `current_dir()/src/errors/errors.json` — `src/service/errors.rs:23`
- Swagger docs — `current_dir()/src/static/docs` — `src/main.rs:37`
- Logs — `current_dir()/log/` (auto-created) — `src/service/log.rs:113`

So the image must lay out exactly that tree under a fixed `WORKDIR`, and that dir must be CWD at runtime.

Favorable facts: uses `rustls-tls` (no OpenSSL runtime dep), and the two C helpers
(`append.c`, `date_prefix.c`) are statically linked at build time via the `cc` crate +
`build.rs` — no C runtime dependency in the final binary beyond libc. This makes a small
static-musl image on Alpine clean.

## Image layout (inside container, `WORKDIR /app`)

Baked into the image (these are source assets, not config):
```
/app/rustopus                       # the release binary
/app/src/errors/errors.json
/app/src/static/docs/...            # served at /docs/
/app/src/static/admin/...           # served at /admin (MCP instance only)
```
Mounted at runtime (volumes):
```
/app/Config.toml         (ro)
/app/soap.json           (ro)
/app/log/                (rw, persisted)
/app/mcp_precache.toml   (rw, SECRET — MCP instance only, see below)
/app/mcp_cache/          (rw, SECRET, persisted — MCP instance only)
```

`mcp_cache/` is the snapshot store's disk tier. Persist it: without it, every
restart costs a full multi-minute rebuild per configured combination, which is
exactly what this design exists to avoid. Size it from `[mcp] disk_max_bytes`
(default 5 GB); each stored snapshot is ~5.6 MB gzipped. It must be writable by
uid 10001, and the service creates it `0700` with `0600` files.

It is **secret-grade too**: the files contain a partner's own negotiated prices
and stock levels. Not credentials, but not scratch data either — provision it
like the file below, not like a cache volume you would happily expose.

`mcp_precache.toml` is **not an ordinary config volume**. Every entry in it holds
a live Octopus authcode in plain text, because the precache job runs with no user
present to supply one. Provision it as a secret: `0600`, owned by the container's
uid (10001), never baked into the image, never in the build context. It is
mounted read-write because `/admin` rewrites it when an entry is added or edited
— the file is written to a temporary path and renamed, so a crash mid-write
cannot leave a half-written credential file behind.

## Files to create (repo root)

### 1. `Dockerfile` — multi-stage, static musl on Alpine

Builder stage:
- Base `rust:1.88-alpine` (edition 2024 needs rustc ≥ 1.88 per CLAUDE.md; bump if newer is pinned).
- `apk add --no-cache musl-dev build-base` so the `cc` crate can compile the C helpers against musl.
- Copy the repo (context trimmed by `.dockerignore`) and build **only the server**:
  `cargo build --release -p rustopus`
  (`-p rustopus` avoids compiling the heavy `client/` eframe/egui GUI; the `client/` manifest
  must still be present for workspace resolution, so the whole repo is copied — it just isn't built.)
- Optional caching: pre-copy `Cargo.toml`/`Cargo.lock` + a stub build to cache the dependency layer
  before copying `src/`. Mark as a nice-to-have, not required for correctness.

Runtime stage:
- Base `alpine:3` (small, keeps a shell for debugging; `scratch`/`distroless-static` also work
  since the binary is static — note this as an alternative).
- Create a non-root user (fixed uid, e.g. `10001`) and `mkdir -p /app/log` owned by it so the
  mounted log volume is writable.
- `WORKDIR /app`; copy from builder: the binary, `src/errors/errors.json`, `src/static/docs/`.
- `EXPOSE 1140` (documentational; actual port comes from the mounted `Config.toml`).
- `USER 10001`; `ENTRYPOINT ["./rustopus"]`.

### 2. `.dockerignore`

Exclude from build context: `target/`, `client/target/` if any, `example/`, `test/`, `ping/`,
`*.log`, `*.csv`, `*.xml`, `.git/`, `.github/`, `.claude/`, `.vscode/`, and the runtime config
`Config.toml` / `soap.json` / **`mcp_precache.toml`** (those are mounted, not baked — and the
last one is a credential file that must never enter a build context or an image layer).

### 3. (Optional) `compose.yaml` — convenience run

Single `rustopus` service: builds the Dockerfile, maps host port → container `1140`, and mounts
`./Config.toml`, `./soap.json` read-only plus `./log` read-write. Lets the user `docker compose up`
without remembering the `-v` flags. Include only if the user wants it.

## Two instances from one image

The MCP endpoint holds a multi-gigabyte catalog cache. Running it inside the
process that serves the live API would let one OOM take `/get-product` down for
every existing consumer, so the same image is run **twice** with different
mounted `Config.toml` files:

| Host | Instance | `[mcp] enabled` | Memory limit | RAM budget | Disk budget |
| --- | --- | --- | --- | --- | --- |
| `api.orinkhungary.hu` | A — public REST API | `false` | unchanged (small heap) | n/a | n/a |
| `mcp.orinkhungary.hu` | B — MCP + admin | `true` | 1–1.5 GB | **0** (`max_bytes`, disk-only) | 5 GB (`disk_max_bytes`) |

One binary, one pipeline, two instances. Nothing about container A changes: with
the flag off, no MCP route is registered, no precache task is spawned and the
cache is never constructed.

Sizing rule for container B — `max_bytes` governs **memory only**. Measured on the
real catalog (24,344 products, release build):

| `max_bytes` | Idle RSS | Cost per tool call |
| --- | --- | --- |
| `0` (disk-only) | **12.2 MB** | ~90 ms to load from disk |
| `300_000_000` | **102.9 MB** with one snapshot resident | 0 |

Container B ships with **`max_bytes = 0`**. On a 1–1.5 GB host a flat 12 MB
footprint that does not grow with the number of configured combinations is worth
far more than a tenth of a second per call, and an OOM kill would take the MCP
service down entirely.

If you do raise it, budget ~46 MB per resident snapshot **plus** room for three
things alive at the same time:

1. the actix server and its workers (~12 MB idle);
2. the peak of a snapshot build — the raw ~46 MB XML response *plus* the parsed
   structures derived from it, before the intermediates are dropped;
3. moka's asynchronous eviction, which lets the cache briefly overshoot its cap.

Note that even in disk-only mode a query still materializes one snapshot (~46 MB)
in memory for the duration of the call, so peak memory scales with *concurrent*
queries, not with the number of combinations.

Container B's extra runtime inputs:

- `/app/mcp_precache.toml` and `/app/mcp_cache/` — the secret mounts described above.
- `RUSTOPUS_ADMIN_TOKEN` — the `/admin` password, supplied as an environment
  variable (or a Docker/K8s secret) rather than through the tracked
  `Config.toml`. With neither set, `/admin` is not registered at all.
- Publish `/admin` only on an internal interface or behind the VPN. The token is
  the last line of defence for a credential store, not the only one it deserves.

Note that container B's precache job makes outbound SOAP calls on a timer, so it
needs the same egress to Octopus as container A, and its `soap_concurrency`
budget is shared between the sweep and live MCP traffic.

## Notes / decisions

- **Port** stays driven by the mounted `Config.toml` (`host=0.0.0.0`, `port=1140`). Keep
  `host = "0.0.0.0"` so the container is reachable; document that.
- **errors.json & docs are baked, not mounted** — they're versioned source, so they travel with
  the image and stay in sync with the binary. The same goes for `src/static/admin/`, which is
  the dashboard's own HTML/CSS/JS.
- **No code changes required.** This is purely additive packaging; the CWD-relative layout is
  satisfied by `WORKDIR /app` + the copied tree. (CLAUDE.md's "cargo check after code edits"
  rule doesn't apply — no `.rs`/source edits.)

## Verification (end-to-end)

From repo root, with a real `Config.toml` + `soap.json` present:

```powershell
docker build -t rustopus:local .
docker run --rm -p 1140:1140 `
  -v ${PWD}\Config.toml:/app/Config.toml:ro `
  -v ${PWD}\soap.json:/app/soap.json:ro `
  -v ${PWD}\log:/app/log `
  rustopus:local
```

Then check:
1. Startup log line "Running on '0.0.0.0:1140', with 2 workers" appears (stdout + `./log/<date>.log`).
2. `curl -i http://localhost:1140/` → redirects to `/docs/` (root handler), and `/docs/` serves Swagger.
3. `curl http://localhost:1140/test` → exercises `routes::test::get_handler` without needing a live Octopus backend.
4. A `log/<YYYY.MM.DD>.log` file is written into the mounted host `log/` dir (confirms volume + C FFI append work, and the non-root user can write).
5. (If reachable) hit `/products` or `/bulk` against the configured SOAP url to confirm outbound rustls TLS works from inside the container.
6. On container A only (`[mcp] enabled = false`): `curl -i http://localhost:1140/mcp` and
   `.../admin` both return **404**, and the startup log contains no `MCP` line. That is the
   check that the public API instance is genuinely unchanged.
7. On container B (`[mcp] enabled = true`, `RUSTOPUS_ADMIN_TOKEN` set): `/admin` returns 401
   without credentials and 200 with them, and `stat -c %a /app/mcp_precache.toml` reports `600`
   after the first entry is saved.

Image size sanity check: `docker images rustopus:local` should be in the low tens of MB (static binary + Alpine + docs).
