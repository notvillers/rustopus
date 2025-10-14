/// Default struct(s) for XML(s) got from the Octopus call
use chrono::{DateTime, Utc};

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub struct Hiba {
    pub kod: u64,
    pub leiras: String
}


#[derive(Clone)]
pub struct CallData {
    pub authcode: String,
    pub url: String,
    pub xmlns: String,
    pub pid: Option<i64>,
    pub type_mod: Option<i64>,
    pub from_date: Option<DateTime<Utc>>,
    pub to_date: Option<DateTime<Utc>>,
    pub unpaid: Option<i64>
}

impl Default for CallData {
    fn default() -> Self {
        Self {
            authcode: "".into(),
            url: "".into(),
            xmlns: "".into(),
            pid: None,
            type_mod: None,
            from_date: None,
            to_date: None,
            unpaid: None
        }
    }
}