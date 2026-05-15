// String manipulation

pub fn remove_breaks(string: &str) -> String {
    string.replace(['\n', '\r'], " ").to_string()
}
