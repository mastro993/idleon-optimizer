//! Fixture loaders shared by the coral tests.

/// A committed fixture, by its path from the repository root.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

/// `fixtures/data.json`, the synthetic account save.
pub fn fixture() -> String {
    read("fixtures/data.json")
}
