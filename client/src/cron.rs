use std::fs;
use std::path::PathBuf;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};

use crate::api::EndpointParams;

const CRONS_FILE: &str = "crons.toml";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum IntervalUnit {
    Minutes,
    Hours,
    Days,
}

impl IntervalUnit {
    pub fn label(&self) -> &str {
        match self {
            IntervalUnit::Minutes => "Minutes",
            IntervalUnit::Hours => "Hours",
            IntervalUnit::Days => "Days",
        }
    }

    pub fn to_duration(&self, value: u64) -> Duration {
        match self {
            IntervalUnit::Minutes => Duration::minutes(value as i64),
            IntervalUnit::Hours => Duration::hours(value as i64),
            IntervalUnit::Days => Duration::days(value as i64),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    pub name: String,
    pub enabled: bool,
    pub endpoint: String,
    pub params: EndpointParams,
    pub interval_value: u64,
    pub interval_unit: IntervalUnit,
    pub output_dir: String,
    /// Filename template; supports `{date}` and `{datetime}` placeholders.
    pub output_filename: String,
    #[serde(default)]
    pub last_run: Option<String>,
    #[serde(default)]
    pub last_status: Option<String>,
}

impl CronJob {
    pub fn new(name: String, endpoint: String) -> Self {
        Self {
            name,
            enabled: true,
            endpoint,
            params: EndpointParams::default(),
            interval_value: 60,
            interval_unit: IntervalUnit::Minutes,
            output_dir: String::new(),
            output_filename: "{date}.xml".to_string(),
            last_run: None,
            last_status: None,
        }
    }

    /// Returns seconds remaining until the next scheduled run.
    /// Negative means overdue.
    pub fn seconds_until_next_run(&self) -> Option<i64> {
        let ts = self.last_run.as_deref()?;
        let last = ts.parse::<DateTime<Utc>>().ok()?;
        let interval = self.interval_unit.to_duration(self.interval_value);
        let next = last + interval;
        Some((next - Utc::now()).num_seconds())
    }

    pub fn next_run_label(&self) -> String {
        if !self.enabled {
            return "disabled".to_string();
        }
        match self.seconds_until_next_run() {
            None => "pending".to_string(),
            Some(secs) if secs <= 0 => "now".to_string(),
            Some(secs) => {
                let d = secs / 86400;
                let h = (secs % 86400) / 3600;
                let m = (secs % 3600) / 60;
                let s = secs % 60;
                if d > 0 {
                    format!("{d}d {h}h {m}m")
                } else if h > 0 {
                    format!("{h}h {m}m {s}s")
                } else if m > 0 {
                    format!("{m}m {s}s")
                } else {
                    format!("{s}s")
                }
            }
        }
    }

    pub fn is_due(&self) -> bool {
        match &self.last_run {
            None => true,
            Some(ts) => {
                let Ok(last) = ts.parse::<DateTime<Utc>>() else {
                    return true;
                };
                let interval = self.interval_unit.to_duration(self.interval_value);
                Utc::now() >= last + interval
            }
        }
    }

    /// Keep the filename extension in sync with the selected output format.
    pub fn sync_filename_extension(&mut self) {
        let ext = if self.params.data_type.eq_ignore_ascii_case("csv") {
            "csv"
        } else {
            "xml"
        };
        let name = &self.output_filename;
        if let Some(stem) = name.strip_suffix(".xml").or_else(|| name.strip_suffix(".csv")) {
            self.output_filename = format!("{stem}.{ext}");
        }
    }

    /// Resolve filename placeholders at the current time.
    pub fn resolved_filename(&self) -> String {
        let now = Utc::now();
        let date = now.format("%Y-%m-%d").to_string();
        let datetime = now.format("%Y-%m-%dT%H-%M-%S").to_string();
        self.output_filename
            .replace("{date}", &date)
            .replace("{datetime}", &datetime)
    }

    pub fn output_path(&self) -> PathBuf {
        PathBuf::from(&self.output_dir).join(self.resolved_filename())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CronConfig {
    pub jobs: Vec<CronJob>,
}

impl CronConfig {
    fn config_path() -> PathBuf {
        crate::config::data_path(CRONS_FILE)
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = fs::read_to_string(&path) {
            toml::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        if let Ok(content) = toml::to_string_pretty(self) {
            let path = Self::config_path();
            crate::config::ensure_parent_dir(&path);
            let _ = fs::write(path, content);
        }
    }
}
