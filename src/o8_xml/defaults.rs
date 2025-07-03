/// Default struct(s) for XML(s) got from the Octopus call

use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct Hiba {
    pub kod: u64,
    pub leiras: String
}
