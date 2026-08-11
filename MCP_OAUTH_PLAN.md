# OAuth 2.1 for `/mcp` (claude.ai custom connector)

## Context

`/mcp` identifies its caller from two request headers — `X-Authcode` and `X-Pid`
(`src/service/mcp/tools.rs:597`, names in `src/service/mcp/mod.rs`). That works for
`mcp-remote`, for `curl` and for Claude Code, all of which let the operator set
arbitrary headers.

It does not work for the **claude.ai "Add custom connector" dialog**, which has no
custom-header field. It offers exactly two authentication shapes:

- **no auth** — the client sends nothing, or
- **OAuth** — with an optional Client ID / Client Secret under *Advanced settings*
  ("Individual sign-in": each member of the organization signs in to connect).

Added as-is today, the connector would look like it worked: `initialize` and
`tools/list` need no credentials, so the tool list would appear. Then **every tool
call** would fail with *"Both `X-Authcode` and `X-Pid` are required"*, because the
headers the connector cannot send are the only identity `/mcp` understands.

This document plans the real fix: Rustopus issues its own OAuth 2.1 tokens, and
`/mcp` becomes an OAuth-protected resource in the sense the MCP specification means.
It is a **plan, not a changelog** — nothing described here is implemented yet.

Two things are deliberately **not** in scope. Dynamic Client Registration (RFC 7591)
is not implemented: clients are created by hand in `/admin`, which is what the
dialog's Client ID / Client Secret fields exist for. And there is no new user
directory: the sign-in page asks for the Octopus credentials the partner already
has.

## Decisions taken up front

| Question | Decision | Why |
| :-- | :-- | :-- |
| What the sign-in page asks for | The existing **Octopus authcode + pid** | No new credential to issue, revoke or reset, and no password KDF dependency. OAuth here wraps the credential the partner already holds rather than inventing a parallel one. |
| How a connector gets `client_id` / `client_secret` | **By hand, from `/admin`** | An unauthenticated, writing endpoint on the public internet needs rate limiting, a cap and a sweeper. Two fields pasted once per organization do not. |
| Today's header authentication | **Kept, behind `oauth_allow_headers`** | A credential-less request still gets the `401` that starts Claude's OAuth flow, so nothing about the connector suffers — while `mcp-remote`, `curl` and any existing integration keep working through the transition. |
| Refresh-token rotation | **Not rotated** | OAuth 2.1 requires rotation *or* a confidential client; claude.ai supplies a client secret, so the client is confidential. Not rotating is what lets the credential file stay untouched between logins (see "Why the session file is not rewritten hourly"). |

## 1. Roles and identifiers

Rustopus becomes both the **authorization server** and the **resource server** for
its own MCP endpoint. There is no third party.

| Thing | Value |
| :-- | :-- |
| Issuer | `[mcp] public_url`, e.g. `https://mcp.orinkhungary.hu` |
| Canonical resource URI | `{public_url}/mcp` |
| Scope | one: `catalog.read` |
| Token type | `Bearer`, opaque (not a JWT) |

`public_url` is already required for export download links; OAuth gives it a second
job. With OAuth enabled the server must log a startup warning when `public_url` is
absent, still the localhost default, or not `https://` — every URL in the metadata
documents is built from it, and a wrong value produces a connector that fails during
discovery with nothing in the log to explain it.

## 2. HTTP surface

### 2.1 Metadata documents (origin root)

These must sit at the **root**, not under `/mcp`; they are registered in `main.rs`
only when OAuth is enabled. Both spellings of each are served, because clients
disagree about where to look — the cost is two extra route registrations.

`GET /.well-known/oauth-protected-resource` and
`GET /.well-known/oauth-protected-resource/mcp` (RFC 9728):

```json
{
  "resource": "https://mcp.orinkhungary.hu/mcp",
  "authorization_servers": ["https://mcp.orinkhungary.hu"],
  "bearer_methods_supported": ["header"],
  "scopes_supported": ["catalog.read"],
  "resource_documentation": "https://mcp.orinkhungary.hu/docs/"
}
```

`GET /.well-known/oauth-authorization-server` and
`GET /.well-known/oauth-authorization-server/mcp` (RFC 8414):

```json
{
  "issuer": "https://mcp.orinkhungary.hu",
  "authorization_endpoint": "https://mcp.orinkhungary.hu/oauth/authorize",
  "token_endpoint": "https://mcp.orinkhungary.hu/oauth/token",
  "revocation_endpoint": "https://mcp.orinkhungary.hu/oauth/revoke",
  "response_types_supported": ["code"],
  "grant_types_supported": ["authorization_code", "refresh_token"],
  "code_challenge_methods_supported": ["S256"],
  "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post"],
  "scopes_supported": ["catalog.read"],
  "service_documentation": "https://mcp.orinkhungary.hu/docs/"
}
```

No `registration_endpoint` is published. A client that can only do dynamic
registration therefore fails at discovery rather than silently half-working — which
is the intended outcome, and the error to expect if a future client stops accepting
manual credentials.

### 2.2 The `/oauth` scope

Mounted as a **scope**, like `/admin`: it is a small internal app with its own
authentication model, not a fetcher, so the singular/plural alias convention in
`CLAUDE.md` does not apply. Document it at the mount site as the fourth deliberate
exception, alongside `/mcp`, `/admin` and `/export/{token}`.

| Route | Purpose |
| :-- | :-- |
| `GET /oauth/authorize` | Validate the authorization request, render the sign-in page |
| `POST /oauth/login` | Accept authcode + pid, mint the authorization code, redirect back |
| `POST /oauth/token` | `authorization_code` and `refresh_token` grants |
| `POST /oauth/revoke` | RFC 7009 token revocation |
| `GET /oauth/login.css`, `GET /oauth/login.js` | Sign-in page assets from `src/static/oauth/` |

**`GET /oauth/authorize`** validates, in this order: `client_id` exists and is
enabled; `redirect_uri` matches one of that client's registered URIs **exactly**
(string equality — no prefix matching, no wildcards); `response_type=code`;
`code_challenge` present with `code_challenge_method=S256`; `scope` empty or
`catalog.read`; `resource`, when present, equals the canonical resource URI.

A failure after the client and redirect URI are known redirects back with
`error=`/`error_description=`/`state=`, per RFC 6749 §4.1.2.1. A failure *of* the
client or redirect URI renders a plain error page instead — redirecting to an
unverified URI is how open redirectors are built.

On success it stores the validated request in memory under an opaque `request_id`
(10-minute TTL) and renders the sign-in form carrying only that id. The raw
parameters never make a round trip through the browser, so nothing the user's page
can be tricked into echoing back changes what the code is bound to.

**`POST /oauth/login`** takes `request_id`, `authcode`, `pid`, form-encoded, POST
only — an authcode must never reach a query string, where it would land in every
access log between the browser and here (the same rule that keeps authcodes out of
`/export` links).

Validation before a token is issued:

1. A **cheap Octopus call** proves the authcode: `RequestGet::Products` through the
   existing dispatch in `src/service/get_data.rs`, with `from_date` set to now, which
   makes the pull incremental and returns almost no rows. A bad authcode fails here
   in well under a second. *Verify against the live ERP during implementation that
   the date filter really is honoured; if it is not, this check falls back to another
   narrow call rather than pulling the full catalogue on every login.*
2. The **pid is not verified synchronously**. Only prices vary by pid
   (`src/service/mcp/index.rs:818` — `GetCikkekAuth` and the stock call take an
   authcode alone), so proving it means a full price pull, which is far too slow to
   sit in a login form. Instead, login **spawns a background snapshot build** for the
   new `(authcode, pid)`. That both surfaces a wrong pid as a visible error in
   `/admin` and warms the cache, so the user's first question does not wait ~260 s
   for a cold catalogue.

On success it creates the grant (§3), mints a single-use authorization code (60 s,
bound to `client_id`, `redirect_uri`, `code_challenge` and `resource`) and answers
`303 See Other` to the client's redirect URI with `code` and the original `state`.

**`POST /oauth/token`** handles both grants. Client authentication is
`client_secret_basic` or `client_secret_post`, compared in constant time (§7 —
`secrets_match` moves out of `admin.rs` so there is one such comparison in the
codebase). PKCE is verified as
`BASE64URL-ENCODE(SHA256(ASCII(code_verifier))) == code_challenge`, unpadded.
An authorization code is deleted on first use, whether or not the exchange succeeds.
Errors are RFC 6749 §5.2 JSON (`{"error": "invalid_grant"}`) — deliberately **not**
the house XML error shape, because an OAuth client parses this, not a partner.

Success:

```json
{
  "access_token": "…",
  "token_type": "Bearer",
  "expires_in": 3600,
  "refresh_token": "…",
  "scope": "catalog.read"
}
```

## 3. Tokens and grants

| | Form | Where it lives | Lifetime |
| :-- | :-- | :-- | :-- |
| Authorization code | opaque | memory | 60 s, single use |
| Access token | opaque | **memory only**, keyed by SHA-256 | 1 h (`oauth_access_ttl_secs`) |
| Refresh token | opaque, stored **hashed** | `oauth_sessions.toml` | 30 days (`oauth_refresh_ttl_secs`) |
| Grant | record | `oauth_sessions.toml` | until revoked or the refresh token expires |

Tokens are built the way download tokens already are — two
`uuid::Uuid::new_v4().simple()` halves (`src/service/mcp/export.rs:156`), which is
~244 bits from the OS random source, through a dependency this crate already has.

`oauth_sessions.toml` is **secret-grade**: it holds the authcode in plain text, for
exactly the reason `mcp_precache.toml` does — the server has to present it to Octopus
with no user in the loop. `0600`, written through a temp file and a rename, gitignored,
mounted as a secret. One grant:

```toml
[[grant]]
id = "9f3c1a2b-1234"        # fingerprint(hash(authcode)) + pid, as PrecacheEntry::id does
client_id = "…"
label = "FFD3…0E37 pid=1234"
authcode = "…"               # SECRET
pid = 1234
resource = "https://mcp.orinkhungary.hu/mcp"
scope = "catalog.read"
refresh_hash = "…"           # SHA-256 hex of the refresh token, never the token
created_at = "2026-08-11T09:12:00Z"
expires_at = "2026-09-10T09:12:00Z"
```

### Why the session file is not rewritten hourly

`src/service/mcp/precache.rs` already argues that a file holding live credentials
should not be rewritten on a timer. The same reasoning drives two choices here:

- **Access tokens are never persisted.** They live in memory, so an hourly refresh
  touches no file. A restart drops them; clients notice a `401`, refresh, and carry
  on without a human involved.
- **Refresh tokens are not rotated.** Rotation would mean writing the credential file
  every time an access token expires. The client is confidential, so OAuth 2.1 does
  not require it.

The file is therefore written on **login, revocation and the expiry sweep** — that
is, when an administrator or a user actually did something.

## 4. Client store

`oauth_clients.toml`, on the `blocklist.toml` model rather than the
`mcp_precache.toml` one: it is **not** secret-grade, because the client secret is
stored hashed and shown exactly once, at creation. It is still `0600` through a
temp-file-and-rename — nothing but this server needs to read it.

```toml
[[client]]
client_id = "…"                # uuid
name = "Orink Hungary (claude.ai organization)"
secret_hash = "…"              # SHA-256 hex
redirect_uris = [
  "https://claude.ai/api/mcp/auth_callback",
  "https://claude.com/api/mcp/auth_callback"
]
created_at = "2026-08-11T09:00:00Z"
enabled = true
```

Redirect URIs to register in practice: the two claude.ai callbacks above for the
connector, and — for local `mcp-remote` testing only — loopback URIs. `mcp-remote`
listens on a random localhost port, so loopback is matched by host with any port, per
RFC 8252 §7.3, and **only** for `127.0.0.1` / `::1` over `http`. Every other URI is
matched by exact string.

## 5. Guarding `/mcp`

A `from_fn` middleware wrapping **only** the MCP scope:
`service.scope_with_path("/mcp").wrap(from_fn(oauth::guard))`.

It has to be a middleware rather than the transport's own hook: rmcp's
`on_request` is `Fn(&HttpRequest, &mut Extensions)`
(`rmcp-actix-web/src/transport/streamable_http_server.rs:81`) and **cannot reject a
request**. Wrapping the scope also covers the transport's `GET` (SSE) and `DELETE`
alongside `POST`, which per-route plumbing would not.

Order of business:

1. **`Authorization: Bearer …`** → hash → look up the grant → check expiry and that
   the token's `resource` is this server's canonical URI (RFC 8707 audience binding;
   a token minted for another resource must not work here) → insert `McpAuth` into
   the actix request extensions.
2. **No bearer, but `X-Authcode` + `X-Pid`**, and `oauth_allow_headers` is on →
   today's path, unchanged.
3. **Otherwise `401`**, with

   ```
   WWW-Authenticate: Bearer resource_metadata="https://mcp.orinkhungary.hu/.well-known/oauth-protected-resource"
   ```

   plus `error="invalid_token"` when a token *was* presented and failed. This
   response is what makes Claude start the OAuth flow at all; without it the
   connector assumes an unauthenticated server and never asks anyone to sign in.
4. **Re-establish the blocklist check.** Once the authcode is known from the token,
   call `blocklist::check(&[], Some(authcode), Surface::Mcp)` and return the same
   `403` / error `204` body the middleware in `src/service/blocklist.rs` returns.
   This is not optional: the app-wide guard reads the code from the `X-Authcode`
   header (`src/service/blocklist.rs:615`), an OAuth caller sends no such header, and
   without this step **every authcode block rule silently stops matching on `/mcp`**.
   IP rules are unaffected — they are matched before this point.

`extract_auth` in `src/service/mcp/tools.rs` then reads `McpAuth` from
`HttpRequest::extensions()` first and falls back to the headers. This is the
middleware→extensions propagation `rmcp-actix-web` documents for exactly this case,
so the tools themselves need no change: they keep reading `McpAuth` from
`RequestContext::extensions`, and no tool gains a partner argument.

`/export/{token}` stays unauthenticated. The link is opened in a browser that holds
no connector credentials; the unguessable, expiring token is the guard, and putting a
bearer requirement on it would break the one thing it exists to do.

## 6. Configuration

New keys under `[mcp]` in `Config.toml`, all `Option<T>` with defaults in code, like
every key already there:

| Key | Default | Meaning |
| :-- | :-- | :-- |
| `oauth_enabled` | `false` | Registers the metadata and `/oauth` scopes, arms the guard |
| `oauth_allow_headers` | `true` | Keep serving `X-Authcode` / `X-Pid` callers |
| `oauth_clients_path` | `oauth_clients.toml` | Client store |
| `oauth_sessions_path` | `oauth_sessions.toml` | Grant store (secret-grade) |
| `oauth_access_ttl_secs` | `3600` | Access-token lifetime |
| `oauth_refresh_ttl_secs` | `2592000` | Refresh-token lifetime (30 days) |
| `oauth_login_rate_limit` | `10` | Failed sign-ins per IP per 10 minutes |

**Trap to remember:** `get_mcp_settings()` in `src/service/config.rs:164` builds an
all-`None` `McpConfig` literal for the case where the `[mcp]` table is absent. Every
new field has to be added there too or the crate will not compile — and the compile
error is the good outcome, so do not paper over it with `..Default::default()`.

The issuer is `public_url`; there is no separate `oauth_issuer` key, because two
sources of truth for the same hostname is how a metadata document ends up pointing at
a server nobody can reach.

## 7. Files

**New**

```
src/service/mcp/oauth/mod.rs         types, config accessors, the grant/token model
src/service/mcp/oauth/store.rs       clients + sessions on disk (RwLock, temp-file-and-rename, 0600)
src/service/mcp/oauth/endpoints.rs   metadata, authorize, login, token, revoke; scope() + well_known_scope()
src/service/mcp/oauth/guard.rs       the /mcp middleware and bearer resolution
src/static/oauth/login.html          sign-in page
src/static/oauth/login.css
src/static/oauth/login.js
```

`store.rs` is written against the shape `src/service/blocklist.rs` already
established — a `Lazy<RwLock<…>>` over the parsed file, `load()` / `save()` /
`upsert()` / `remove()`, `restrict_permissions()` to `0600`, a missing file treated
as "nothing configured" rather than an error. Copy the structure, not the code.

**Modified**

| File | Change |
| :-- | :-- |
| `src/main.rs` | Register the two metadata routes and the `/oauth` scope when enabled; wrap the MCP scope with the guard |
| `src/service/mcp/tools.rs` | `extract_auth` reads request extensions first, headers second |
| `src/service/mcp/mod.rs` | `secrets_match` lifted here from `admin.rs:72` so both callers share one constant-time compare |
| `src/service/mcp/admin.rs` | OAuth client and session panels in the API (§8) |
| `src/service/config.rs` | The seven keys above, plus the all-`None` literal |
| `src/static/admin/*` | Two new panels, `class="mcp-only"`, rows built with `textContent` |
| `Config.toml`, `.gitignore` | New keys (disabled); ignore both new files |
| `README.md`, `CLAUDE.md` | Connector setup; the new module and files |
| `src/static/docs/openapi.yaml` | `/oauth/*` and the metadata documents under the `MCP` tag, flagged as non-REST like `/mcp` and `/admin` |

**Dependency:** `base64 = "0.22"`, for base64url on the PKCE path. The hand-rolled
decoder in `admin.rs` is standard-alphabet and was written for one 40-character
string; a PKCE comparison is a security boundary and is the wrong place to reuse it.

## 8. `/admin` additions

`GET /admin/api/state` gains an `oauth` section — `null` when OAuth is off, exactly
as `cache` and `disk` are `null` on a REST-only instance:

```json
"oauth": {
  "enabled": true,
  "clients":  [{ "client_id": "…", "name": "…", "redirect_uris": [], "enabled": true, "created_at": "…" }],
  "sessions": [{ "id": "…", "client": "…", "authcode": "FFD3…0E37", "pid": 1234,
                 "created_at": "…", "expires_at": "…", "last_used": "…", "precached": false }]
}
```

- `POST /admin/api/oauth/clients` — create; **the only response that ever carries the
  client secret**, and it says so.
- `PATCH` / `DELETE /admin/api/oauth/clients/{id}` — rename, edit redirect URIs,
  enable/disable, delete.
- `DELETE /admin/api/oauth/sessions/{id}` — revoke a grant; its access tokens are
  dropped from memory in the same call.

Session rows are subject to the same rule as everything else in this dashboard: the
authcode is shown masked through `mask_authcode`, never in full, not even in an
error. The `precached: false` flag gives the operator the obvious next click — adding
that `(authcode, pid)` to the precache so the partner's first question is not the one
that waits for a cold catalogue.

## 9. Gotchas

- **The app-wide CSP blocks the redirect back to Claude.** `security_headers()` in
  `src/main.rs:35` sets `form-action 'self'`, and Chrome applies `form-action` to the
  *redirect target* of a form submission — so the `303` from `POST /oauth/login` to
  `https://claude.ai/…` is refused, with a console error and no server-side symptom
  at all. Fix: set an explicit CSP on the `/oauth/*` handlers,
  `form-action 'self' https://claude.ai https://claude.com`. Actix's `DefaultHeaders`
  only adds a header when it is absent, so the handler's value wins.
- **The metadata documents need CORS.** They are public documents fetched by clients
  from other origins; add `Access-Control-Allow-Origin: *` on those routes and drop
  the app-wide `Cross-Origin-Resource-Policy: same-origin` there.
- **The sign-in page cannot use inline JavaScript.** `script-src 'self'` — the assets
  are external files, exactly like the admin dashboard's.
- **Blocklist coverage regresses silently without §5.4.** Worth a test of its own.
- **`tests/get_test.rs` spawns the real binary** on the port from `Config.toml`. Keep
  `oauth_enabled = false` in the committed file so the integration test is untouched.
- **Rate-limit the sign-in form.** Without it, `/oauth/login` is an oracle for
  guessing authcodes against the ERP. An in-memory counter per IP, `429` past the
  limit, and the operator escalates to a blocklist rule; failures are logged with the
  IP and the masked code.
- **Config changes need a restart.** `get_settings()` parses `Config.toml` once into
  a cached `Lazy`; so does everything built from it here.

## 10. Rollout

1. Merge with `oauth_enabled = false`. No route is registered, no file is read, no
   behaviour changes — the same posture `[mcp] enabled` already has.
2. Bring up a staging instance over HTTPS with a real `public_url`.
3. Create a client in `/admin`, note the secret.
4. `curl` each metadata document; drive one full code → token → `tools/call` sequence
   by hand.
5. `npx mcp-remote https://…/mcp` — it performs discovery, PKCE and the browser
   round trip end to end, and is the last cheap failure before Claude is involved.
6. Add the connector on claude.ai: the `/mcp` URL, then Client ID and Client Secret
   under *Advanced settings*. Organization-wide connectors are added by an owner.
7. Leave `oauth_allow_headers = true` for a transition window, migrate the remaining
   header-based callers, then turn it off.

## 11. Acceptance checks

- `cargo check` and `cargo clippy --all-targets --all-features` clean.
- Unit tests in the new module:
  - a `code_verifier` that does not hash to the `code_challenge` is rejected;
  - an authorization code cannot be used twice, and is consumed even on a failed
    exchange;
  - an expired authorization code is rejected;
  - `redirect_uri` matching is exact, and loopback-with-any-port applies only to
    loopback;
  - a token whose `resource` is not this server's canonical URI produces `401`;
  - both metadata documents serialize with the fields listed in §2.1;
  - a refresh exchange returns a new access token and the **same** refresh token;
  - revocation makes both tokens stop working immediately;
  - a blocked authcode presented through a valid bearer token still gets `403` / `204`.
- `cargo test` — `tests/get_test.rs` unaffected.
- Manual, in the order of §10.
