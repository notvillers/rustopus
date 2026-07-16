<div align="center">

<img src="./client/src/assets/images/octopus.png" alt="rustopus logo" height="72">

<br><br>

<samp>OPEN SOURCE&nbsp;&nbsp;•&nbsp;&nbsp;<a href="LICENSE">MIT LICENSE</a>&nbsp;&nbsp;•&nbsp;&nbsp;BUILT WITH RUST</samp>

<h1>
THE BRIDGE<br>
THAT SPEAKS<br>
OCTOPUS
</h1>

<samp>HUNGARIAN SOAP/XML IN — CLEAN ENGLISH XML (OR CSV) OUT.</samp>

<br><br>

![RUST](https://img.shields.io/badge/RUST-2024_EDITION-0038E8?style=flat-square)
![ACTIX](https://img.shields.io/badge/SERVER-ACTIX--WEB-0038E8?style=flat-square)
![VERSION](https://img.shields.io/badge/VERSION-1.1.0-0038E8?style=flat-square)
[![LICENSE](https://img.shields.io/badge/LICENSE-MIT-0038E8?style=flat-square)](LICENSE)

</div>

<br>

> Rustopus sits between the **Octopus 8 ERP** and your clients. It fetches the
> Hungarian-tagged SOAP payloads, translates them into English-tagged XML or
> CSV — and forwards English-tagged input back to Octopus as Hungarian.

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

### `soap.json`

Manages the defaults of the XML handling. If the file exists in the repository
[root](/) directory, its `url` becomes the default for every GET and POST —
used for both `url` and `xmlns`.

```json
{ "url": "<default wsdl url>" }
```

<br>

## #2 CALL

<samp>EVERY FETCHER, TWO NAMES — SINGULAR AND PLURAL.</samp>

`/get-product` · `/get-stock` · `/get-price` · `/get-image` · `/get-barcode` · `/get-bulk` · `/get-invoice` · `/get-mat` · `/post-order`

Ready-to-run request examples in shell, Python, JavaScript, C# and PowerShell:

**→ [DOCS](./docs/)** — when the server runs, `/docs/` serves the landing page and `/docs/swagger.html` the live Swagger UI.

<br>

## #3 SCHEDULE

<samp>RUSTOPUS-CLIENT — THE DESKTOP COMPANION.</samp>

A native GUI app to exercise the server and cron the calls, unattended.

**→ [CLIENT README](./client/README.md)**

<br>

---

<div align="center">
<samp>RUSTOPUS V1.1.0 · <a href="LICENSE">MIT LICENSE</a> · 2026</samp>
</div>
