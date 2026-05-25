// String manipulation

pub fn remove_breaks(string: &str) -> String {
    string.replace(['\n', '\r'], " ").to_string()
}

// TODO: Replace this method with the ./src/C/string_man.c's remove_breaks
