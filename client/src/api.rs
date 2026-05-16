use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Endpoint {
    Products,
    Prices,
    Stocks,
    Invoices,
    Bulk,
    Images,
    Barcodes,
}

impl Endpoint {
    pub fn all() -> Vec<Endpoint> {
        vec![
            Endpoint::Products,
            Endpoint::Prices,
            Endpoint::Stocks,
            Endpoint::Invoices,
            Endpoint::Bulk,
            Endpoint::Images,
            Endpoint::Barcodes,
        ]
    }

    pub fn label(&self) -> &str {
        match self {
            Endpoint::Products => "Products",
            Endpoint::Prices => "Prices",
            Endpoint::Stocks => "Stocks",
            Endpoint::Invoices => "Invoices",
            Endpoint::Bulk => "Bulk",
            Endpoint::Images => "Images",
            Endpoint::Barcodes => "Barcodes",
        }
    }

    pub fn path(&self) -> &str {
        match self {
            Endpoint::Products => "/get-products",
            Endpoint::Prices => "/get-prices",
            Endpoint::Stocks => "/get-stocks",
            Endpoint::Invoices => "/get-invoices",
            Endpoint::Bulk => "/get-bulk",
            Endpoint::Images => "/get-images",
            Endpoint::Barcodes => "/get-barcodes",
        }
    }

    pub fn needs_pid(&self) -> bool {
        matches!(
            self,
            Endpoint::Prices | Endpoint::Invoices | Endpoint::Bulk
        )
    }

    pub fn has_from_date(&self) -> bool {
        matches!(
            self,
            Endpoint::Products
                | Endpoint::Stocks
                | Endpoint::Invoices
                | Endpoint::Bulk
                | Endpoint::Images
                | Endpoint::Barcodes
        )
    }

    pub fn has_to_date(&self) -> bool {
        matches!(self, Endpoint::Invoices)
    }

    pub fn has_language(&self) -> bool {
        !matches!(self, Endpoint::Bulk)
    }

    pub fn has_data_type(&self) -> bool {
        !matches!(self, Endpoint::Images)
    }

    pub fn has_type_mod(&self) -> bool {
        matches!(self, Endpoint::Invoices)
    }

    pub fn has_unpaid(&self) -> bool {
        matches!(self, Endpoint::Invoices)
    }
}

#[derive(Debug, Default, Clone)]
pub struct EndpointParams {
    pub from_date: String,
    pub to_date: String,
    pub language: String,
    pub data_type: String,
    pub type_mod: String,
    pub unpaid: String,
}

pub fn fetch(
    server_url: &str,
    octopus_url: &str,
    authcode: &str,
    xmlns: &str,
    pid: &str,
    endpoint: &Endpoint,
    params: &EndpointParams,
) -> Result<String, String> {
    let base = server_url.trim_end_matches('/');
    let mut query: HashMap<String, String> = HashMap::new();

    if !octopus_url.is_empty() {
        query.insert("url".into(), octopus_url.into());
    }
    if !authcode.is_empty() {
        query.insert("authcode".into(), authcode.into());
    }
    if !xmlns.is_empty() {
        query.insert("xmlns".into(), xmlns.into());
    }
    if endpoint.needs_pid() && !pid.is_empty() {
        query.insert("pid".into(), pid.into());
    }
    if endpoint.has_from_date() && !params.from_date.is_empty() {
        query.insert("from_date".into(), params.from_date.clone());
    }
    if endpoint.has_to_date() && !params.to_date.is_empty() {
        query.insert("to_date".into(), params.to_date.clone());
    }
    if endpoint.has_language() && !params.language.is_empty() {
        query.insert("language".into(), params.language.clone());
    }
    if endpoint.has_data_type() && !params.data_type.is_empty() {
        query.insert("data_type".into(), params.data_type.clone());
    }
    if endpoint.has_type_mod() && !params.type_mod.is_empty() {
        query.insert("type_mod".into(), params.type_mod.clone());
    }
    if endpoint.has_unpaid() && !params.unpaid.is_empty() {
        query.insert("unpaid".into(), params.unpaid.clone());
    }

    let url = format!("{}{}", base, endpoint.path());

    let client = reqwest::blocking::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {e}"))?;

    let response = client
        .get(&url)
        .query(&query.iter().collect::<Vec<_>>())
        .send()
        .map_err(|e| format!("Request failed: {e}"))?;

    let status = response.status();
    let body = response
        .text()
        .map_err(|e| format!("Failed to read response body: {e}"))?;

    if status.is_success() {
        Ok(body)
    } else {
        Err(format!("HTTP {}: {}", status, body))
    }
}
