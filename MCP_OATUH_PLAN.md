# Plan: write `MCP_OAUTH_PLAN.md` (real OAuth 2.1 for `/mcp`)

## Context

The `/mcp` endpoint identifies its caller from the `X-Authcode` / `X-Pid` request
headers ([tools.rs:597](src/service/mcp/tools.rs:597)). The claude.ai **"Add custom
connector"** dialog has no field for custom headers — it offers only *no auth* or
*OAuth* (Client ID / Client Secret under Advanced settings, "Individual sign-in").
Added as-is today, the connector would complete `initialize` and `tools/list`
(neither needs credentials) and then fail **every tool call** with "Both
`X-Authcode` and `X-Pid` are required".

The user asked for the real OAuth 2.1 route to be designed and the design written
to a markdown file in the repository root.

**Deliverable: one new file, `MCP_OAUTH_PLAN.md` in the repo root.** No code
changes in this pass. Written in English, in the style and depth of the existing
[DOCKER_PLAN.md](DOCKER_PLAN.md) and [COUPA_PLAN.md](COUPA_PLAN.md) (context →
decisions → module-by-module design → gotchas → rollout → verification), so it can
be handed to whoever implements it.

## Decisions locked with the user

| Question | Chosen |
| :-- | :-- |
| What the sign-in page asks for | The existing **Octopus authcode + pid**, validated against Octopus. No new user store, no password KDF. |
| How the connector gets `client_id`/`client_secret` | **Manually, from `/admin`.** No public dynamic-client-registration endpoint (RFC 7591). |
| Today's header auth | **Kept behind a config switch.** A request with no credentials gets `401` (which is what starts Claude's OAuth flow); a request carrying `X-Authcode` is still served. |

## Design the document must specify

### 1. Roles and identifiers

Rustopus becomes both the Authorization Server and the Resource Server for its own
MCP endpoint. Issuer = `[mcp] public_url`; canonical resource URI = `{public_url}/mcp`.
Scope: a single `catalog.read`.

### 2. HTTP surface

Root-level, registered in [main.rs](src/main.rs) only when OAuth is enabled — the
`.well-known` documents must sit at the origin root, not under `/mcp`:

- `GET /.well-known/oauth-protected-resource` **and** `/.well-known/oauth-protected-resource/mcp`
  (RFC 9728) — `resource`, `authorization_servers`, `bearer_methods_supported: ["header"]`,
  `scopes_supported`. Both spellings, because clients differ on where they look.
- `GET /.well-known/oauth-authorization-server` **and** `/.well-known/oauth-authorization-server/mcp`
  (RFC 8414) — `issuer`, `authorization_endpoint`, `token_endpoint`, `revocation_endpoint`,
  `response_types_supported: ["code"]`, `grant_types_supported: ["authorization_code","refresh_token"]`,
  `code_challenge_methods_supported: ["S256"]`,
  `token_endpoint_auth_methods_supported: ["client_secret_basic","client_secret_post"]`.

Under `/oauth` (a scope, like `/admin` — not a fetcher, so no plural alias; document
it as the fourth deliberate exception to the route+alias convention):

- `GET /oauth/authorize` — validates `client_id`, exact-match `redirect_uri`,
  `response_type=code`, `code_challenge` + `code_challenge_method=S256`, `scope`,
  `resource`. Errors redirect back with `error=` when the `redirect_uri` is valid,
  otherwise render a plain page. On success renders the login form carrying an
  opaque `request_id` (in memory, 10-minute TTL) — never the raw parameters.
- `POST /oauth/login` — `request_id` + `authcode` + `pid`, form-encoded, POST only.
  Validated with one cheap Octopus call before a token is issued. On success creates
  the grant and a single-use authorization code (60 s, bound to `client_id`,
  `redirect_uri`, `code_challenge`, `resource`) and `303`s to the client's redirect
  URI with `code` and `state`.
- `POST /oauth/token` — `authorization_code` (with `code_verifier`) and
  `refresh_token` grants. Client authentication via Basic or POST body, compared in
  constant time. RFC 6749 §5.2 JSON errors, **not** the house XML error shape.
- `POST /oauth/revoke` — RFC 7009.
- `GET /oauth/login.css`, `/oauth/login.js` — served from `src/static/oauth/`,
  external files only (CSP is `script-src 'self'`), like the admin dashboard.

### 3. Token model

- **Access token** — opaque, two `uuid::Uuid::new_v4().simple()` halves, exactly as
  [export.rs:156](src/service/mcp/export.rs:156) already builds download tokens.
  Held **in memory only**, keyed by SHA-256; TTL 1 h.
- **Refresh token** — opaque, persisted as a hash, **non-rotating** (OAuth 2.1 permits
  this for confidential clients, and claude.ai supplies a client secret); TTL 30 days.
- **Grant record** — `oauth_sessions.toml`, secret-grade: it holds the authcode in
  plain text for the same reason [precache.rs](src/service/mcp/precache.rs) does — the
  server has to present it to Octopus with no user in the loop. `0600`, temp-file-and-rename,
  gitignored.
- The file is written on **login, revoke and expiry sweep only** — never on token
  refresh. This is the same reasoning the precache module already documents (a
  credential file should not be rewritten hourly) and it is what makes the
  non-rotating refresh token worth having. Access tokens are lost on restart and
  clients silently re-obtain them.

### 4. Client store

`oauth_clients.toml` — **not** secret-grade, on the [blocklist.toml](src/service/blocklist.rs)
model: the client secret is stored hashed and shown once at creation. Fields:
`client_id` (uuid), `name`, `secret_hash`, `redirect_uris` (exact list), `created_at`,
`enabled`. Managed from `/admin`.

The document lists the redirect URIs to register in practice:
`https://claude.ai/api/mcp/auth_callback` and `https://claude.com/api/mcp/auth_callback`
for the connector; loopback (`http://127.0.0.1:*`, RFC 8252 §7.3, loopback only) for
local `mcp-remote` testing.

### 5. Guarding `/mcp`

A `from_fn` middleware wrapping **only** the MCP scope (`scope_with_path("/mcp").wrap(...)`),
because rmcp's `on_request` hook is `Fn(&HttpRequest, &mut Extensions)` and **cannot
reject a request** — the `401` has to come from a middleware. Order:

1. `Authorization: Bearer` → resolve hash → grant → expiry and audience (`resource`)
   check → insert `McpAuth` into the actix request extensions.
2. No bearer, but `X-Authcode` + `X-Pid` and `oauth_allow_headers` (default `true`)
   → today's path, unchanged.
3. Otherwise `401` with
   `WWW-Authenticate: Bearer resource_metadata="{issuer}/.well-known/oauth-protected-resource"`
   (plus `error="invalid_token"` when a token was presented and failed). This response
   is what makes Claude start the OAuth flow.
4. **After** resolving the authcode from a token, call
   `blocklist::check(&[], Some(authcode), Surface::Mcp)` and return the same
   403 / error `204` body. Without this step authcode block rules silently stop
   matching on `/mcp`, because [blocklist.rs:615](src/service/blocklist.rs:615) reads
   the code from the header and an OAuth caller sends none.

[tools.rs](src/service/mcp/tools.rs)'s `extract_auth` then reads `McpAuth` from
`HttpRequest::extensions()` first and falls back to the headers — the
middleware→extensions propagation that `rmcp-actix-web` documents for exactly this.

### 6. Configuration

New all-`Option` keys under `[mcp]`, defaults in code like every existing one:
`oauth_enabled` (default `false`), `oauth_allow_headers` (default `true`),
`oauth_clients_path`, `oauth_sessions_path`, `oauth_access_ttl_secs`,
`oauth_refresh_ttl_secs`, `oauth_login_rate_limit`. Issuer comes from `public_url`;
log a startup warning when OAuth is on and `public_url` is not HTTPS or is still the
localhost default. The doc must flag the trap in
[config.rs:164](src/service/config.rs:164): `get_mcp_settings()` builds an
all-`None` `McpConfig` literal, so every new field has to be added there too.

### 7. Files the implementation will touch

New: `src/service/mcp/oauth/{mod,store,endpoints,guard}.rs`,
`src/static/oauth/{login.html,login.css,login.js}`.
Modified: [main.rs](src/main.rs) (two scopes + the wrap), [tools.rs](src/service/mcp/tools.rs)
(`extract_auth`), [mod.rs](src/service/mcp/mod.rs) (lift `secrets_match` out of
[admin.rs:72](src/service/mcp/admin.rs:72) so both use one constant-time compare),
[config.rs](src/service/config.rs), [admin.rs](src/service/mcp/admin.rs) +
`src/static/admin/*` (clients and sessions panels — `class="mcp-only"`, rows built with
`textContent`), `Config.toml`, `.gitignore`, `README.md`, `CLAUDE.md`,
[openapi.yaml](src/static/docs/openapi.yaml) under the `MCP` tag.
One dependency: `base64 = "0.22"` for base64url on the PKCE comparison — a security
path is the wrong place for the hand-rolled decoder in `admin.rs`.

### 8. Gotchas the document must call out

- **CSP `form-action 'self'` breaks the redirect out.** Chrome checks the redirect
  target of a form submission against `form-action`, so the login POST's `303` to
  claude.ai is blocked by the app-wide policy in
  [main.rs:35](src/main.rs:35). Fix: set a CSP header on the `/oauth/*` handlers —
  actix's `DefaultHeaders` only adds a header when it is absent, so the handler's
  value wins — with `form-action 'self' https://claude.ai https://claude.com`.
- **The metadata documents need CORS.** They are public documents; add
  `Access-Control-Allow-Origin: *` and drop the app-wide
  `Cross-Origin-Resource-Policy: same-origin` on those routes.
- **Cold cache after first login**: the first tool call can rebuild a snapshot
  (~260 s) and time the connector out. The `/admin` session row should offer "add to
  precache" for that `(authcode, pid)`.
- **`tests/get_test.rs` spawns the real binary on port 1140** — the committed
  `Config.toml` must keep `oauth_enabled = false`.
- **`/export/{token}` stays unauthenticated**: the link is opened in a browser that
  holds no connector credentials; the unguessable, expiring token is the guard.
- Login attempts are rate-limited per IP (in-memory counter) so the form is not an
  authcode oracle; the operator escalates to a blocklist rule.

### 9. Rollout

Merge with `oauth_enabled = false` (no behaviour change) → staging over HTTPS with a
real `public_url` → register a client in `/admin` → verify with
`npx mcp-remote https://…/mcp`, which performs the whole discovery + PKCE flow → add
the connector on claude.ai with the client id/secret → keep `oauth_allow_headers = true`
for a transition window, then turn it off.

## Verification

The plan document is the deliverable, so verification is of the document, not of code:

- `MCP_OAUTH_PLAN.md` exists in the repo root and covers every section above.
- Every file path, function name and line reference in it resolves in the current
  tree (`rg` the referenced symbols: `extract_auth`, `secrets_match`, `blocklist::check`,
  `get_mcp_settings`, `scope_with_path`, `new_token`).
- No code, config or dependency is changed in this pass — `git status` shows one new
  untracked file.

The document itself must end with the acceptance checks the *implementation* will be
held to, so they are agreed up front:

- `cargo check` and `cargo clippy --all-targets --all-features` clean.
- Unit tests in the new module: S256 mismatch rejected, authorization code single-use,
  code expiry, exact `redirect_uri` match, audience mismatch → 401, metadata JSON
  shape, non-rotating refresh, revoke.
- `cargo test` — existing `tests/get_test.rs` unaffected.
- Manual: curl each `.well-known` document; drive one full code→token→`tools/call`
  sequence; then `npx mcp-remote` end-to-end; then the claude.ai connector.
