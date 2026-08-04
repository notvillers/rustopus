//! Catalog snapshot: one in-memory, searchable view of the product catalog for
//! a single `(authcode, pid, url)` combination.
//!
//! Nothing here parses SOAP. The snapshot is built by calling the **existing**
//! dispatch in `service/get_data.rs` — the same `RequestGet` variants the REST
//! routes use — and merging the English models it returns. When Octopus changes,
//! the fix lands once in `forms/` and this module follows for free.
//!
//! Two sizing rules, both measured in the TypeScript prototype this replaces:
//!
//! * **HTML is stripped from descriptions at ingest.** It cut the prototype's
//!   heap from 356 MB to 108 MB, and nothing downstream renders HTML.
//! * **Empty fields become `None` rather than empty strings.** In Rust this
//!   saves little memory (an empty `String` allocates nothing), but it keeps the
//!   JSON handed to the model free of dozens of `""` fields per product, which
//!   is the scarcer budget.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use regex::Regex;

use crate::{
    forms::{
        r#in::xml::defaults::CallData,
        out::xml::{
            prices::Price,
            products::{Product, Size},
            stocks::Product as StockProduct
        }
    },
    routes::default::derive_xmlns,
    service::{
        get::{
            prices::{PricesData, PricesXML},
            products::{ProductsData, ProductsXML},
            stocks::{StocksData, StocksXML}
        },
        get_data::{RequestGet, ResponseGet},
        log::{elogger, logger},
        mcp::mask_authcode
    }
};

/// Product dimensions.
///
/// A local mirror of `forms::out::xml::products::Size` rather than a reuse of
/// it: that model is `Serialize`-only by way of the shared `out` macro, and a
/// snapshot has to round-trip through the disk store. Converting here keeps
/// `forms/` untouched.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct Dimensions {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<f64>
}

impl From<Size> for Dimensions {
    fn from(size: Size) -> Self {
        Self { x: size.x, y: size.y, z: size.z }
    }
}


/// Serde-visible product record, already merged with the caller's price and
/// stock. Every optional field is dropped from the JSON when absent.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct IndexedProduct {
    /// Internal ERP record id (`cikkid`). Exposed for cross-checking against the
    /// ERP UI, but deliberately **not** part of the search haystack — see
    /// [`FoldedEntry`].
    pub id: u64,
    /// Primary article number / SKU (`cikkszam`).
    pub no: String,
    /// Product name (`cikknev`).
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    /// Manufacturer's own part number (`gycikkszam`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oem_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_unit_qty: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_category_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub main_category_name: Option<String>,
    /// Long description, flattened to plain text at ingest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub weight: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<Dimensions>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sell_unit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin_country: Option<String>,
    /// The caller's own net price, not a list price.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub list_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sale_price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock: Option<f64>
}

/// The lean shape `search_products` returns: enough to identify a product and
/// act on it, without burning the context window on a full record.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ProductSummary {
    pub no: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brand: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub category_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub oem_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub price: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stock: Option<f64>
}

impl From<&IndexedProduct> for ProductSummary {
    fn from(product: &IndexedProduct) -> Self {
        Self {
            no: product.no.clone(),
            name: product.name.clone(),
            brand: product.brand.clone(),
            category_name: product.category_name.clone(),
            unit: product.unit.clone(),
            oem_code: product.oem_code.clone(),
            price: product.price,
            currency: product.currency.clone(),
            stock: product.stock
        }
    }
}


/// Accent-folded haystacks for one product, held parallel to `products`.
///
/// Kept as separate fields rather than one flat string because the ranking below
/// weights an article-number hit far above a category hit, which a single
/// haystack cannot express.
#[derive(Debug, Clone)]
struct FoldedEntry {
    /// Folded product name.
    name: String,
    /// Folded brand + category names — matched, but weakly.
    rest: String,
    /// Folded primary article number. An exact hit here outranks everything.
    sku: String,
    /// Folded manufacturer part number.
    alt_codes: Vec<String>
}

/// One `(authcode, pid, url)` combination's catalog, ready to answer questions.
#[derive(Debug)]
pub struct CatalogSnapshot {
    pub products: Vec<IndexedProduct>,
    /// Folded article number -> index into `products`.
    pub by_sku: HashMap<String, u32>,
    /// Accent-folded haystacks, parallel to `products`.
    folded: Vec<FoldedEntry>,
    pub fetched_at: DateTime<Utc>,
    /// Measured at build time; feeds the cache weigher.
    pub bytes: u64
}


/// What actually goes to disk.
///
/// Only the two things that cannot be recomputed. `folded`, `by_sku` and `bytes`
/// are all derived from `products`, so storing them would roughly double the
/// file for data [`assemble`] regenerates in a single pass on load.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct PersistedSnapshot {
    pub products: Vec<IndexedProduct>,
    pub fetched_at: DateTime<Utc>
}

impl From<&CatalogSnapshot> for PersistedSnapshot {
    fn from(snapshot: &CatalogSnapshot) -> Self {
        Self {
            products: snapshot.products.clone(),
            fetched_at: snapshot.fetched_at
        }
    }
}

impl From<PersistedSnapshot> for CatalogSnapshot {
    fn from(persisted: PersistedSnapshot) -> Self {
        let mut snapshot = assemble(persisted.products);
        // `assemble` stamps "now"; a reloaded snapshot must keep the age it had
        // when it was fetched, or every restart would look like fresh data.
        snapshot.fetched_at = persisted.fetched_at;
        snapshot
    }
}


/// Folds one character to its unaccented lowercase form.
///
/// Hand-rolled rather than NFD-normalizing through a Unicode crate: the ranges
/// below (Latin-1 Supplement + Latin Extended-A) cover Hungarian — including the
/// double-acute `ő`/`ű` that a naive Latin-1 table misses — as well as German,
/// French and the Latin-script Slavic languages the catalog contains. Anything
/// outside those ranges passes through lowercased.
fn fold_char(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => 'a',
        'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => 'e',
        'í' | 'ì' | 'î' | 'ï' | 'ĩ' | 'ī' | 'ĭ' | 'į' => 'i',
        'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ő' | 'ō' | 'ŏ' | 'ø' => 'o',
        'ú' | 'ù' | 'û' | 'ü' | 'ű' | 'ũ' | 'ū' | 'ŭ' | 'ů' | 'ų' => 'u',
        'ý' | 'ÿ' => 'y',
        'ñ' | 'ń' | 'ň' | 'ņ' => 'n',
        'ç' | 'ć' | 'č' | 'ĉ' | 'ċ' => 'c',
        'š' | 'ś' | 'ş' | 'ŝ' => 's',
        'ž' | 'ź' | 'ż' => 'z',
        'ř' | 'ŕ' => 'r',
        'ť' | 'ţ' => 't',
        'ď' | 'đ' => 'd',
        'ĺ' | 'ľ' | 'ł' => 'l',
        'ğ' | 'ĝ' | 'ġ' | 'ģ' => 'g',
        other => other
    }
}


/// Accent- and case-insensitive form used for every comparison in this module,
/// so `szovegkiemelo` matches `Szövegkiemelő`.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(fold_char)
        .collect()
}


static TAG_BR: Lazy<Option<Regex>> = Lazy::new(|| build_regex(r"(?i)<br\s*/?>"));
static TAG_P_CLOSE: Lazy<Option<Regex>> = Lazy::new(|| build_regex(r"(?i)</p>"));
static TAG_ANY: Lazy<Option<Regex>> = Lazy::new(|| build_regex(r"<[^>]+>"));
/// Horizontal whitespace, including the non-breaking space `&nbsp;` decodes to.
static SPACES: Lazy<Option<Regex>> = Lazy::new(|| build_regex(r"[ \t\u{00a0}]+"));
static BLANK_LINES: Lazy<Option<Regex>> = Lazy::new(|| build_regex(r"\n{3,}"));

/// Compiles a pattern once, logging instead of panicking if one is malformed —
/// a bad pattern degrades stripping to a no-op rather than taking the service
/// down.
fn build_regex(pattern: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(regex) => Some(regex),
        Err(error) => {
            elogger(format!("MCP: failed to compile regex '{}': {}", pattern, error));
            None
        }
    }
}


/// Longest entity name this decoder will consider, so a bare `&` in prose does
/// not send it scanning to the end of a 2 KB description looking for a `;`.
const MAX_ENTITY_LEN: usize = 10;


/// Resolves one HTML entity body (the text between `&` and `;`).
///
/// Covers numeric references plus the named entities that actually turn up in
/// Octopus descriptions — typography, symbols and the accented Latin letters
/// Hungarian and German product text uses. Anything unrecognized returns `None`
/// and is left verbatim, which is the safe failure: a stray `&foo;` in the text
/// is better than silently deleting it.
fn resolve_entity(name: &str) -> Option<char> {
    // Numeric: &#8482; (decimal) and &#x2122; (hex).
    if let Some(digits) = name.strip_prefix('#') {
        let code = match digits.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16).ok()?,
            None => digits.parse::<u32>().ok()?
        };
        return char::from_u32(code)
    }

    Some(match name {
        // The five the previous implementation handled.
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        // A plain space, not U+00A0: nothing downstream renders this text, and a
        // non-breaking space would survive the whitespace collapse below.
        "nbsp" => ' ',
        // Typography — the long tail that was surviving into tool output.
        "apos" => '\'',
        "trade" => '™',
        "copy" => '©',
        "reg" => '®',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "bull" => '•',
        "middot" => '·',
        "lsquo" => '\u{2018}',
        "rsquo" => '\u{2019}',
        "sbquo" => '\u{201a}',
        "ldquo" => '\u{201c}',
        "rdquo" => '\u{201d}',
        "bdquo" => '\u{201e}',
        "laquo" => '«',
        "raquo" => '»',
        "dagger" => '†',
        "Dagger" => '‡',
        "permil" => '‰',
        "prime" => '′',
        "Prime" => '″',
        // Symbols and units.
        "deg" => '°',
        "plusmn" => '±',
        "times" => '×',
        "divide" => '÷',
        "minus" => '−',
        "ne" => '≠',
        "le" => '≤',
        "ge" => '≥',
        "sup2" => '²',
        "sup3" => '³',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "micro" => 'µ',
        "euro" => '€',
        "pound" => '£',
        "yen" => '¥',
        "cent" => '¢',
        "sect" => '§',
        "para" => '¶',
        // Accented Latin, including the Hungarian double acutes.
        "aacute" => 'á', "Aacute" => 'Á',
        "eacute" => 'é', "Eacute" => 'É',
        "iacute" => 'í', "Iacute" => 'Í',
        "oacute" => 'ó', "Oacute" => 'Ó',
        "uacute" => 'ú', "Uacute" => 'Ú',
        "ouml" => 'ö', "Ouml" => 'Ö',
        "uuml" => 'ü', "Uuml" => 'Ü',
        "auml" => 'ä', "Auml" => 'Ä',
        "odblac" => 'ő', "Odblac" => 'Ő',
        "udblac" => 'ű', "Udblac" => 'Ű',
        "agrave" => 'à', "egrave" => 'è',
        "acirc" => 'â', "ecirc" => 'ê', "ocirc" => 'ô', "ucirc" => 'û',
        "ntilde" => 'ñ', "ccedil" => 'ç',
        "szlig" => 'ß',
        "oslash" => 'ø', "aring" => 'å', "aelig" => 'æ',
        _ => return None
    })
}


/// Replaces HTML entities with the characters they stand for.
///
/// One left-to-right pass, deliberately: decoding is not re-applied to its own
/// output, so an escaped `&amp;trade;` stays the literal text `&trade;` instead
/// of turning into `™`. Chained `str::replace` calls cannot get that right,
/// because whichever runs last sees the output of the others.
fn decode_entities(text: &str) -> String {
    if !text.contains('&') {
        return text.to_string()
    }

    let mut decoded = String::with_capacity(text.len());
    let mut rest = text;

    while let Some(start) = rest.find('&') {
        decoded.push_str(&rest[..start]);
        let candidate = &rest[start + 1..];

        // Bounded search: an unterminated `&` is prose, not an entity.
        let terminator = candidate.char_indices()
            .take(MAX_ENTITY_LEN + 1)
            .find(|(_, c)| *c == ';')
            .map(|(index, _)| index);

        match terminator.and_then(|end| resolve_entity(&candidate[..end]).map(|c| (c, end))) {
            Some((character, end)) => {
                decoded.push(character);
                rest = &candidate[end + 1..];
            }
            None => {
                decoded.push('&');
                rest = candidate;
            }
        }
    }

    decoded.push_str(rest);
    decoded
}


/// Flattens an HTML description to plain text.
///
/// Descriptions arrive as HTML and the markup is roughly a third of the
/// catalog's footprint. Nothing downstream renders it, so it is flattened once
/// at ingest instead of being carried per query.
pub fn strip_html(html: &str) -> Option<String> {
    if html.trim().is_empty() {
        return None
    }

    // Normalize line endings first. Octopus descriptions mix `\n`, `\r\n` and
    // runs of `\n\r`, and the blank-line collapse below only recognizes `\n` —
    // without this, `\n\r\n\r\n\r` survives untouched into the tool output.
    let mut text = html.replace("\r\n", "\n").replace('\r', "\n");

    if let Some(regex) = TAG_BR.as_ref() {
        text = regex.replace_all(&text, "\n").into_owned();
    }
    if let Some(regex) = TAG_P_CLOSE.as_ref() {
        text = regex.replace_all(&text, "\n").into_owned();
    }
    if let Some(regex) = TAG_ANY.as_ref() {
        text = regex.replace_all(&text, "").into_owned();
    }

    // After tag removal, so an escaped `&lt;b&gt;` becomes the literal text
    // `<b>` rather than a tag the stripper would have eaten.
    text = decode_entities(&text);

    if let Some(regex) = SPACES.as_ref() {
        text = regex.replace_all(&text, " ").into_owned();
    }
    if let Some(regex) = BLANK_LINES.as_ref() {
        text = regex.replace_all(&text, "\n\n").into_owned();
    }

    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}


/// Empty and whitespace-only strings become `None`.
fn non_empty(value: String) -> Option<String> {
    if value.trim().is_empty() { None } else { Some(value) }
}


/// Search filters, all accent-folded substring matches.
#[derive(Debug, Default, Clone)]
pub struct SearchFilters {
    pub brand: Option<String>,
    pub category: Option<String>,
    pub main_category: Option<String>
}

/// Outcome of a search: the page of results plus how many matched in total, so
/// the caller can say when a result set was truncated.
pub struct SearchOutcome {
    pub results: Vec<ProductSummary>,
    pub matched: usize
}


impl CatalogSnapshot {
    /// How old this snapshot is, in seconds.
    pub fn age_secs(&self) -> i64 {
        (Utc::now() - self.fetched_at).num_seconds().max(0)
    }

    /// Exact lookup by article number, then by manufacturer part number, then by
    /// internal record id. The id is accepted here but never fed to the ranking
    /// (see [`FoldedEntry`]).
    pub fn get_by_no(&self, needle: &str) -> Option<&IndexedProduct> {
        let trimmed = needle.trim();
        let folded = fold(trimmed);
        if let Some(index) = self.by_sku.get(&folded)
            && let Some(product) = self.products.get(*index as usize) {
                return Some(product)
        }
        self.products.iter().enumerate().find_map(|(position, product)| {
            let matches_alt = self.folded.get(position)
                .is_some_and(|entry| entry.alt_codes.contains(&folded));
            let matches_id = product.id.to_string() == trimmed;
            (matches_alt || matches_id).then_some(product)
        })
    }

    /// Does this product pass the (already folded) filters?
    fn passes(&self, product: &IndexedProduct, filters: &SearchFilters) -> bool {
        if let Some(brand) = &filters.brand
            && !fold(product.brand.as_deref().unwrap_or_default()).contains(brand) {
                return false
        }
        if let Some(category) = &filters.category {
            let haystack = fold(&format!(
                "{} {}",
                product.category_code.as_deref().unwrap_or_default(),
                product.category_name.as_deref().unwrap_or_default()
            ));
            if !haystack.contains(category) {
                return false
            }
        }
        if let Some(main_category) = &filters.main_category {
            let haystack = fold(&format!(
                "{} {}",
                product.main_category_code.as_deref().unwrap_or_default(),
                product.main_category_name.as_deref().unwrap_or_default()
            ));
            if !haystack.contains(main_category) {
                return false
            }
        }
        true
    }

    /// Ranked search. Every query term must match somewhere, and the best match
    /// kind per term decides its weight.
    ///
    /// The weights are ported verbatim from the TypeScript prototype, where they
    /// were tuned against real colleague queries: an exact article number beats
    /// an exact manufacturer code, which beats a name hit, which beats a brand or
    /// category hit. Changing them changes answers people already trust.
    pub fn search(&self, query: &str, filters: &SearchFilters, limit: usize) -> SearchOutcome {
        self.search_page(query, filters, 0, limit)
    }

    /// [`search`](Self::search) with an offset, so a caller can walk a result set
    /// larger than one page.
    ///
    /// Paging is for *browsing* — reviewing a category of a couple of hundred
    /// products. It is not a bulk-export mechanism: a catalog of 24,000 rows
    /// cannot be carried through a model's context at any page size, which is
    /// what `export_products` is for.
    pub fn search_page(
        &self,
        query: &str,
        filters: &SearchFilters,
        offset: usize,
        limit: usize
    ) -> SearchOutcome {
        let scored = self.ranked(query, filters);
        let matched = scored.len();

        let results = scored.iter()
            .skip(offset)
            .take(limit)
            .filter_map(|(_, position)| self.products.get(*position))
            .map(ProductSummary::from)
            .collect();

        SearchOutcome { results, matched }
    }

    /// How many products a query and filter combination matches, without
    /// building any output. Used to answer "is there anything to export?"
    /// before committing to writing a file.
    pub fn count_matching(&self, query: &str, filters: &SearchFilters) -> usize {
        self.ranked(query, filters).len()
    }

    /// Every matching product in rank order, borrowed rather than copied.
    ///
    /// The export path uses this: 24,000 owned records would be a second copy of
    /// the whole catalog in memory, on a host that has ~1–1.5 GB.
    pub fn select(&self, query: &str, filters: &SearchFilters) -> Vec<&IndexedProduct> {
        self.ranked(query, filters).iter()
            .filter_map(|(_, position)| self.products.get(*position))
            .collect()
    }

    /// The shared ranking pass: every product that passes the filters and
    /// matches every query term, sorted best first.
    fn ranked(&self, query: &str, filters: &SearchFilters) -> Vec<(f64, usize)> {
        let folded_query = fold(query);
        let terms: Vec<&str> = folded_query.split_whitespace().collect();

        let mut scored: Vec<(f64, usize)> = Vec::new();

        for (position, product) in self.products.iter().enumerate() {
            if !self.passes(product, filters) {
                continue
            }
            let Some(entry) = self.folded.get(position) else {
                continue
            };

            let mut score = 0.0_f64;
            let mut all_matched = true;
            for term in &terms {
                let best = if entry.sku == *term {
                    2000.0
                } else if entry.alt_codes.iter().any(|code| code == term) {
                    800.0
                } else if entry.sku.starts_with(term) {
                    300.0
                } else if entry.alt_codes.iter().any(|code| code.starts_with(term)) {
                    200.0
                } else if entry.name.contains(term) {
                    if entry.name.starts_with(term) { 120.0 } else { 60.0 }
                } else if entry.rest.contains(term) {
                    20.0
                } else if entry.sku.contains(term) || entry.alt_codes.iter().any(|code| code.contains(term)) {
                    15.0
                } else {
                    0.0
                };

                if best == 0.0 {
                    all_matched = false;
                    break
                }
                score += best;
            }

            if !all_matched && !terms.is_empty() {
                continue
            }
            // Shorter names are usually the more canonical match at equal score.
            scored.push((score - entry.name.len() as f64 / 1000.0, position));
        }

        scored.sort_by(|a, b| {
            b.0.partial_cmp(&a.0)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| {
                    let left = self.products.get(a.1).map(|p| p.no.as_str()).unwrap_or_default();
                    let right = self.products.get(b.1).map(|p| p.no.as_str()).unwrap_or_default();
                    left.cmp(right)
                })
        });

        scored
    }

    /// The `limit` closest article numbers to a miss, so a failed `get_product`
    /// can suggest alternatives instead of dead-ending the model.
    pub fn did_you_mean(&self, needle: &str, limit: usize) -> Vec<ProductSummary> {
        let outcome = self.search(needle, &SearchFilters::default(), limit);
        if !outcome.results.is_empty() {
            return outcome.results
        }
        // No term matched in full; fall back to a prefix sweep over article
        // numbers, which catches a typo in the tail of a code.
        let folded = fold(needle.trim());
        let prefix: String = folded.chars().take(4).collect();
        if prefix.is_empty() {
            return Vec::new()
        }
        self.folded.iter().enumerate()
            .filter(|(_, entry)| entry.sku.starts_with(&prefix))
            .filter_map(|(position, _)| self.products.get(position))
            .take(limit)
            .map(ProductSummary::from)
            .collect()
    }

    /// Distinct brands / main groups / product groups with counts.
    pub fn categories(&self) -> Categories {
        let mut brands: HashMap<String, u32> = HashMap::new();
        let mut main_groups: HashMap<(String, String), u32> = HashMap::new();
        let mut groups: HashMap<(String, String), u32> = HashMap::new();

        for product in &self.products {
            if let Some(brand) = &product.brand {
                *brands.entry(brand.clone()).or_default() += 1;
            }
            if product.main_category_code.is_some() || product.main_category_name.is_some() {
                let key = (
                    product.main_category_code.clone().unwrap_or_default(),
                    product.main_category_name.clone().unwrap_or_default()
                );
                *main_groups.entry(key).or_default() += 1;
            }
            if product.category_code.is_some() || product.category_name.is_some() {
                let key = (
                    product.category_code.clone().unwrap_or_default(),
                    product.category_name.clone().unwrap_or_default()
                );
                *groups.entry(key).or_default() += 1;
            }
        }

        Categories {
            brands: sorted_counts(brands.into_iter().map(|(name, count)| CategoryCount {
                code: None,
                name,
                count
            })),
            main_categories: sorted_counts(main_groups.into_iter().map(|((code, name), count)| CategoryCount {
                code: non_empty(code),
                name,
                count
            })),
            categories: sorted_counts(groups.into_iter().map(|((code, name), count)| CategoryCount {
                code: non_empty(code),
                name,
                count
            }))
        }
    }
}


/// Most populous first, then alphabetical, so a truncated list keeps the useful end.
fn sorted_counts(counts: impl Iterator<Item = CategoryCount>) -> Vec<CategoryCount> {
    let mut items: Vec<CategoryCount> = counts.collect();
    items.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.name.cmp(&b.name)));
    items
}


#[derive(Debug, Clone, serde::Serialize)]
pub struct CategoryCount {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub name: String,
    pub count: u32
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct Categories {
    pub brands: Vec<CategoryCount>,
    pub main_categories: Vec<CategoryCount>,
    pub categories: Vec<CategoryCount>
}


/// Why a snapshot could not be built. Each variant maps to an actionable MCP
/// error, so the model can tell "wrong credentials" from "ERP unreachable".
#[derive(Debug)]
pub enum SnapshotError {
    /// Octopus answered, but with an error in the envelope (bad authcode, etc.).
    Upstream(String),
    /// The response did not deserialize into the expected shape.
    Malformed
}

impl std::fmt::Display for SnapshotError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SnapshotError::Upstream(message) => write!(f, "{}", message),
            SnapshotError::Malformed => write!(f, "the ERP response could not be read")
        }
    }
}


/// Builds the `CallData` a snapshot fetch needs, mirroring how the REST routes
/// assemble theirs. `language`/`data_type` stay unset so the English XML models
/// come back, which is what this module indexes.
fn call_data(authcode: &str, pid: i64, url: &str, from_date: Option<DateTime<Utc>>) -> CallData {
    CallData {
        authcode: authcode.to_string(),
        url: url.to_string(),
        xmlns: derive_xmlns(url),
        pid: Some(pid),
        from_date,
        ..Default::default()
    }
}


/// The three raw pieces a snapshot is assembled from.
struct CatalogParts {
    products: Vec<Product>,
    /// Always the full price list: `GetArlistaAuth` takes no date parameter, so
    /// there is no such thing as an incremental price pull.
    prices: HashMap<String, Price>,
    /// Stock levels, keyed by article number. Incremental when `from_date` was
    /// set, in which case it holds only what moved since.
    stocks: HashMap<String, f64>
}


/// Fetches products, prices and stock for one combination.
///
/// `from_date` makes the product and stock pulls incremental (`web_update`);
/// prices come back in full either way. Only prices vary by `pid` —
/// `GetCikkekAuth` and `GetCikkekKeszletValtozasAuth` take an authcode alone —
/// so two pids under one authcode differ solely in the price columns.
async fn fetch_parts(
    authcode: &str,
    pid: i64,
    url: &str,
    from_date: Option<DateTime<Utc>>
) -> Result<CatalogParts, SnapshotError> {
    // Products first: without them there is nothing to merge into, and an auth
    // failure surfaces once here rather than three times over.
    let products_response = RequestGet::Products(call_data(authcode, pid, url, from_date)).into_data().await;
    let ResponseGet::Products(ProductsData::Xml(ProductsXML::En(products))) = products_response else {
        return Err(SnapshotError::Malformed)
    };

    if let Some(error) = products.body.response.result.answer.error {
        return Err(SnapshotError::Upstream(format!("{} ({})", error.description, error.code)))
    }

    // Prices and stock are independent of each other, so they overlap. Both go
    // through the existing SOAP_GATE, so this cannot exceed the configured
    // outbound concurrency.
    let (prices_response, stocks_response) = futures::join!(
        // Prices carry no date: `call_data`'s `from_date` is ignored by
        // `GetArlistaAuth`, so this is a full list on every refresh.
        RequestGet::Prices(call_data(authcode, pid, url, None)).into_data(),
        RequestGet::Stocks(call_data(authcode, pid, url, from_date)).into_data()
    );

    let prices: HashMap<String, Price> = match prices_response {
        ResponseGet::Prices(PricesData::Xml(PricesXML::En(envelope)))
            if envelope.body.response.result.answer.error.is_none() =>
        {
            envelope.body.response.result.answer.prices.price
                .into_iter()
                .map(|price| (price.no.clone(), price))
                .collect()
        }
        _ => {
            // A price failure degrades the snapshot rather than failing it: the
            // catalog is still worth answering from, just without figures.
            elogger(format!("MCP snapshot: prices unavailable for {} pid={}", mask_authcode(authcode), pid));
            HashMap::new()
        }
    };

    let stocks: HashMap<String, f64> = match stocks_response {
        ResponseGet::Stocks(StocksData::Xml(StocksXML::En(envelope)))
            if envelope.body.response.result.answer.error.is_none() =>
        {
            envelope.body.response.result.answer.products.product
                .into_iter()
                .filter_map(|stock: StockProduct| stock.stock.map(|level| (stock.no, level)))
                .collect()
        }
        _ => {
            elogger(format!("MCP snapshot: stock unavailable for {} pid={}", mask_authcode(authcode), pid));
            HashMap::new()
        }
    };

    Ok(CatalogParts {
        products: products.body.response.result.answer.products.product,
        prices,
        stocks
    })
}


/// Builds the folded haystacks and the article-number map over a finished
/// product list, and measures the result.
///
/// `id` (`cikkid`) is deliberately absent from every haystack: it is an internal
/// key whose values collide with other products' article numbers, so indexing it
/// corrupts the ranking. Exact lookup by id still works through
/// [`CatalogSnapshot::get_by_no`].
fn assemble(products: Vec<IndexedProduct>) -> CatalogSnapshot {
    let mut folded: Vec<FoldedEntry> = Vec::with_capacity(products.len());
    let mut by_sku: HashMap<String, u32> = HashMap::with_capacity(products.len());

    for (position, product) in products.iter().enumerate() {
        let entry = FoldedEntry {
            name: fold(&product.name),
            rest: fold(&format!(
                "{} {} {}",
                product.brand.as_deref().unwrap_or_default(),
                product.category_name.as_deref().unwrap_or_default(),
                product.main_category_name.as_deref().unwrap_or_default()
            )),
            sku: fold(&product.no),
            alt_codes: product.oem_code.as_deref().map(|code| vec![fold(code)]).unwrap_or_default()
        };
        if !entry.sku.is_empty() {
            // `u32` indices keep the map small; a catalog past four billion rows
            // is not a case this service needs to handle.
            by_sku.insert(entry.sku.clone(), position as u32);
        }
        folded.push(entry);
    }

    let bytes = measure_bytes(&products, &folded, &by_sku);
    CatalogSnapshot { products, by_sku, folded, fetched_at: Utc::now(), bytes }
}


/// Builds a complete snapshot from scratch.
pub async fn build_snapshot(
    authcode: &str,
    pid: i64,
    url: &str,
    from_date: Option<DateTime<Utc>>
) -> Result<CatalogSnapshot, SnapshotError> {
    let started = std::time::Instant::now();
    let parts = fetch_parts(authcode, pid, url, from_date).await?;

    let products = parts.products.into_iter()
        .map(|product| {
            let price = parts.prices.get(&product.no);
            let stock = parts.stocks.get(&product.no).copied();
            to_indexed(product, price, stock)
        })
        .collect();

    let snapshot = assemble(products);
    log_build("built", authcode, pid, &snapshot, started);
    Ok(snapshot)
}


/// Refreshes an existing snapshot with only what changed since `since`.
///
/// Necessary because an incremental product pull returns *only* changed records:
/// publishing that as-is would replace a 24,000-product catalog with a handful of
/// rows. The delta is merged over the previous snapshot instead — changed rows
/// replace their predecessors, new rows are appended, and everything else is
/// carried forward with refreshed prices.
///
/// Incremental responses do not report products **deleted** in the ERP, which is
/// why the precache job still schedules a full pull weekly.
pub async fn refresh_snapshot(
    previous: &CatalogSnapshot,
    authcode: &str,
    pid: i64,
    url: &str,
    since: DateTime<Utc>
) -> Result<CatalogSnapshot, SnapshotError> {
    let started = std::time::Instant::now();
    let parts = fetch_parts(authcode, pid, url, Some(since)).await?;

    let mut products = previous.products.clone();
    let mut positions: HashMap<String, usize> = HashMap::with_capacity(products.len());
    for (position, product) in products.iter().enumerate() {
        positions.insert(product.no.clone(), position);
    }

    // Prices arrived in full, so they are authoritative for every row. A failed
    // price fetch yields an empty map — carry the previous figures rather than
    // blanking the catalog.
    if !parts.prices.is_empty() {
        for product in products.iter_mut() {
            let price = parts.prices.get(&product.no);
            product.price = price.and_then(|entry| entry.price);
            product.list_price = price.and_then(|entry| entry.list_price);
            product.sale_price = price.and_then(|entry| entry.sale_price);
            product.currency = price.and_then(|entry| non_empty(entry.currency.clone()));
        }
    }

    // Stock deltas: only the rows that moved.
    for (no, level) in &parts.stocks {
        if let Some(position) = positions.get(no)
            && let Some(product) = products.get_mut(*position) {
                product.stock = Some(*level);
        }
    }

    // Product deltas replace or append.
    let changed = parts.products.len();
    for product in parts.products {
        let price = parts.prices.get(&product.no);
        let existing = positions.get(&product.no).copied();
        let stock = parts.stocks.get(&product.no).copied()
            .or_else(|| existing.and_then(|position| products.get(position)).and_then(|p| p.stock));
        let indexed = to_indexed(product, price, stock);

        match existing {
            Some(position) => products[position] = indexed,
            None => products.push(indexed)
        }
    }

    let snapshot = assemble(products);
    log_build(&format!("refreshed ({} changed)", changed), authcode, pid, &snapshot, started);
    Ok(snapshot)
}


/// One line per build, with the authcode masked and the measured size included
/// so the configured budget can be tuned against reality.
fn log_build(
    what: &str,
    authcode: &str,
    pid: i64,
    snapshot: &CatalogSnapshot,
    started: std::time::Instant
) {
    logger(format!(
        "MCP snapshot {} for {} pid={}: {} products, {:.1} MB, {:.1}s",
        what,
        mask_authcode(authcode),
        pid,
        snapshot.products.len(),
        snapshot.bytes as f64 / 1_048_576.0,
        started.elapsed().as_secs_f64()
    ));
}


/// Projects an English `Product` plus the caller's price and stock into the
/// indexed record, dropping empty fields and flattening the description.
fn to_indexed(product: Product, price: Option<&Price>, stock: Option<f64>) -> IndexedProduct {
    IndexedProduct {
        id: product.id,
        no: product.no,
        name: product.name,
        brand: non_empty(product.brand),
        oem_code: non_empty(product.oem_code),
        unit: non_empty(product.unit),
        base_unit: non_empty(product.base_unit),
        base_unit_qty: product.base_unit_qty,
        category_code: non_empty(product.category_code),
        category_name: non_empty(product.category_name),
        main_category_code: non_empty(product.main_category_code),
        main_category_name: non_empty(product.main_category_name),
        description: strip_html(&product.description),
        weight: product.weight,
        size: product.size.map(Dimensions::from),
        sell_unit: product.sell_unit,
        origin_country: non_empty(product.origin_country),
        price: price.and_then(|p| p.price),
        list_price: price.and_then(|p| p.list_price),
        sale_price: price.and_then(|p| p.sale_price),
        currency: price.and_then(|p| non_empty(p.currency.clone())),
        stock
    }
}


/// Approximate heap footprint of a snapshot, in bytes.
///
/// Deliberately an estimate: it counts the struct arrays plus every owned
/// string's capacity, which is where a catalog's memory actually goes. It feeds
/// the cache weigher and is logged per build so the configured budget can be
/// tuned against measured reality rather than a guess.
fn measure_bytes(
    products: &[IndexedProduct],
    folded: &[FoldedEntry],
    by_sku: &HashMap<String, u32>
) -> u64 {
    let mut bytes = size_of_val(products) as u64;
    bytes += size_of_val(folded) as u64;

    for product in products {
        bytes += product.no.capacity() as u64;
        bytes += product.name.capacity() as u64;
        for field in [
            &product.brand, &product.oem_code, &product.unit, &product.base_unit,
            &product.category_code, &product.category_name, &product.main_category_code,
            &product.main_category_name, &product.description, &product.origin_country,
            &product.currency
        ] {
            bytes += field.as_ref().map_or(0, |value| value.capacity()) as u64;
        }
    }

    for entry in folded {
        bytes += (entry.name.capacity() + entry.rest.capacity() + entry.sku.capacity()) as u64;
        bytes += (entry.alt_codes.capacity() * size_of::<String>()) as u64;
        bytes += entry.alt_codes.iter().map(|code| code.capacity() as u64).sum::<u64>();
    }

    // HashMap overhead: roughly the key plus one bucket entry per row.
    bytes += by_sku.keys()
        .map(|key| (key.capacity() + size_of::<(String, u32)>()) as u64)
        .sum::<u64>();

    bytes
}


/// A synthetic snapshot of a declared size, for exercising the cache's budget
/// and eviction behaviour without a live ERP.
#[cfg(test)]
pub fn test_snapshot(bytes: u64) -> CatalogSnapshot {
    CatalogSnapshot {
        products: Vec::new(),
        by_sku: HashMap::new(),
        folded: Vec::new(),
        fetched_at: Utc::now(),
        bytes
    }
}


/// A synthetic snapshot holding real products, for exercising serialization and
/// the disk store.
#[cfg(test)]
pub fn test_snapshot_with_products(count: usize) -> CatalogSnapshot {
    let products = (0..count)
        .map(|index| test_product(&format!("A-{}", index), "Szövegkiemelő", "Orink", ""))
        .collect();
    assemble(products)
}


/// A synthetic product, shared by this module's tests and the export module's.
#[cfg(test)]
pub fn test_product(no: &str, name: &str, brand: &str, oem: &str) -> IndexedProduct {
    IndexedProduct {
        id: 1,
        no: no.into(),
        name: name.into(),
        brand: non_empty(brand.into()),
        oem_code: non_empty(oem.into()),
        unit: None,
        base_unit: None,
        base_unit_qty: None,
        category_code: None,
        category_name: None,
        main_category_code: None,
        main_category_name: None,
        description: None,
        weight: None,
        size: None,
        sell_unit: None,
        origin_country: None,
        price: None,
        list_price: None,
        sale_price: None,
        currency: None,
        stock: None
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    pub(super) fn product(no: &str, name: &str, brand: &str, oem: &str) -> IndexedProduct {
        IndexedProduct {
            id: 1,
            no: no.into(),
            name: name.into(),
            brand: non_empty(brand.into()),
            oem_code: non_empty(oem.into()),
            unit: None,
            base_unit: None,
            base_unit_qty: None,
            category_code: None,
            category_name: None,
            main_category_code: None,
            main_category_name: None,
            description: None,
            weight: None,
            size: None,
            sell_unit: None,
            origin_country: None,
            price: None,
            list_price: None,
            sale_price: None,
            currency: None,
            stock: None
        }
    }

    fn snapshot(rows: Vec<IndexedProduct>) -> CatalogSnapshot {
        let mut by_sku = HashMap::new();
        let mut folded = Vec::new();
        for (position, row) in rows.iter().enumerate() {
            let entry = FoldedEntry {
                name: fold(&row.name),
                rest: fold(row.brand.as_deref().unwrap_or_default()),
                sku: fold(&row.no),
                alt_codes: row.oem_code.as_deref().map(|code| vec![fold(code)]).unwrap_or_default()
            };
            by_sku.insert(entry.sku.clone(), position as u32);
            folded.push(entry);
        }
        CatalogSnapshot { products: rows, by_sku, folded, fetched_at: Utc::now(), bytes: 0 }
    }

    #[test]
    fn folding_ignores_hungarian_accents() {
        // The case the prototype's tests pin: an unaccented query must reach an
        // accented name, double-acute included.
        assert_eq!(fold("Szövegkiemelő"), "szovegkiemelo");
        assert_eq!(fold("ÁRVÍZTŰRŐ tükörfúrógép"), "arvizturo tukorfurogep");
    }

    #[test]
    fn exact_article_number_outranks_a_name_hit() {
        let mut rows = vec![
            product("XYZ-1", "ABC-1 lookalike", "Orink", ""),
            product("ABC-1", "Highlighter", "Orink", "")
        ];
        rows[0].id = 7;
        let snapshot = snapshot(rows);

        let outcome = snapshot.search("abc-1", &SearchFilters::default(), 10);
        assert_eq!(outcome.matched, 2);
        assert_eq!(outcome.results[0].no, "ABC-1");
    }

    #[test]
    fn every_term_must_match() {
        let snapshot = snapshot(vec![product("A1", "Blue highlighter", "Orink", "")]);
        assert_eq!(snapshot.search("blue highlighter", &SearchFilters::default(), 10).matched, 1);
        assert_eq!(snapshot.search("blue stapler", &SearchFilters::default(), 10).matched, 0);
    }

    #[test]
    fn accent_folded_query_finds_accented_name() {
        let snapshot = snapshot(vec![product("A1", "Szövegkiemelő sárga", "Orink", "")]);
        assert_eq!(snapshot.search("szovegkiemelo", &SearchFilters::default(), 10).matched, 1);
    }

    #[test]
    fn record_id_is_not_searchable() {
        // `id` 12345 collides with another product's article number; searching
        // for it must return only the article-number holder, never the id owner.
        let mut rows = vec![product("99999", "Decoy", "Orink", ""), product("12345", "Real", "Orink", "")];
        rows[0].id = 12345;
        let snapshot = snapshot(rows);

        let outcome = snapshot.search("12345", &SearchFilters::default(), 10);
        assert_eq!(outcome.matched, 1);
        assert_eq!(outcome.results[0].no, "12345");
    }

    #[test]
    fn lookup_falls_back_to_oem_code_and_record_id() {
        let mut rows = vec![product("A1", "Pen", "Orink", "MFG-77")];
        rows[0].id = 4242;
        let snapshot = snapshot(rows);

        assert_eq!(snapshot.get_by_no("a1").map(|p| p.no.as_str()), Some("A1"));
        assert_eq!(snapshot.get_by_no("MFG-77").map(|p| p.no.as_str()), Some("A1"));
        assert_eq!(snapshot.get_by_no("4242").map(|p| p.no.as_str()), Some("A1"));
        assert!(snapshot.get_by_no("nope").is_none());
    }

    #[test]
    fn filters_narrow_results() {
        let snapshot = snapshot(vec![
            product("A1", "Pen", "Orink", ""),
            product("A2", "Pen", "Other", "")
        ]);
        let filters = SearchFilters { brand: Some(fold("orink")), ..Default::default() };
        let outcome = snapshot.search("pen", &filters, 10);
        assert_eq!(outcome.matched, 1);
        assert_eq!(outcome.results[0].no, "A1");
    }

    #[test]
    fn search_reports_total_matches_beyond_the_limit() {
        let rows = (0..10).map(|i| product(&format!("A{}", i), "Pen", "Orink", "")).collect();
        let snapshot = snapshot(rows);
        let outcome = snapshot.search("pen", &SearchFilters::default(), 3);
        assert_eq!(outcome.results.len(), 3);
        assert_eq!(outcome.matched, 10);
    }

    #[test]
    fn html_is_flattened_at_ingest() {
        let stripped = strip_html("<p>Blue&nbsp;pen</p><br/><b>2 mm</b>");
        assert_eq!(stripped.as_deref(), Some("Blue pen\n\n2 mm"));
        assert_eq!(strip_html("   "), None);
    }

    #[test]
    fn entities_beyond_the_common_five_are_decoded() {
        // All of these were surviving verbatim into tool output.
        assert_eq!(strip_html("PEFC&trade;").as_deref(), Some("PEFC™"));
        assert_eq!(strip_html("&bdquo;quoted&rdquo;").as_deref(), Some("„quoted”"));
        assert_eq!(strip_html("talaj &ndash; a v&iacute;z").as_deref(), Some("talaj – a víz"));
        assert_eq!(strip_html("&copy; 2026 &reg;").as_deref(), Some("© 2026 ®"));
        assert_eq!(strip_html("20&deg;C &plusmn;2").as_deref(), Some("20°C ±2"));
    }

    #[test]
    fn hungarian_letters_written_as_entities_survive_a_round_trip() {
        // The umlaut and the double acute are different letters in Hungarian
        // (`ö`/`ő`, `ü`/`ű`) and must decode to different characters.
        assert_eq!(
            strip_html("s&ouml;t&eacute;t z&ouml;ld, &udblac;rlap, els&odblac;").as_deref(),
            Some("sötét zöld, űrlap, első")
        );
        // And the decoded form is still reachable by an unaccented search term.
        assert_eq!(fold(&strip_html("Sz&ouml;vegkiemel&odblac;").unwrap()), "szovegkiemelo");
    }

    #[test]
    fn numeric_entities_are_decoded_in_both_bases() {
        assert_eq!(strip_html("&#8482; and &#x2122;").as_deref(), Some("™ and ™"));
        assert_eq!(strip_html("&#233;").as_deref(), Some("é"));
    }

    #[test]
    fn unknown_and_malformed_entities_are_left_verbatim() {
        // Dropping them silently would lose real text; leaving them is the safe
        // failure.
        assert_eq!(strip_html("R&D &notareal; x").as_deref(), Some("R&D &notareal; x"));
        assert_eq!(strip_html("a & b").as_deref(), Some("a & b"));
        // An unterminated `&` must not send the scanner to the end of the text.
        assert_eq!(strip_html("100 & 200 rest").as_deref(), Some("100 & 200 rest"));
        assert_eq!(strip_html("&#99999999;").as_deref(), Some("&#99999999;"));
    }

    #[test]
    fn decoding_is_not_applied_to_its_own_output() {
        // `&amp;trade;` is an escaped ampersand followed by literal text, not a
        // trademark sign. Chained replaces get this wrong; one pass does not.
        assert_eq!(strip_html("&amp;trade;").as_deref(), Some("&trade;"));
    }

    #[test]
    fn mixed_line_endings_collapse_to_blank_lines() {
        // The exact shape Octopus sends: `\n\r` pairs the old `\n{3,}` rule
        // could not see.
        assert_eq!(
            strip_html("Tan&uacute;s&iacute;tv&aacute;ny:\n\r\n\r\nPEFC\n\r\n\r\nEU").as_deref(),
            Some("Tanúsítvány:\n\nPEFC\n\nEU")
        );
        assert_eq!(strip_html("a\r\n\r\n\r\n\r\nb").as_deref(), Some("a\n\nb"));
    }

    #[test]
    fn escaped_markup_survives_tag_stripping_as_text() {
        // Decoding after tag removal keeps escaped markup as readable text
        // rather than feeding it back to the stripper.
        assert_eq!(strip_html("use &lt;b&gt; for bold").as_deref(), Some("use <b> for bold"));
    }

    #[test]
    fn did_you_mean_suggests_near_article_numbers() {
        let snapshot = snapshot(vec![
            product("ABCD-1", "Pen", "Orink", ""),
            product("ABCD-2", "Pen", "Orink", ""),
            product("ZZZZ-9", "Stapler", "Orink", "")
        ]);
        let suggestions = snapshot.did_you_mean("ABCD-7", 5);
        assert!(suggestions.iter().all(|s| s.no.starts_with("ABCD")));
        assert_eq!(suggestions.len(), 2);
    }

    #[test]
    fn categories_are_counted_and_ordered_by_size() {
        let mut rows = vec![
            product("A1", "Pen", "Orink", ""),
            product("A2", "Pen", "Orink", ""),
            product("A3", "Pen", "Other", "")
        ];
        for row in rows.iter_mut() {
            row.category_name = Some("Pens".into());
            row.category_code = Some("PEN".into());
        }
        let categories = snapshot(rows).categories();
        assert_eq!(categories.brands[0].name, "Orink");
        assert_eq!(categories.brands[0].count, 2);
        assert_eq!(categories.categories[0].count, 3);
    }
}
