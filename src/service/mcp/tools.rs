//! The MCP service itself: tool definitions plus the Streamable HTTP transport
//! that carries them.
//!
//! Caller identity arrives as HTTP headers (`X-Authcode`, `X-Pid`), is lifted
//! into rmcp's request extensions by the `on_request` hook below, and is read
//! back inside each tool through `RequestContext::extensions`. **No tool takes a
//! partner argument** — the pid is fixed per user by their connector config, so
//! a model cannot ask for another partner's prices.
//!
//! Four tools, shaped by what people ask rather than by SOAP operation. Every
//! tool definition costs context on every request, so there is deliberately no
//! tool-per-endpoint mapping, and deliberately **no sync tool**: refresh is the
//! precache job's business, and a model-triggered 28-second sync is exactly what
//! this design exists to prevent.

use std::sync::Arc;
use std::time::Duration;

use actix_web::HttpRequest;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, Implementation, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router
};
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp_actix_web::transport::StreamableHttpService;
use serde_json::json;

use crate::{
    macros::mcp::McpToolArgs,
    service::{
        log::{elogger, logger},
        mcp::{
            AUTHCODE_HEADER, McpAuth, PID_HEADER,
            cache::cache,
            index::{CatalogSnapshot, SearchFilters, fold},
            mask_authcode
        },
        soap_config::get_default_url
    }
};

/// How often the transport sends SSE keep-alive pings.
const SSE_KEEP_ALIVE_SECS: u64 = 30;

/// Results returned when a search does not ask for a specific count.
const DEFAULT_SEARCH_LIMIT: usize = 20;

/// Hard ceiling on results, applied server-side whatever the caller asks for.
/// A larger page buries the answer and burns the context window.
const MAX_SEARCH_LIMIT: usize = 50;

/// Categories listed per kind before the list is truncated.
const DEFAULT_CATEGORY_LIMIT: usize = 40;

/// Near matches offered when an article number is not found.
const DID_YOU_MEAN_LIMIT: usize = 5;


McpToolArgs! {
    pub struct SearchProductsArgs {
        /// Words to search for. Accent- and case-insensitive: `szovegkiemelo`
        /// matches `Szövegkiemelő`. Every word must match somewhere.
        pub query: String,
        /// Restrict to a brand / manufacturer, e.g. "Orink".
        pub brand: Option<String>,
        /// Restrict to a product group, by code or name.
        pub category: Option<String>,
        /// Restrict to a main group, by code or name.
        pub main_category: Option<String>,
        /// How many results to return. Clamped to 50.
        pub limit: Option<u32>
    }

    pub struct GetProductArgs {
        /// Article number (`cikkszam`). A manufacturer part number or the
        /// internal record id also resolves.
        pub no: String
    }

    pub struct ListCategoriesArgs {
        /// Which grouping to list: `brands`, `main_categories`, `categories`, or
        /// `all` (the default).
        pub kind: Option<String>,
        /// How many entries per grouping. Clamped to 200.
        pub limit: Option<u32>
    }
}


/// The MCP service. One instance is built per session by the transport's service
/// factory; all shared state lives in the process-wide cache rather than in this
/// struct, so constructing it is cheap.
#[derive(Clone)]
pub struct RustopusMcp {
    #[expect(
        dead_code,
        reason = "Initialized by Self::new(); the #[tool_handler] macro reaches the router \
                  through Self::tool_router(), not through this field. Matches the pattern in \
                  rmcp-actix-web's own examples."
    )]
    tool_router: ToolRouter<RustopusMcp>
}

impl Default for RustopusMcp {
    fn default() -> Self {
        Self::new()
    }
}

#[tool_router]
impl RustopusMcp {
    pub fn new() -> Self {
        Self { tool_router: Self::tool_router() }
    }

    /// Search the catalog. The workhorse: name, article number, brand and
    /// manufacturer part number in one ranked pass, with the caller's own price
    /// and stock inline so a follow-up call is not needed to answer "how much".
    #[tool(description = "Search Orink products by name, article number, brand or manufacturer part number. \
        Accent-insensitive. Returns the caller's own price and current stock inline.")]
    async fn search_products(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<SearchProductsArgs>
    ) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        if args.query.trim().is_empty() {
            return Ok(CallToolResult::error(vec![Content::text(
                "query must not be empty. Pass the words to search for, e.g. \"szovegkiemelo sarga\"."
            )]))
        }

        let limit = clamp(args.limit, DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT);
        let filters = SearchFilters {
            brand: args.brand.as_deref().map(fold),
            category: args.category.as_deref().map(fold),
            main_category: args.main_category.as_deref().map(fold)
        };

        let outcome = snapshot.search(&args.query, &filters, limit);

        if outcome.matched == 0 {
            // An empty result set is actionable, not exceptional: the model can
            // retry with fewer words or a different spelling.
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "No product matched '{}'{}. Every word must match; try fewer words, \
                 or use list_categories to see the available brands and groups.",
                args.query,
                describe_filters(&args)
            ))]))
        }

        Ok(json_result(json!({
            "matched": outcome.matched,
            "returned": outcome.results.len(),
            "truncated": outcome.matched > outcome.results.len(),
            "catalog_age_seconds": snapshot.age_secs(),
            "results": outcome.results
        })))
    }

    /// Everything known about one product, for when a search result needs
    /// following up.
    #[tool(description = "Full master data for one Orink product by article number, \
        including the caller's own price and current stock.")]
    async fn get_product(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<GetProductArgs>
    ) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        match snapshot.get_by_no(&args.no) {
            Some(product) => Ok(json_result(json!({
                "catalog_age_seconds": snapshot.age_secs(),
                "product": product
            }))),
            None => {
                // Offer near matches rather than dead-ending: an article number
                // is usually mistyped, not absent.
                let suggestions = snapshot.did_you_mean(&args.no, DID_YOU_MEAN_LIMIT);
                let message = if suggestions.is_empty() {
                    format!("No product with article number '{}'.", args.no)
                } else {
                    format!(
                        "No product with article number '{}'. Did you mean: {}?",
                        args.no,
                        suggestions.iter()
                            .map(|s| format!("{} ({})", s.no, s.name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                };
                Ok(CallToolResult::error(vec![Content::text(message)]))
            }
        }
    }

    /// The vocabulary of the catalog, so filters can be chosen from real values
    /// rather than guessed.
    #[tool(description = "List the brands, main groups and product groups in the Orink catalog, with product counts.")]
    async fn list_categories(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ListCategoriesArgs>
    ) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        let limit = clamp(args.limit, DEFAULT_CATEGORY_LIMIT, 200);
        let categories = snapshot.categories();
        let kind = args.kind.as_deref().unwrap_or("all").to_lowercase();

        let mut payload = json!({ "catalog_age_seconds": snapshot.age_secs() });
        let mut include = |name: &str, all: &[crate::service::mcp::index::CategoryCount]| {
            if let Some(object) = payload.as_object_mut() {
                object.insert(name.to_string(), json!({
                    "total": all.len(),
                    "truncated": all.len() > limit,
                    "items": all.iter().take(limit).collect::<Vec<_>>()
                }));
            }
        };

        match kind.as_str() {
            "brands" => include("brands", &categories.brands),
            "main_categories" | "main" => include("main_categories", &categories.main_categories),
            "categories" | "groups" => include("categories", &categories.categories),
            _ => {
                include("brands", &categories.brands);
                include("main_categories", &categories.main_categories);
                include("categories", &categories.categories);
            }
        }

        Ok(json_result(payload))
    }

    /// Freshness, so an answer can be qualified instead of implied to be live.
    #[tool(description = "How many products the cached Orink catalog holds and how old the data is.")]
    async fn catalog_status(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        Ok(json_result(json!({
            "products": snapshot.products.len(),
            "fetched_at": snapshot.fetched_at.to_rfc3339(),
            "age_seconds": snapshot.age_secs(),
            "priced_products": snapshot.products.iter().filter(|p| p.price.is_some()).count(),
            "note": "Prices and stock are specific to the partner id configured on this connector."
        })))
    }

    /// The caller's catalog snapshot, or a ready-made error result explaining
    /// what is missing.
    ///
    /// Returns `Err(CallToolResult)` rather than `Err(McpError)` on purpose: a
    /// missing header or a bad authcode is the caller's to fix, and a tool-level
    /// error reaches the user, where a protocol error would be rendered opaquely.
    async fn snapshot(
        &self,
        context: &RequestContext<RoleServer>
    ) -> Result<Arc<CatalogSnapshot>, CallToolResult> {
        let Some(auth) = context.extensions.get::<McpAuth>() else {
            return Err(CallToolResult::error(vec![Content::text(format!(
                "This connector is not sending credentials. Both the {} and {} request headers \
                 must be configured on it before product data can be read.",
                AUTHCODE_HEADER, PID_HEADER
            ))]))
        };

        let Some(url) = get_default_url() else {
            // Server-side misconfiguration, not the caller's problem — but they
            // still need to be told why nothing works.
            elogger("MCP: no default SOAP url configured (soap.json missing or empty)");
            return Err(CallToolResult::error(vec![Content::text(
                "The Rustopus server has no Octopus url configured, so no catalog can be read. \
                 This is a server-side configuration problem."
            )]))
        };

        match cache().get_or_build(&auth.authcode, auth.pid, &url).await {
            Ok(snapshot) => Ok(snapshot),
            Err(error) => {
                elogger(format!("MCP snapshot failed for {}: {}", auth.masked(), error));
                Err(CallToolResult::error(vec![Content::text(format!(
                    "Could not read the catalog from the ERP: {}. If this persists, check that the \
                     authcode and partner id on this connector are correct.",
                    error
                ))]))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for RustopusMcp {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rustopus-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(
                "Orink product data from the Octopus 8 ERP. Prices and stock are specific to the \
                 partner id configured on this connector, so figures are the caller's own, not list \
                 prices. Data comes from a periodically refreshed snapshot — call catalog_status \
                 when the freshness of an answer matters."
            )
    }
}


/// Serializes a payload as the tool's text content.
///
/// Serialization of a plain data payload cannot realistically fail, but this
/// service must never panic, so a failure degrades to a readable message.
fn json_result(payload: serde_json::Value) -> CallToolResult {
    match serde_json::to_string(&payload) {
        Ok(text) => CallToolResult::success(vec![Content::text(text)]),
        Err(error) => {
            elogger(format!("MCP: failed to serialize tool result: {}", error));
            CallToolResult::error(vec![Content::text("The result could not be serialized.")])
        }
    }
}


/// Applies the caller's requested size within server-side bounds. A `0` or a
/// missing value falls back to the default rather than returning nothing.
fn clamp(requested: Option<u32>, default: usize, max: usize) -> usize {
    match requested {
        Some(0) | None => default,
        Some(value) => (value as usize).min(max)
    }
}


/// Renders the active filters for a "nothing matched" message, so the model can
/// see whether a filter, rather than the query, was the problem.
fn describe_filters(args: &SearchProductsArgs) -> String {
    let mut parts = Vec::new();
    if let Some(brand) = &args.brand {
        parts.push(format!("brand '{}'", brand));
    }
    if let Some(category) = &args.category {
        parts.push(format!("category '{}'", category));
    }
    if let Some(main_category) = &args.main_category {
        parts.push(format!("main category '{}'", main_category));
    }
    if parts.is_empty() {
        return String::new()
    }
    format!(" with {}", parts.join(" and "))
}


/// Reads `X-Authcode` / `X-Pid` off the HTTP request and hands them to the MCP
/// layer as a typed extension.
///
/// A request missing either header is not rejected here: the transport also
/// carries `initialize` and `tools/list`, which need no credentials. Tools that
/// do need them return an actionable MCP error instead, which the model can act
/// on, unlike a transport-level rejection.
fn extract_auth(request: &HttpRequest, extensions: &mut rmcp::model::Extensions) {
    let authcode = request.headers()
        .get(AUTHCODE_HEADER)
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let pid = request.headers()
        .get(PID_HEADER)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<i64>().ok());

    match (authcode, pid) {
        (Some(authcode), Some(pid)) => {
            logger(format!("MCP request: credentials received ({} pid={})", mask_authcode(authcode), pid));
            extensions.insert(McpAuth { authcode: authcode.to_string(), pid });
        }
        (Some(authcode), None) => elogger(format!(
            "MCP request: {} received ({}) but {} missing or not an integer",
            AUTHCODE_HEADER, mask_authcode(authcode), PID_HEADER
        )),
        (None, Some(pid)) => elogger(format!(
            "MCP request: {} received (pid={}) but {} missing",
            PID_HEADER, pid, AUTHCODE_HEADER
        )),
        (None, None) => logger(format!(
            "MCP request: neither {} nor {} present (fine for initialize/tools list)",
            AUTHCODE_HEADER, PID_HEADER
        ))
    }
}


/// Builds the Streamable HTTP transport carrying [`RustopusMcp`].
///
/// Must be called **outside** `HttpServer::new()`'s closure so every actix
/// worker shares one `LocalSessionManager`; building it per worker would make a
/// session created on one worker unknown to the next.
pub fn build_service() -> StreamableHttpService<RustopusMcp, LocalSessionManager> {
    StreamableHttpService::builder()
        .service_factory(Arc::new(|| Ok(RustopusMcp::new())))
        .session_manager(Arc::new(LocalSessionManager::default()))
        .stateful_mode(true)
        .sse_keep_alive(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
        .on_request_fn(extract_auth)
        .build()
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limits_are_clamped_server_side() {
        assert_eq!(clamp(None, 20, 50), 20);
        assert_eq!(clamp(Some(0), 20, 50), 20);
        assert_eq!(clamp(Some(5), 20, 50), 5);
        // A caller asking for the whole catalog still gets one page.
        assert_eq!(clamp(Some(100_000), 20, 50), 50);
    }

    #[test]
    fn filter_description_mentions_only_active_filters() {
        let args = SearchProductsArgs {
            query: "pen".into(),
            brand: Some("Orink".into()),
            category: None,
            main_category: None,
            limit: None
        };
        assert_eq!(describe_filters(&args), " with brand 'Orink'");

        let bare = SearchProductsArgs {
            query: "pen".into(),
            brand: None,
            category: None,
            main_category: None,
            limit: None
        };
        assert_eq!(describe_filters(&bare), "");
    }
}
