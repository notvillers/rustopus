// CSV serialization helpers
use std::cell::Cell;

thread_local! {
    /// Whether the CSV currently being serialized on this thread is Hungarian.
    /// Set by `routes::default::send_csv` right before serialization; CSV writing
    /// is synchronous, so the flag never leaks across requests.
    static CSV_HU: Cell<bool> = const { Cell::new(false) };
}

/// Sets the language flag consulted by the `serialize_with` helpers below.
pub fn set_csv_hu(hu: bool) {
    CSV_HU.with(|c| c.set(hu));
}

/// `#[serde(serialize_with = "...")]` helper for `bool` CSV columns.
/// Writes `Igaz`/`Hamis` for Hungarian exports, `true`/`false` otherwise.
pub fn bool_lang<S: serde::Serializer>(value: &bool, serializer: S) -> Result<S::Ok, S::Error> {
    let text = match (CSV_HU.with(|c| c.get()), value) {
        (true, true) => "Igaz",
        (true, false) => "Hamis",
        (false, true) => "true",
        (false, false) => "false"
    };
    serializer.serialize_str(text)
}
