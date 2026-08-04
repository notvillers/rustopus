<div align="center">

<img src="./src/static/docs/assets/octo.png" alt="rustopus logo" height="72">

<samp>OPEN SOURCE&nbsp;&nbsp;•&nbsp;&nbsp;<a href="LICENSE">MIT LICENSE</a>&nbsp;&nbsp;•&nbsp;&nbsp;BUILT WITH RUST</samp>

<h1>
THIS API SPEAKS OCTOPUS
</h1>

<samp>HUNGARIAN SOAP/XML IN — CLEAN ENGLISH XML (OR CSV) OUT.</samp>

<br><br>

![RUST](https://img.shields.io/badge/RUST-2024_EDITION-0038E8?style=flat-square)
![ACTIX](https://img.shields.io/badge/SERVER-ACTIX--WEB-0038E8?style=flat-square)
![VERSION](https://img.shields.io/badge/VERSION-1.1.0-0038E8?style=flat-square)
[![LICENSE](https://img.shields.io/badge/LICENSE-MIT-0038E8?style=flat-square)](LICENSE)

</div>

<br>

> Rustopus sits between the **Octopus 8 ERP** and your clients. It fetches the Hungarian-tagged SOAP payloads, translates them into English-tagged XML, CSV or XLSX — and forwards English-tagged input back to Octopus as Hungarian.

<br>

<samp>RUN VIA TERMINAL</samp>

```bash
cargo run    # reads Config.toml + soap.json from the repo root
```

<br>

## #1 CONFIGURE

<samp>TWO FILES. ZERO CEREMONY.</samp>

### [`Config.toml`](Config.toml)

Manages the defaults of the webserver.

| KEY | WHAT IT DOES | DEFAULT |
| :-- | :-- | :-- |
| `host` | Bind hostname — `"0.0.0.0"` to accept outside connections | `"0.0.0.0"` |
| `port` | Port the webapp is served on | `8080` |
| `timeout` | Timeout limit in second(s) | `1200` |
| `workers` | Worker count — the higher, the faster | `std::thread::available_parallelism()` |
| `soap_concurrency` | Max concurrent outbound SOAP calls — extra requests wait in a queue | `4` |

The optional `[mcp]` table switches on the MCP endpoint (see [#3](#3-ask)). Every
key is optional and every default is applied in code, so leaving the table out
entirely is the same as `enabled = false`.

| KEY | WHAT IT DOES | DEFAULT |
| :-- | :-- | :-- |
| `enabled` | Serve `/mcp` and `/admin`, and start the precache job | `false` |
| `max_bytes` | **In-memory** snapshot budget in bytes. **`0` disables the memory tier**, serving every query from disk: ~90 ms per call, ~12 MB idle. Above 0, budget ~46 MB per resident snapshot plus room for the server and a build's peak | `300_000_000` |
| `disk_path` | Where snapshots are mirrored on disk (relative paths resolve against the working directory) | `"mcp_cache"` |
| `disk_max_bytes` | **On-disk** budget in bytes. Stored snapshots are gzipped (~5.6 MB each) | `5_000_000_000` |
| `export_path` | Where generated Excel/CSV exports are written before download | `"mcp_exports"` |
| `export_ttl_secs` | How long an export download link stays valid | `3600` (1 h) |
| `public_url` | Base URL download links are built from. **Set this in any real deployment** | `"http://localhost:1140"` |
| `ttl_secs` | How long a cached catalog snapshot stays valid | `21600` (6 h) |
| `precache_interval_secs` | How often the refresh sweep runs | `3600` (1 h) |
| `admin_token` | `/admin` password. Prefer the `RUSTOPUS_ADMIN_TOKEN` environment variable — this file is tracked in git | unset (`/admin` not served) |

### `soap.json`

Manages the defaults of the XML handling. If the file exists in the repository
[root](/) directory, its `url` becomes the default for every GET and POST —
used for both `url` and `xmlns`.

```json
{ "url": "<default wsdl url>" }
```

### `src/static/docs/landing-config.js`

Sets the API base URL shown in the docs landing page's `CALL VIA TERMINAL`
example (`RUSTOPUS_API_BASE`). Leave it `""` to fall back to the host the
page is served from.

<br>

## #2 CALL

<samp>EVERY FETCHER, TWO NAMES — SINGULAR AND PLURAL.</samp>

`/get-product` · `/get-stock` · `/get-price` · `/get-image` · `/get-barcode` · `/get-bulk` · `/get-invoice` · `/get-mat` · `/post-order`

Ready-to-run request examples in shell, Python, JavaScript, C# and PowerShell:

**→ [DOCS](./src/static/docs/)** — when the server runs, `/` (and `/docs/`) serves the
docs landing page and `/docs/swagger.html` the live Swagger UI, rendered
from [`openapi.yaml`](./src/static/docs/openapi.yaml) — a new endpoint only
needs an `openapi.yaml` entry to show up there.

<br>

## #3 ASK

<samp>THE SAME CATALOG, ANSWERABLE BY AN AI ASSISTANT.</samp>

With `[mcp] enabled = true`, Rustopus also serves `/mcp` — a
[Model Context Protocol](https://modelcontextprotocol.io) endpoint over Streamable
HTTP. It exists because a full `/get-product` pull is ~46 MB and ~28 seconds,
which is unusable in a chat: MCP callers are answered from a cached catalog
snapshot instead, in milliseconds.

**The cache is MCP-only.** The nine endpoints above are untouched and still read
live on every call.

Snapshots are held in two tiers, because the host has limited RAM and one
snapshot is ~46 MB:

| TIER | COST OF A LOOKUP | HOLDS |
| :-- | :-- | :-- |
| Memory | microseconds | as many snapshots as `max_bytes` allows — **off when it is `0`** |
| Disk (`mcp_cache/`) | ~90 ms | everything, gzipped to ~5.6 MB each, surviving restarts |
| Octopus | minutes | the source of truth |

Measured on the real catalog — 24,344 products, release build:

| | Cold from Octopus | From disk |
| :-- | :-- | :-- |
| Load time | **262 s** | **0.09 s** |
| Idle process memory | — | **12.2 MB** disk-only vs **102.9 MB** holding one snapshot |

The shipped configuration sets `max_bytes = 0`, so nothing is held resident and
memory stays flat however many combinations exist. Raise it if you would rather
trade ~90 MB of RAM for those 90 ms.

Those files hold a partner's **own negotiated prices**, so the directory is
written `0700` with `0600` files and should be treated as sensitive.

| TOOL | ANSWERS |
| :-- | :-- |
| `search_products` | Find products by name, article number, brand or manufacturer part number — accent-insensitive, with price and stock inline. Pages with `offset` |
| `get_product` | Full master data for one article number, with a "did you mean" list when it misses |
| `list_categories` | Brands, main groups and product groups, with counts |
| `catalog_status` | Snapshot age and product count, so the assistant can state how fresh an answer is |
| `export_products` | **Excel or CSV of the whole catalog** (or any filtered slice), returned as a download link |

`export_products` exists because paging is not a bulk mechanism: ~24,000 products
cannot cross a model's context at any page size. The rows are written to a file
server-side and only a link comes back, so a full-catalog export is one call and
costs no context. Measured: **24,349 rows → 2.25 MB .xlsx**, prices and stock
written as numbers so they can be summed in Excel without retyping.

The link carries an unguessable token rather than an authcode — an authcode in a
URL would land in every access log on the way — and both the link and the file
expire (`export_ttl_secs`, default 1 hour). Set **`public_url`** to the hostname
colleagues' browsers can reach, or the links will point at localhost.

Credentials travel as request headers — never in the URL, where they would land
in access logs. Configure both on the connector:

| HEADER | VALUE |
| :-- | :-- |
| `X-Authcode` | The user's own Octopus authentication code |
| `X-Pid` | The user's partner id — prices and stock are specific to it |

`/admin` manages which `(authcode, pid)` combinations a background job keeps
warm. It holds **live authcodes at rest**, because the job runs with nobody
present to supply one, so it has its own token (`RUSTOPUS_ADMIN_TOKEN`), never
returns a full authcode, writes `mcp_precache.toml` as `0600`, and should be
bound to an internal interface or put behind the VPN.

Run the MCP instance as a **separate container** from the public API — see
[`DOCKER_PLAN.md`](DOCKER_PLAN.md). A multi-gigabyte cache in the process serving
`/get-product` would let one OOM take the API down for every existing consumer.
