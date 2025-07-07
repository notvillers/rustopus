/// Default struct(s) for XML(s) got from the Octopus call

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
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
    pub pid: Option<i64>
}
