// String manipulations
use serde::{Deserialize, Deserializer};
use serde_json::Value;

#[allow(dead_code)]
pub fn trim<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Value::deserialize(deserializer)?;
    match value {
        Value::String(s) => Ok(remove_breaks(&s)),
        other => Ok(other.to_string())
    }
}


fn remove_breaks(string: &str) -> String {
    string
        .replace("\r\n", " ")
        .replace('\n', " ")
        .replace('\r', " ")
        .trim()
        .to_string()
}