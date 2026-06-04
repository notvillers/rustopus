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
```
Mounted at runtime (volumes):
```
/app/Config.toml   (ro)
/app/soap.json     (ro)
/app/log/          (rw, persisted)
```

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
`Config.toml` / `soap.json` (those are mounted, not baked).

### 3. (Optional) `compose.yaml` — convenience run

Single `rustopus` service: builds the Dockerfile, maps host port → container `1140`, and mounts
`./Config.toml`, `./soap.json` read-only plus `./log` read-write. Lets the user `docker compose up`
without remembering the `-v` flags. Include only if the user wants it.

## Notes / decisions

- **Port** stays driven by the mounted `Config.toml` (`host=0.0.0.0`, `port=1140`). Keep
  `host = "0.0.0.0"` so the container is reachable; document that.
- **errors.json & docs are baked, not mounted** — they're versioned source, so they travel with
  the image and stay in sync with the binary.
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

Image size sanity check: `docker images rustopus:local` should be in the low tens of MB (static binary + Alpine + docs).
