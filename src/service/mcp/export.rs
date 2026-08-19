//! Bulk export: catalog rows to a spreadsheet, delivered as a download link.
//!
//! ## Why this exists
//!
//! Colleagues need product data in Excel, and the obvious approach — paging
//! through `search_products` — does not work at this scale. The catalog is
//! ~24,000 products; at any page size those rows would have to pass through the
//! model's context on the way to the user, which is both ruinously expensive and
//! lossy. So the rows never go near the model: they are written to a file
//! server-side and the tool returns only a URL, a row count and a size.
//!
//! ## Why the files are guarded
//!
//! An export holds the caller's **own negotiated prices** and stock. The
//! download link therefore carries an unguessable token rather than an authcode
//! — an authcode in a URL would land in every access log between here and the
//! browser — and both the token and the file expire. Files are written `0600`
//! inside a `0700` directory, like the snapshot store.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use actix_files::NamedFile;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{HttpRequest, HttpResponse, Responder, Scope, web};
use once_cell::sync::Lazy;

use crate::service::{
    config::get_mcp_settings,
    ipv4::log_ip,
    log::{elog_with_ip, elogger, log_with_ip, logger},
    mcp::index::IndexedProduct,
    path::get_current_or_root_dir
};

/// Column order for both formats. Fixed rather than derived from the struct so
/// the spreadsheet stays stable for people building their own sheets on top of
/// it, and so the header row can be human-readable.
const COLUMNS: [(&str, &str); 17] = [
    ("no", "Article number"),
    ("name", "Name"),
    ("brand", "Brand"),
    ("oem_code", "Manufacturer part number"),
    ("barcode", "Barcode (EAN)"),
    ("category_code", "Product group code"),
    ("category_name", "Product group"),
    ("main_category_code", "Main group code"),
    ("main_category_name", "Main group"),
    ("unit", "Unit"),
    ("base_unit", "Base unit"),
    ("base_unit_qty", "Base units per unit"),
    ("price", "Price"),
    ("currency", "Currency"),
    ("stock", "Stock"),
    ("weight", "Weight (kg)"),
    ("origin_country", "Country of origin")
];


/// Output format for an export.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Xlsx,
    Csv
}

impl Format {
    /// Parses the tool's `format` argument. `None` means the caller did not ask,
    /// which is the common case and means Excel.
    pub fn parse(value: Option<&str>) -> Option<Self> {
        match value.map(str::trim).map(str::to_lowercase).as_deref() {
            None | Some("") | Some("xlsx") | Some("excel") | Some("xls") => Some(Format::Xlsx),
            Some("csv") => Some(Format::Csv),
            _ => None
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            Format::Xlsx => "xlsx",
            Format::Csv => "csv"
        }
    }

    fn content_type(&self) -> &'static str {
        match self {
            Format::Xlsx => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            Format::Csv => "text/csv; charset=utf-8"
        }
    }
}


/// A written export, ready to be handed to the caller as a link.
pub struct Prepared {
    pub file_name: String,
    pub bytes: u64,
    pub url: String
}


/// One downloadable file, held only until it expires.
struct Entry {
    path: PathBuf,
    file_name: String,
    content_type: &'static str,
    created: Instant
}

/// Live download tokens. In memory on purpose: a restart should invalidate every
/// outstanding link rather than leave price files reachable indefinitely.
static TOKENS: Lazy<Mutex<HashMap<String, Entry>>> = Lazy::new(|| Mutex::new(HashMap::new()));


/// How long a download link stays valid.
pub fn ttl_secs() -> u64 {
    get_mcp_settings().export_ttl_secs()
}


/// Directory exports are written to, resolved against the working directory like
/// every other runtime path in this service.
pub fn export_dir() -> PathBuf {
    let configured = get_mcp_settings().export_path();
    let path = PathBuf::from(&configured);
    if path.is_absolute() {
        return path
    }
    let mut base = get_current_or_root_dir();
    base.push(configured);
    base
}


/// Narrows a path to owner-only access. Logged rather than fatal — refusing to
/// export at all would be a worse outcome than the platform's default
/// permissions on a directory the deployment notes already call private.
fn restrict(path: &std::path::Path, mode: u32) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if let Err(error) = std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)) {
            elogger(format!("MCP export: cannot restrict permissions on '{:?}': {}", path, error));
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (path, mode);
    }
}


/// An unguessable token. Built from the OS random source through `uuid`, which
/// this service already depends on, rather than a sequence a caller could walk.
fn new_token() -> String {
    format!("{}{}", uuid::Uuid::new_v4().simple(), uuid::Uuid::new_v4().simple())
}


/// Renders one product field as text, for CSV.
fn field_text(product: &IndexedProduct, key: &str) -> String {
    match key {
        "no" => product.no.clone(),
        "name" => product.name.clone(),
        "brand" => product.brand.clone().unwrap_or_default(),
        "oem_code" => product.oem_code.clone().unwrap_or_default(),
        // The main EAN only. A product's other codes identify packaging units,
        // and a webshop import wants one value per row, not a list in a cell.
        // Deliberately text and never a `field_number`: a spreadsheet renders a
        // 13-digit number in scientific notation and eats a leading zero.
        "barcode" => product.barcodes.first().cloned().unwrap_or_default(),
        "category_code" => product.category_code.clone().unwrap_or_default(),
        "category_name" => product.category_name.clone().unwrap_or_default(),
        "main_category_code" => product.main_category_code.clone().unwrap_or_default(),
        "main_category_name" => product.main_category_name.clone().unwrap_or_default(),
        "unit" => product.unit.clone().unwrap_or_default(),
        "base_unit" => product.base_unit.clone().unwrap_or_default(),
        "currency" => product.currency.clone().unwrap_or_default(),
        "origin_country" => product.origin_country.clone().unwrap_or_default(),
        "base_unit_qty" => product.base_unit_qty.map(|v| v.to_string()).unwrap_or_default(),
        "price" => product.price.map(|v| v.to_string()).unwrap_or_default(),
        "stock" => product.stock.map(|v| v.to_string()).unwrap_or_default(),
        "weight" => product.weight.map(|v| v.to_string()).unwrap_or_default(),
        _ => String::new()
    }
}

/// The numeric fields, which go into a spreadsheet as numbers rather than text
/// so they can be summed and sorted without the recipient retyping them.
fn field_number(product: &IndexedProduct, key: &str) -> Option<f64> {
    match key {
        "base_unit_qty" => product.base_unit_qty,
        "price" => product.price,
        "stock" => product.stock,
        "weight" => product.weight,
        _ => None
    }
}


/// Writes the rows to a file and registers a download token for it.
///
/// Blocking and CPU-bound by nature — callers must run it through
/// `web::block` rather than on an async worker.
pub fn write(rows: Vec<&IndexedProduct>, format: Format) -> Result<Prepared, String> {
    let dir = export_dir();
    if !dir.is_dir() {
        std::fs::create_dir_all(&dir).map_err(|error| format!("cannot create '{:?}': {}", dir, error))?;
        restrict(&dir, 0o700);
    }

    let token = new_token();
    let file_name = format!("orink-products-{}.{}", chrono::Utc::now().format("%Y%m%d-%H%M"), format.extension());
    let path = dir.join(format!("{}.{}", token, format.extension()));

    match format {
        Format::Xlsx => write_xlsx(&rows, &path)?,
        Format::Csv => write_csv(&rows, &path)?
    }
    restrict(&path, 0o600);

    let bytes = std::fs::metadata(&path).map(|meta| meta.len()).unwrap_or(0);

    if let Ok(mut tokens) = TOKENS.lock() {
        tokens.insert(token.clone(), Entry {
            path,
            file_name: file_name.clone(),
            content_type: format.content_type(),
            created: Instant::now()
        });
    }
    sweep_expired();

    Ok(Prepared {
        file_name,
        bytes,
        url: format!("{}/export/{}", public_base_url(), token)
    })
}


/// Base URL the download link is built on.
///
/// Configured rather than derived from the request, because the MCP transport
/// gives a tool no view of the `Host` header, and a link the colleague's browser
/// can actually reach is the whole point.
fn public_base_url() -> String {
    get_mcp_settings().public_url()
}


fn write_xlsx(rows: &[&IndexedProduct], path: &PathBuf) -> Result<(), String> {
    use rust_xlsxwriter::{Format as XlsxFormat, Workbook};

    let mut workbook = Workbook::new();
    let bold = XlsxFormat::new().set_bold();
    let worksheet = workbook.add_worksheet();

    for (column, (_, header)) in COLUMNS.iter().enumerate() {
        worksheet.write_string_with_format(0, column as u16, *header, &bold)
            .map_err(|error| error.to_string())?;
    }
    // Freeze the header so a 24,000-row sheet stays navigable.
    worksheet.set_freeze_panes(1, 0).map_err(|error| error.to_string())?;

    for (index, product) in rows.iter().enumerate() {
        // Row 0 is the header, so data starts at 1. `u32` is the sheet's own row
        // type and the format's hard limit is ~1M rows, well above this catalog.
        let row = index as u32 + 1;
        for (column, (key, _)) in COLUMNS.iter().enumerate() {
            let column = column as u16;
            match field_number(product, key) {
                Some(number) => worksheet.write_number(row, column, number).map_err(|e| e.to_string())?,
                None => worksheet.write_string(row, column, field_text(product, key)).map_err(|e| e.to_string())?
            };
        }
    }

    workbook.save(path).map_err(|error| error.to_string())
}


fn write_csv(rows: &[&IndexedProduct], path: &PathBuf) -> Result<(), String> {
    // Semicolon-delimited, matching what the REST endpoints produce — and what a
    // Hungarian Excel opens without an import dialog.
    let mut writer = csv::WriterBuilder::new()
        .delimiter(b';')
        .from_path(path)
        .map_err(|error| error.to_string())?;

    writer.write_record(COLUMNS.iter().map(|(_, header)| *header))
        .map_err(|error| error.to_string())?;

    for product in rows {
        let record: Vec<String> = COLUMNS.iter().map(|(key, _)| field_text(product, key)).collect();
        writer.write_record(&record).map_err(|error| error.to_string())?;
    }

    writer.flush().map_err(|error| error.to_string())
}


/// Resolves a download token to the file it stands for, or `None` when it is
/// unknown or expired.
pub fn resolve(token: &str) -> Option<(PathBuf, String, &'static str)> {
    sweep_expired();
    let tokens = TOKENS.lock().ok()?;
    let entry = tokens.get(token)?;
    Some((entry.path.clone(), entry.file_name.clone(), entry.content_type))
}


/// Deletes expired exports and forgets their tokens.
///
/// Run on every write and every download rather than on a timer: exports are
/// infrequent, so a sweep at those two moments keeps the directory tidy without
/// another background task.
pub fn sweep_expired() {
    let ttl = Duration::from_secs(ttl_secs());
    let mut removed = Vec::new();

    if let Ok(mut tokens) = TOKENS.lock() {
        tokens.retain(|_, entry| {
            if entry.created.elapsed() < ttl {
                return true
            }
            removed.push(entry.path.clone());
            false
        });
    }

    for path in removed {
        if let Err(error) = std::fs::remove_file(&path) {
            elogger(format!("MCP export: cannot remove expired '{:?}': {}", path, error));
        }
    }
}


/// Serves one export by its download token.
///
/// The token is the only credential: it is unguessable, single-purpose and
/// short-lived, which is what lets the link work in a plain browser without the
/// colleague having an authcode. An unknown or expired token gets a flat 404 —
/// no hint about whether it ever existed.
async fn download(path: web::Path<String>, request: HttpRequest) -> impl Responder {
    let token = path.into_inner();
    let ip_address = log_ip(request.clone()).await.to_string();

    let Some((file, file_name, content_type)) = resolve(&token) else {
        elog_with_ip(&ip_address, "EXPORT: download rejected — unknown or expired token");
        return HttpResponse::NotFound()
            .content_type("text/plain")
            .body("This download link has expired or does not exist. Ask for the export again.")
    };

    match NamedFile::open_async(&file).await {
        Ok(named) => {
            log_with_ip(&ip_address, format!("EXPORT: served '{}'", file_name));
            // Attachment, so a browser saves the workbook instead of trying to
            // render it, and under the friendly name rather than the token.
            let named = named.set_content_disposition(ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(file_name)]
            });
            // A failed parse just leaves the type actix inferred from the
            // extension, which is already correct for both formats.
            match content_type.parse() {
                Ok(parsed) => named.set_content_type(parsed).into_response(&request),
                Err(_) => named.into_response(&request)
            }
        }
        Err(error) => {
            // The token was live but the file is gone — a manual deletion, or a
            // sweep that raced this request.
            elogger(format!("MCP export: token resolved but '{:?}' is unreadable: {}", file, error));
            HttpResponse::NotFound()
                .content_type("text/plain")
                .body("This export is no longer available. Ask for it again.")
        }
    }
}


/// The `/export` scope.
///
/// The third mount that is a scope rather than the repo's `get`/`get_alias`
/// route pair, and deliberately so: this is a token-authenticated file download
/// with a path parameter, not a fetcher endpoint, so a plural alias would mean
/// nothing. Registered only when MCP is enabled.
pub fn scope() -> Scope {
    web::scope("/export")
        .route("/{token}", web::get().to(download))
}


/// Deletes every export left on disk, whether this process knows its token or
/// not. Called at startup: tokens live in memory, so files from a previous run
/// are unreachable and would otherwise sit there holding price data.
pub fn purge_orphans() {
    let dir = export_dir();
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return
    };

    let mut count = 0;
    for entry in entries.flatten() {
        if entry.path().is_file() && std::fs::remove_file(entry.path()).is_ok() {
            count += 1;
        }
    }
    if count > 0 {
        logger(format!("MCP export: cleared {} orphaned export(s) from a previous run", count));
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_defaults_to_excel() {
        assert_eq!(Format::parse(None), Some(Format::Xlsx));
        assert_eq!(Format::parse(Some("")), Some(Format::Xlsx));
        assert_eq!(Format::parse(Some("xlsx")), Some(Format::Xlsx));
        assert_eq!(Format::parse(Some(" Excel ")), Some(Format::Xlsx));
        assert_eq!(Format::parse(Some("CSV")), Some(Format::Csv));
        assert_eq!(Format::parse(Some("pdf")), None);
    }

    #[test]
    fn tokens_are_long_and_unpredictable() {
        let one = new_token();
        let two = new_token();
        assert_ne!(one, two);
        // Two UUIDs' worth of hex: not something a caller can walk.
        assert_eq!(one.len(), 64);
        assert!(one.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn every_column_renders_without_panicking() {
        let product = crate::service::mcp::index::test_product("A-1", "Pen", "Orink", "MFG-1");
        for (key, _) in COLUMNS {
            let _ = field_text(&product, key);
            let _ = field_number(&product, key);
        }
        assert_eq!(field_text(&product, "no"), "A-1");
        assert_eq!(field_text(&product, "brand"), "Orink");
        // A barcode stays text, or Excel turns 13 digits into 5.99877E+12.
        assert_eq!(field_number(&product, "barcode"), None);
        // Absent optional values render as empty, never as "None".
        assert_eq!(field_text(&product, "currency"), "");
        assert_eq!(field_number(&product, "price"), None);
    }

    #[test]
    fn the_barcode_column_carries_the_main_ean() {
        let mut product = crate::service::mcp::index::test_product("A-1", "Pen", "Orink", "");
        assert_eq!(field_text(&product, "barcode"), "", "no codes renders empty, not \"None\"");

        product.barcodes = vec!["5998765432109".into(), "15998765432106".into()];
        assert_eq!(field_text(&product, "barcode"), "5998765432109");
    }

    #[test]
    fn numeric_columns_are_written_as_numbers() {
        let mut product = crate::service::mcp::index::test_product("A-1", "Pen", "Orink", "");
        product.price = Some(110.5);
        product.stock = Some(43.0);
        assert_eq!(field_number(&product, "price"), Some(110.5));
        assert_eq!(field_number(&product, "stock"), Some(43.0));
        // Text columns never claim to be numeric.
        assert_eq!(field_number(&product, "name"), None);
    }

    #[test]
    fn the_sheet_carries_exactly_one_price_column() {
        // Three price columns made a recipient guess which one to quote. The
        // export publishes only the partner's own figure, so there is nothing to
        // pick between.
        let price_columns: Vec<&str> = COLUMNS.iter()
            .map(|(_, header)| *header)
            .filter(|header| header.to_lowercase().contains("price"))
            .collect();
        assert_eq!(price_columns, vec!["Price"], "the sheet must offer one price and no choice");

        let mut product = crate::service::mcp::index::test_product("A-1", "Pen", "Orink", "");
        product.price = Some(1495.0);
        assert_eq!(field_number(&product, "price"), Some(1495.0));
    }
}
