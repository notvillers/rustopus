//! The MCP service itself: tool definitions plus the Streamable HTTP transport
//! that carries them.
//!
//! Caller identity arrives as HTTP headers (`X-Authcode`, `X-Pid`), is lifted
//! into rmcp's request extensions by the `on_request` hook below, and is read
//! back inside each tool through `RequestContext::extensions`. **No read tool
//! takes a partner argument** — the pid is fixed per user by their connector
//! config, so a model cannot ask for another partner's prices. `submit_order` is
//! the one exception: booking to another partner is a real case, and Octopus
//! decides whether this authcode may do it.
//!
//! Five catalog tools, three invoice tools (unless `[mcp] invoices_enabled` is
//! off), plus `submit_order` when `[mcp] orders_enabled` is on — shaped by what
//! people ask rather than by SOAP operation. Every
//! tool definition costs context on every request, so there is deliberately no
//! tool-per-endpoint mapping, and deliberately **no sync tool**: refresh is the
//! precache job's business, and a model-triggered 28-second sync is exactly what
//! this design exists to prevent.

use std::sync::Arc;
use std::time::Duration;

use actix_web::{HttpMessage, HttpRequest};
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
            export,
            index::{CatalogSnapshot, SearchFilters, fold},
            invoices::{self, Fetch, Question, Scope},
            mask_authcode,
            orders::{self, SubmitOrderArgs, lenient_flag}
        },
        config::get_mcp_settings,
        soap_config::get_default_url
    }
};

/// How often the transport sends SSE keep-alive pings.
const SSE_KEEP_ALIVE_SECS: u64 = 30;

/// Results returned when a search does not ask for a specific count.
const DEFAULT_SEARCH_LIMIT: usize = 20;

/// Hard ceiling on results per page, applied server-side whatever the caller
/// asks for. A larger page buries the answer and burns the context window; to
/// get past it, page with `offset` or — for anything bulk — use
/// `export_products`, which does not go through the context at all.
const MAX_SEARCH_LIMIT: usize = 100;

/// Categories listed per kind before the list is truncated.
const DEFAULT_CATEGORY_LIMIT: usize = 40;

/// Near matches offered when an article number is not found.
const DID_YOU_MEAN_LIMIT: usize = 5;


/// The caller's masked identity for a tool-call log line. `extract_auth` already
/// logs (or complains about) the credentials on every HTTP request that reaches
/// the transport; this is the per-tool-call counterpart, so a log line also says
/// *which* tool ran and with *what* outcome.
fn caller_identity(context: &RequestContext<RoleServer>) -> String {
    context.extensions.get::<McpAuth>()
        .map(McpAuth::masked)
        .unwrap_or_else(|| "no-auth".to_string())
}


/// Deserializes a count that may arrive as a number *or* as a numeric string.
///
/// The published schema says `integer`, and that stays correct — this only makes
/// parsing tolerant. Bridges between a client and this server (`mcp-remote`, and
/// anything that reserializes arguments) can lose the schema's type information,
/// after which a caller sends `"20"` and gets
/// `invalid type: string "20", expected u32` — a message that reads like a server
/// bug and blocks the call entirely. Accepting both costs nothing and is the
/// difference between a working tool and a dead end for the person asking.
pub(crate) fn lenient_count<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>
{
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Count {
        Number(u32),
        Text(String)
    }

    match Option::<Count>::deserialize(deserializer)? {
        None => Ok(None),
        Some(Count::Number(value)) => Ok(Some(value)),
        Some(Count::Text(text)) => {
            let trimmed = text.trim();
            if trimmed.is_empty() {
                return Ok(None)
            }
            trimmed.parse::<u32>()
                .map(Some)
                .map_err(|_| serde::de::Error::custom(format!("'{}' is not a whole number", trimmed)))
        }
    }
}


McpToolArgs! {
    pub struct SearchProductsArgs {
        /// Words to search for. Accent- and case-insensitive: `szovegkiemelo`
        /// matches `Szövegkiemelő`. Every word must match somewhere. A barcode
        /// (EAN) or a manufacturer part number can be pasted here directly.
        pub query: String,
        /// Restrict to a brand / manufacturer, e.g. "Orink".
        pub brand: Option<String>,
        /// Restrict to a product group, by code or name.
        pub category: Option<String>,
        /// Restrict to a main group, by code or name.
        pub main_category: Option<String>,
        /// How many results to return. Clamped to 100.
        #[serde(default, deserialize_with = "lenient_count")]
        pub limit: Option<u32>,
        /// How many results to skip, for walking a large result set. Use the
        /// `next_offset` from the previous response. To export a whole category
        /// or the whole catalog, use `export_products` instead — paging a
        /// catalog of 24,000 products through chat is not workable.
        #[serde(default, deserialize_with = "lenient_count")]
        pub offset: Option<u32>
    }

    pub struct GetProductArgs {
        /// Article number (`cikkszam`). A barcode (EAN), a manufacturer part
        /// number or the internal record id also resolves.
        pub no: String
    }

    pub struct ListCategoriesArgs {
        /// Which grouping to list: `brands`, `main_categories`, `categories`, or
        /// `all` (the default).
        pub kind: Option<String>,
        /// How many entries per grouping. Clamped to 200.
        #[serde(default, deserialize_with = "lenient_count")]
        pub limit: Option<u32>
    }

    pub struct ExportProductsArgs {
        /// `xlsx` (default, for Excel) or `csv` (semicolon-delimited, as the
        /// REST endpoints produce).
        pub format: Option<String>,
        /// Optional words to narrow the export. Omit to export everything.
        pub query: Option<String>,
        /// Restrict to a brand / manufacturer.
        pub brand: Option<String>,
        /// Restrict to a product group, by code or name.
        pub category: Option<String>,
        /// Restrict to a main group, by code or name.
        pub main_category: Option<String>
    }

    pub struct ListInvoicesArgs {
        /// First invoice date, `YYYY-MM-DD`. Defaults to 90 days before `to_date`.
        pub from_date: Option<String>,
        /// Last invoice date, `YYYY-MM-DD`. Defaults to today. At most two years
        /// after `from_date`.
        pub to_date: Option<String>,
        /// Every invoice with an outstanding balance, whatever its age ("what do
        /// I owe"). The dates, if given, then only narrow that list.
        #[serde(default, deserialize_with = "lenient_flag")]
        pub unpaid_only: Option<bool>,
        /// Only unpaid invoices past their payment deadline. Implies `unpaid_only`.
        #[serde(default, deserialize_with = "lenient_flag")]
        pub overdue_only: Option<bool>,
        /// Words to look for: invoice number, the partner's own order number, or
        /// an article number or product name on any line. Accent-insensitive.
        pub query: Option<String>,
        /// How many invoices to return. Clamped to 100.
        #[serde(default, deserialize_with = "lenient_count")]
        pub limit: Option<u32>,
        /// How many to skip; use `next_offset` from the previous response. For
        /// more than a few pages use `export_invoices`.
        #[serde(default, deserialize_with = "lenient_count")]
        pub offset: Option<u32>
    }

    pub struct GetInvoiceArgs {
        /// The invoice number, as shown by `list_invoices`.
        pub no: String,
        /// The invoice's date, `YYYY-MM-DD`. Needed only when the invoice was not
        /// in a `list_invoices` result of the last few minutes.
        pub date: Option<String>
    }

    pub struct ExportInvoicesArgs {
        /// `xlsx` (default, for Excel) or `csv` (semicolon-delimited).
        pub format: Option<String>,
        /// `lines` (default): one row per invoice line, with its invoice's number
        /// and dates. `invoices`: one row per invoice, with its totals.
        pub detail: Option<String>,
        /// First invoice date, `YYYY-MM-DD`. Defaults to 90 days before `to_date`.
        pub from_date: Option<String>,
        /// Last invoice date, `YYYY-MM-DD`. Defaults to today. At most two years
        /// after `from_date`.
        pub to_date: Option<String>,
        /// Every invoice with an outstanding balance, whatever its age.
        #[serde(default, deserialize_with = "lenient_flag")]
        pub unpaid_only: Option<bool>,
        /// Only unpaid invoices past their payment deadline.
        #[serde(default, deserialize_with = "lenient_flag")]
        pub overdue_only: Option<bool>,
        /// Words to narrow the export, as in `list_invoices`.
        pub query: Option<String>
    }
}


/// The MCP service. One instance is built per session by the transport's service
/// factory; all shared state lives in the process-wide cache rather than in this
/// struct, so constructing it is cheap.
///
/// The router is held per instance (and `#[tool_handler]` pointed at it) so a
/// tool can be left out of it: `submit_order` is removed when orders are off,
/// which keeps it out of `tools/list` rather than listing a tool that only
/// ever refuses.
#[derive(Clone)]
pub struct RustopusMcp {
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
        let mut tool_router = Self::tool_router();
        let settings = get_mcp_settings();
        if !settings.orders_enabled() {
            tool_router.remove_route("submit_order");
        }
        if !settings.invoices_enabled() {
            for name in ["list_invoices", "get_invoice", "export_invoices"] {
                tool_router.remove_route(name);
            }
        }
        Self { tool_router }
    }

    /// Search the catalog. The workhorse: name, article number, barcode, brand
    /// and manufacturer part number in one ranked pass, with the caller's own
    /// price and stock inline so a follow-up call is not needed to answer
    /// "how much".
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "Search Orink products by name, article number, barcode (EAN), brand or \
        manufacturer part number. Accent-insensitive. Returns this partner's own price and current \
        stock inline.")]
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
        let offset = args.offset.unwrap_or(0) as usize;
        let filters = SearchFilters {
            brand: args.brand.as_deref().map(fold),
            category: args.category.as_deref().map(fold),
            main_category: args.main_category.as_deref().map(fold)
        };

        let outcome = snapshot.search_page(&args.query, &filters, offset, limit);

        logger(format!(
            "MCP tool 'search_products' by {}: query='{}' brand={:?} category={:?} main_category={:?} offset={} limit={} -> matched={} returned={}",
            caller_identity(&context), args.query, args.brand, args.category, args.main_category,
            offset, limit, outcome.matched, outcome.results.len()
        ));

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

        let next_offset = offset + outcome.results.len();
        let has_more = next_offset < outcome.matched;

        let mut payload = json!({
            "matched": outcome.matched,
            "returned": outcome.results.len(),
            "offset": offset,
            "has_more": has_more,
            "catalog_age_seconds": snapshot.age_secs(),
            "results": outcome.results
        });

        if has_more && let Some(object) = payload.as_object_mut() {
            object.insert("next_offset".into(), json!(next_offset));
            // Steer the model away from paging its way through thousands of
            // rows: that is what export_products exists for.
            object.insert("hint".into(), json!(format!(
                "{} more match. Call again with offset={} for the next page, or use \
                 export_products to get all {} rows as a spreadsheet in one step.",
                outcome.matched - next_offset, next_offset, outcome.matched
            )));
        }

        Ok(json_result(payload))
    }

    /// Bulk export. The answer to "I need this in Excel", and the reason
    /// `search_products` does not need to page through 24,000 products: the rows
    /// go to a file, never through the model's context.
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "Export Orink products to an Excel (.xlsx) or CSV file and return a download link. \
        Use this whenever more than a page of products is needed — a whole category, a whole brand, or the \
        entire catalog. Filters are optional; with none, every product is exported. The rows are written to \
        a file, so this works for the full catalog and costs no context.")]
    async fn export_products(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ExportProductsArgs>
    ) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        let format = match export::Format::parse(args.format.as_deref()) {
            Some(format) => format,
            None => return Ok(CallToolResult::error(vec![Content::text(
                "format must be 'xlsx' or 'csv'."
            )]))
        };

        let filters = SearchFilters {
            brand: args.brand.as_deref().map(fold),
            category: args.category.as_deref().map(fold),
            main_category: args.main_category.as_deref().map(fold)
        };

        // An empty query means "everything that passes the filters", which is the
        // common case here — unlike search, where a query is required.
        let query = args.query.clone().unwrap_or_default();
        let count = snapshot.count_matching(&query, &filters);

        if count == 0 {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Nothing to export{}{}. Use list_categories to see the available brands and groups.",
                args.query.as_deref().map(|q| format!(" for '{}'", q)).unwrap_or_default(),
                describe_export_filters(&args)
            ))]))
        }

        // Writing a 24,000-row workbook is CPU-bound; keep it off the async
        // workers that also serve the REST endpoints. The snapshot goes in as an
        // `Arc` and the rows are selected inside — copying them out first would
        // put a second catalog in memory on a host that has little to spare.
        let for_export = Arc::clone(&snapshot);
        let filters_for_export = filters.clone();
        let built = actix_web::web::block(move || {
            let rows = for_export.select(&query, &filters_for_export);
            export::write(rows, format)
        }).await;

        let prepared = match built {
            Ok(Ok(prepared)) => prepared,
            Ok(Err(error)) => {
                elogger(format!("MCP export failed for {}: {}", caller_identity(&context), error));
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "The export could not be written: {}", error
                ))]))
            }
            Err(error) => {
                elogger(format!("MCP export failed to run for {}: {}", caller_identity(&context), error));
                return Ok(CallToolResult::error(vec![Content::text(
                    "The export could not be started. This is a server-side problem."
                )]))
            }
        };

        logger(format!(
            "MCP tool 'export_products' by {}: format={} rows={} -> {} ({:.1} MB)",
            caller_identity(&context), format.extension(), count,
            prepared.file_name, prepared.bytes as f64 / 1_048_576.0
        ));

        Ok(json_result(json!({
            "rows": count,
            "format": format.extension(),
            "file_name": prepared.file_name,
            "size_bytes": prepared.bytes,
            "download_url": prepared.url,
            "expires_in_seconds": export::ttl_secs(),
            "catalog_age_seconds": snapshot.age_secs(),
            "note": "Give the user this download_url. The link expires, needs no login, \
                     and the file holds this partner's own prices — do not post it anywhere public."
        })))
    }

    /// Everything known about one product, for when a search result needs
    /// following up.
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "Full master data for one Orink product by article number, barcode (EAN) or \
        manufacturer part number, including current stock. `price` is what this partner pays.")]
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
            Some(product) => {
                logger(format!(
                    "MCP tool 'get_product' by {}: no='{}' -> found (available={})",
                    caller_identity(&context), args.no, product.available
                ));
                let mut payload = json!({
                    "catalog_age_seconds": snapshot.age_secs(),
                    "product": product
                });
                // Split the raw, main-EAN-first `barcodes` array into an explicit
                // primary/secondary pair, matching `search_products`' shape — a
                // caller should never have to infer "primary" from array order.
                if let Some(product_object) = payload["product"].as_object_mut() {
                    product_object.remove("barcodes");
                    if let Some(primary) = product.barcodes.first() {
                        product_object.insert("primary_barcode".into(), json!(primary));
                    }
                    let secondary: Vec<&String> = product.barcodes.iter().skip(1).collect();
                    if !secondary.is_empty() {
                        product_object.insert("secondary_barcodes".into(), json!(secondary));
                    }
                }
                // A withheld product is served rather than hidden — the caller
                // asked for it by article number — but the record alone reads
                // like any other, so say plainly that it is not on offer.
                if !product.available && let Some(object) = payload.as_object_mut() {
                    object.insert(
                        "note".into(),
                        json!(format!(
                            "'{}' exists in the catalog but is not currently published for sale, \
                            so it does not appear in search or export results.",
                            product.no
                        ))
                    );
                }
                Ok(json_result(payload))
            }
            None => {
                logger(format!(
                    "MCP tool 'get_product' by {}: no='{}' -> not found",
                    caller_identity(&context), args.no
                ));
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
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "List the brands, main groups and product groups in the Orink catalog, with product counts.")]
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

        logger(format!(
            "MCP tool 'list_categories' by {}: kind='{}' limit={} -> brands={} main_categories={} categories={}",
            caller_identity(&context), kind, limit,
            categories.brands.len(), categories.main_categories.len(), categories.categories.len()
        ));

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
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "How many products the cached Orink catalog holds and how old the data is.")]
    async fn catalog_status(&self, context: RequestContext<RoleServer>) -> Result<CallToolResult, McpError> {
        let snapshot = match self.snapshot(&context).await {
            Ok(snapshot) => snapshot,
            Err(result) => return Ok(result)
        };

        logger(format!(
            "MCP tool 'catalog_status' by {}: products={} hidden={} age_seconds={}",
            caller_identity(&context), snapshot.available_count(), snapshot.hidden_count(), snapshot.age_secs()
        ));

        // `products` counts what the other tools can actually return, so the
        // number a caller is told matches the number they can reach. The rows
        // the ERP withholds are reported separately rather than folded in.
        Ok(json_result(json!({
            "products": snapshot.available_count(),
            "hidden_products": snapshot.hidden_count(),
            "fetched_at": snapshot.fetched_at.to_rfc3339(),
            "age_seconds": snapshot.age_secs(),
            "priced_products": snapshot.products.iter()
                .filter(|p| p.available && p.price.is_some())
                .count(),
            "note": "Prices and stock are specific to the partner id configured on this connector. \
                `hidden_products` are held in the catalog but not published for sale, so they do not \
                appear in search or export results."
        })))
    }

    /// Places a real order in Octopus and waits for its answer. The one tool
    /// that changes anything; see `service/mcp/orders.rs` for why it is shaped
    /// the way it is.
    #[tool(
        annotations(title = "Submit order", read_only_hint = false, destructive_hint = true, idempotent_hint = false, open_world_hint = true),
        description = "Place a REAL order with Orink and wait for Octopus's answer. Only call this when the \
            user has clearly asked to place the order and has seen what will be ordered (articles, \
            quantities, delivery). Use exact article numbers from search_products or get_product. The \
            answer says, line by line, what was recorded and what was not — relay that, including any \
            line that was NOT recorded. If the answer says the outcome is unknown, do NOT retry: tell the \
            user to check the order in Orink's system first."
    )]
    async fn submit_order(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<SubmitOrderArgs>
    ) -> Result<CallToolResult, McpError> {
        let Some(auth) = context.extensions.get::<McpAuth>().cloned() else {
            return Ok(CallToolResult::error(vec![Content::text(
                "This connector is not sending credentials, so no order can be placed."
            )]))
        };
        if !auth.can_order {
            logger(format!("MCP tool 'submit_order' by {}: refused — connector is read-only", auth.masked()));
            return Ok(CallToolResult::error(vec![Content::text(
                "This connector is read-only: placing orders has not been enabled for it. Nothing was \
                 sent. Whoever runs the Rustopus server can allow orders for this connector."
            )]))
        }
        let Some(url) = get_default_url() else {
            elogger("MCP: no default SOAP url configured (soap.json missing or empty)");
            return Ok(CallToolResult::error(vec![Content::text(
                "The Rustopus server has no Octopus url configured, so no order can be placed. Nothing \
                 was sent. This is a server-side configuration problem."
            )]))
        };

        Ok(orders::run(args, &auth, &url).await)
    }

    /// Invoices for a period, or every unpaid one. The fetch runs in the
    /// background (see `service/mcp/invoices.rs`): a busy partner's year takes
    /// Octopus minutes, far past any client's patience, so this answers
    /// "in progress" rather than letting the call time out.
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "List this partner's \
        Orink invoices for a period (default: the last 90 days), or every unpaid / overdue invoice, with \
        totals per currency. Returns invoice headers; use get_invoice for the lines. A long period can take \
        Octopus several minutes: if the answer has status \"in_progress\", tell the user it is being fetched \
        and call this tool again with exactly the same arguments — the fetch keeps running in between. If \
        \"partial\" is true, some months could not be read: tell the user which (missing_months) and that the \
        listed invoices and totals leave them out. Amounts are rounded to 2 decimals; an invoice's net_total \
        and gross_total are the ERP's own figures (rounded by the ERP, after any advance payment deducted), so \
        they can differ from the sum of its lines by a few fillér, and can be 0 for a fully prepaid invoice.")]
    async fn list_invoices(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ListInvoicesArgs>
    ) -> Result<CallToolResult, McpError> {
        let (auth, url) = match credentials(&context) {
            Ok(credentials) => credentials,
            Err(result) => return Ok(result)
        };
        let today = invoices::today();
        let question = match invoices::resolve(
            args.from_date.as_deref(), args.to_date.as_deref(),
            args.unpaid_only.unwrap_or(false), args.overdue_only.unwrap_or(false),
            args.query.as_deref(), today
        ) {
            Ok(question) => question,
            Err(message) => return Ok(CallToolResult::error(vec![Content::text(message)]))
        };

        let set = match invoices::fetch(&auth, &url, question.scope).await {
            Fetch::Ready(set) => set,
            other => return Ok(not_ready("list_invoices", &auth, other))
        };

        let limit = clamp(args.limit, DEFAULT_SEARCH_LIMIT, MAX_SEARCH_LIMIT);
        let offset = args.offset.unwrap_or(0) as usize;
        let selected = set.select(&question.filter, today);
        let page: Vec<serde_json::Value> = selected.iter()
            .skip(offset)
            .take(limit)
            .map(|invoice| invoice.summary(today))
            .collect();

        logger(format!(
            "MCP tool 'list_invoices' by {}: {} filter={:?} offset={} limit={} -> fetched={} matched={} returned={}",
            auth.masked(), question.scope.describe(), question.filter, offset, limit,
            set.invoices.len(), selected.len(), page.len()
        ));

        let next_offset = offset + page.len();
        let has_more = next_offset < selected.len();
        let mut payload = json!({
            "status": "ready",
            "covers": describe_question(&question),
            "matched": selected.len(),
            "returned": page.len(),
            "offset": offset,
            "has_more": has_more,
            "totals_by_currency": invoices::totals(&selected, today),
            "fetched_seconds_ago": set.age_secs(),
            "invoices": page
        });
        if let Some(object) = payload.as_object_mut() {
            add_partial(object, &set);
            if has_more {
                object.insert("next_offset".into(), json!(next_offset));
                object.insert("hint".into(), json!(format!(
                    "{} more. Call again with offset={} (it is instant now), or use export_invoices \
                     with the same arguments for all {} as a spreadsheet.",
                    selected.len() - next_offset, next_offset, selected.len()
                )));
            }
            if selected.is_empty() {
                object.insert("note".into(), json!(format!(
                    "No invoices match{}.",
                    if question.filter.query.is_some() || question.filter.overdue_only { " these filters" } else { " in this period" }
                )));
            }
        }
        Ok(json_result(payload))
    }

    /// One invoice with its lines.
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "One Orink invoice \
        with all of its lines, by invoice number. Instant for an invoice listed by list_invoices in the last \
        few minutes; otherwise pass its date too, so only that day is fetched from Octopus. Amounts are rounded \
        to 2 decimals; the header's net_total and gross_total are the ERP's own rounded figures after any \
        advance payment deducted, so they need not equal the sum of the lines.")]
    async fn get_invoice(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<GetInvoiceArgs>
    ) -> Result<CallToolResult, McpError> {
        let (auth, url) = match credentials(&context) {
            Ok(credentials) => credentials,
            Err(result) => return Ok(result)
        };
        let today = invoices::today();
        let reference = args.no.trim();
        if reference.is_empty() {
            return Ok(CallToolResult::error(vec![Content::text("no must be an invoice number.")]))
        }

        if let Some((invoice, set)) = invoices::find_kept(&auth, reference) {
            logger(format!("MCP tool 'get_invoice' by {}: no='{}' -> found in a kept result", auth.masked(), reference));
            return Ok(invoice_result(&invoice, today, set.age_secs()))
        }

        let Some(date) = args.date.as_deref() else {
            logger(format!("MCP tool 'get_invoice' by {}: no='{}' -> not kept, no date given", auth.masked(), reference));
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Invoice '{}' is not among the invoices fetched in the last few minutes. Call get_invoice \
                 again with its date (date=YYYY-MM-DD), or list_invoices for the period that contains it.",
                reference
            ))]))
        };
        let day = match invoices::parse_date(date, "date") {
            Ok(day) => day,
            Err(message) => return Ok(CallToolResult::error(vec![Content::text(message)]))
        };

        let set = match invoices::fetch(&auth, &url, Scope::Period { from: day, to: day }).await {
            Fetch::Ready(set) => set,
            other => return Ok(not_ready("get_invoice", &auth, other))
        };
        match set.find(reference) {
            Some(invoice) => {
                logger(format!("MCP tool 'get_invoice' by {}: no='{}' date={} -> found", auth.masked(), reference, day));
                Ok(invoice_result(invoice, today, set.age_secs()))
            }
            None => {
                logger(format!("MCP tool 'get_invoice' by {}: no='{}' date={} -> not found", auth.masked(), reference, day));
                let others: Vec<&str> = set.invoices.iter().filter_map(|invoice| invoice.no.as_deref()).take(10).collect();
                Ok(CallToolResult::error(vec![Content::text(if others.is_empty() {
                    format!("This partner has no invoice dated {}.", day)
                } else {
                    format!("No invoice '{}' dated {}. Invoices on that day: {}.", reference, day, others.join(", "))
                })]))
            }
        }
    }

    /// Invoices to a spreadsheet, for anything past a page or two — a year of
    /// lines cannot cross the context any more than the catalog can.
    #[tool(annotations(read_only_hint = true, open_world_hint = false), description = "Export this partner's \
        Orink invoices to an Excel (.xlsx) or CSV file and return a download link — one row per invoice line \
        (default) or per invoice. Takes the same period and filters as list_invoices. Use it for anything \
        larger than a page, e.g. a whole year. Like list_invoices, a long period can answer \"in_progress\": \
        call again with the same arguments to collect the file.")]
    async fn export_invoices(
        &self,
        context: RequestContext<RoleServer>,
        Parameters(args): Parameters<ExportInvoicesArgs>
    ) -> Result<CallToolResult, McpError> {
        let (auth, url) = match credentials(&context) {
            Ok(credentials) => credentials,
            Err(result) => return Ok(result)
        };
        let Some(format) = export::Format::parse(args.format.as_deref()) else {
            return Ok(CallToolResult::error(vec![Content::text("format must be 'xlsx' or 'csv'.")]))
        };
        let Some(detail) = invoices::Detail::parse(args.detail.as_deref()) else {
            return Ok(CallToolResult::error(vec![Content::text("detail must be 'lines' or 'invoices'.")]))
        };
        let question = match invoices::resolve(
            args.from_date.as_deref(), args.to_date.as_deref(),
            args.unpaid_only.unwrap_or(false), args.overdue_only.unwrap_or(false),
            args.query.as_deref(), invoices::today()
        ) {
            Ok(question) => question,
            Err(message) => return Ok(CallToolResult::error(vec![Content::text(message)]))
        };

        let set = match invoices::fetch(&auth, &url, question.scope).await {
            Fetch::Ready(set) => set,
            other => return Ok(not_ready("export_invoices", &auth, other))
        };

        // The workbook is written from the kept result behind its `Arc`, off the
        // async workers, exactly as the catalog export is.
        let for_export = Arc::clone(&set);
        let filter = question.filter.clone();
        let built = actix_web::web::block(move || invoices::write_export(&for_export, &filter, detail, format)).await;
        let (prepared, rows) = match built {
            Ok(Ok(written)) => written,
            Ok(Err(error)) => {
                elogger(format!("MCP invoice export failed for {}: {}", auth.masked(), error));
                return Ok(CallToolResult::error(vec![Content::text(format!("The export could not be written: {}", error))]))
            }
            Err(error) => {
                elogger(format!("MCP invoice export failed to run for {}: {}", auth.masked(), error));
                return Ok(CallToolResult::error(vec![Content::text(
                    "The export could not be started. This is a server-side problem."
                )]))
            }
        };

        logger(format!(
            "MCP tool 'export_invoices' by {}: {} detail={} format={} rows={} -> {} ({:.1} MB)",
            auth.masked(), question.scope.describe(), detail.name(), format.extension(), rows,
            prepared.file_name, prepared.bytes as f64 / 1_048_576.0
        ));

        let mut payload = json!({
            "status": "ready",
            "covers": describe_question(&question),
            "rows": rows,
            "detail": detail.name(),
            "format": format.extension(),
            "file_name": prepared.file_name,
            "size_bytes": prepared.bytes,
            "download_url": prepared.url,
            "expires_in_seconds": export::ttl_secs(),
            "fetched_seconds_ago": set.age_secs(),
            "note": "Give the user this download_url. The link expires, needs no login, and the file holds \
                     this partner's invoices — do not post it anywhere public."
        });
        if let Some(object) = payload.as_object_mut() {
            add_partial(object, &set);
        }
        Ok(json_result(payload))
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
        let (auth, url) = credentials(context)?;

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

#[tool_handler(router = self.tool_router)]
impl ServerHandler for RustopusMcp {
    fn get_info(&self) -> ServerInfo {
        let mut instructions = String::from(
            "Orink product data from the Octopus 8 ERP. Stock and pricing are specific to the \
             partner id configured on this connector: `price` is what this partner actually \
             pays, not a list or retail price, so it can be quoted as-is. Data comes from a \
             periodically refreshed snapshot — call catalog_status when the freshness of an \
             answer matters."
        );
        if get_mcp_settings().invoices_enabled() {
            instructions.push_str(
                " Invoices come live from the ERP, not from the snapshot, and a long period can take \
                 minutes: when an invoice tool answers status \"in_progress\", tell the user and call it \
                 again with the same arguments rather than giving up or changing them."
            );
        }
        if get_mcp_settings().orders_enabled() {
            instructions.push_str(
                " submit_order places real orders: confirm the lines with the user before calling it, \
                 never call it twice for one request, and relay its per-line result."
            );
        }
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("rustopus-mcp", env!("CARGO_PKG_VERSION")))
            .with_instructions(instructions)
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


/// The caller's identity and the Octopus url, or a ready-made error result.
/// Shared by every tool that reaches Octopus, so a missing header reads the
/// same everywhere.
///
/// The large `Err` is the tool's own answer, built once and returned as-is —
/// boxing it would only add an allocation on a path that ends the call.
#[allow(clippy::result_large_err)]
fn credentials(context: &RequestContext<RoleServer>) -> Result<(McpAuth, String), CallToolResult> {
    let Some(auth) = context.extensions.get::<McpAuth>().cloned() else {
        return Err(CallToolResult::error(vec![Content::text(format!(
            "This connector is not sending credentials. Both the {} and {} request headers \
             must be configured on it before any data can be read.",
            AUTHCODE_HEADER, PID_HEADER
        ))]))
    };

    let Some(url) = get_default_url() else {
        // Server-side misconfiguration, not the caller's problem — but they
        // still need to be told why nothing works.
        elogger("MCP: no default SOAP url configured (soap.json missing or empty)");
        return Err(CallToolResult::error(vec![Content::text(
            "The Rustopus server has no Octopus url configured, so nothing can be read from the ERP. \
             This is a server-side configuration problem."
        )]))
    };

    Ok((auth, url))
}


/// What an invoice answer covers, in words the model can repeat to the user.
fn describe_question(question: &Question) -> String {
    let mut text = question.scope.describe();
    if question.filter.overdue_only {
        text = "overdue unpaid invoices".to_string();
    }
    match (question.filter.from, question.filter.to) {
        (Some(from), Some(to)) => text.push_str(&format!(", dated {} to {}", from, to)),
        (Some(from), None) => text.push_str(&format!(", dated from {}", from)),
        (None, Some(to)) => text.push_str(&format!(", dated up to {}", to)),
        (None, None) => {}
    }
    if let Some(query) = &question.filter.query {
        text.push_str(&format!(", matching '{}'", query));
    }
    text
}


/// The answer for an invoice fetch that has no result yet. "In progress" is a
/// success, not an error: nothing went wrong, and an error result invites the
/// model to give up or to change the arguments — which would start a second
/// fetch instead of collecting the first.
fn not_ready(tool: &str, auth: &McpAuth, fetch: Fetch) -> CallToolResult {
    match fetch {
        Fetch::Ready(_) => CallToolResult::error(vec![Content::text("Internal error: a ready result was not used.")]),
        Fetch::Pending(pending) => {
            let progress = if pending.queued {
                "waiting for an earlier invoice request of this partner to finish".to_string()
            } else if pending.total > 1 {
                format!("{} of {} months fetched, {} invoices so far", pending.done, pending.total, pending.invoices_so_far)
            } else {
                "waiting for Octopus".to_string()
            };
            logger(format!(
                "MCP tool '{}' by {}: {} still running after {}s ({})",
                tool, auth.masked(), pending.scope.describe(), pending.elapsed_secs, progress
            ));
            json_result(json!({
                "status": "in_progress",
                "covers": pending.scope.describe(),
                "progress": progress,
                "elapsed_seconds": pending.elapsed_secs,
                "note": format!(
                    "Octopus is still assembling these invoices — a long period for a busy partner can take \
                     several minutes. Tell the user it is being fetched, then call {} again with exactly the \
                     same arguments to collect the result. The fetch keeps running in between; changing the \
                     arguments would start a different one.", tool
                )
            }))
        }
        Fetch::Failed(message) => {
            logger(format!("MCP tool '{}' by {}: failed — {}", tool, auth.masked(), message));
            CallToolResult::error(vec![Content::text(format!(
                "Octopus did not return the invoices: {}. Calling again with the same arguments retries.",
                message
            ))])
        }
        Fetch::Busy(message) => {
            logger(format!("MCP tool '{}' by {}: refused — too many invoice fetches running", tool, auth.masked()));
            CallToolResult::error(vec![Content::text(message)])
        }
    }
}


/// Marks a result that lacks some months, and names them. The model has to say
/// so: totals over a partial period read as the whole period's otherwise.
fn add_partial(object: &mut serde_json::Map<String, serde_json::Value>, set: &invoices::InvoiceSet) {
    if !set.is_partial() {
        return
    }
    object.insert("partial".into(), json!(true));
    object.insert("missing_months".into(), json!(set.missing));
    object.insert("partial_note".into(), json!(
        "Octopus did not answer for the months in missing_months, so their invoices are not included \
         and the totals leave them out. Tell the user. Asking again in a minute retries only those months."
    ));
}

/// One invoice with its lines, plus whether it is overdue.
fn invoice_result(invoice: &invoices::Invoice, today: chrono::NaiveDate, age_secs: i64) -> CallToolResult {
    let mut payload = json!({ "fetched_seconds_ago": age_secs, "invoice": invoice });
    if invoice.is_overdue(today) && let Some(object) = payload["invoice"].as_object_mut() {
        object.insert("overdue".into(), json!(true));
    }
    json_result(payload)
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
    render_filters(args.brand.as_deref(), args.category.as_deref(), args.main_category.as_deref())
}

/// The same, for the export tool's own argument shape.
fn describe_export_filters(args: &ExportProductsArgs) -> String {
    render_filters(args.brand.as_deref(), args.category.as_deref(), args.main_category.as_deref())
}

fn render_filters(brand: Option<&str>, category: Option<&str>, main_category: Option<&str>) -> String {
    let mut parts = Vec::new();
    if let Some(brand) = brand {
        parts.push(format!("brand '{}'", brand));
    }
    if let Some(category) = category {
        parts.push(format!("category '{}'", category));
    }
    if let Some(main_category) = main_category {
        parts.push(format!("main category '{}'", main_category));
    }
    if parts.is_empty() {
        return String::new()
    }
    format!(" with {}", parts.join(" and "))
}


/// Hands the caller's identity to the MCP layer as a typed extension.
///
/// Two sources, in order. **The request extensions first**: when OAuth is on,
/// `oauth::guard` has already resolved a bearer token to a grant and put an
/// [`McpAuth`] there — the middleware→extensions propagation `rmcp-actix-web`
/// documents for exactly this case. **The headers second**, which is how every
/// caller that can set them still works.
///
/// A request missing both is not rejected here: the transport also carries
/// `initialize` and `tools/list`, which need no credentials. Tools that do need
/// them return an actionable MCP error instead, which the model can act on,
/// unlike a transport-level rejection. (With OAuth on, such a request never
/// reaches this function at all — the guard answers it with a `401`.)
fn extract_auth(request: &HttpRequest, extensions: &mut rmcp::model::Extensions) {
    if let Some(auth) = request.extensions().get::<McpAuth>().cloned() {
        logger(format!("MCP request: credentials resolved from an access token ({})", auth.masked()));
        extensions.insert(auth);
        return
    }

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
            extensions.insert(McpAuth { authcode: authcode.to_string(), pid, can_order: true });
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
            limit: None,
            offset: None
        };
        assert_eq!(describe_filters(&args), " with brand 'Orink'");

        let bare = SearchProductsArgs {
            query: "pen".into(),
            brand: None,
            category: None,
            main_category: None,
            limit: None,
            offset: None
        };
        assert_eq!(describe_filters(&bare), "");
    }
}
