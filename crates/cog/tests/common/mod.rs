//! Fixture loaders shared by the cog tests.

use idleon_cog::CogSave;
use idleon_import::import;

/// A committed fixture, by its path from the repository root.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{path}", env!("CARGO_MANIFEST_DIR")))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

/// The cog board and shelf `fixtures/data.json` imports.
pub fn cog_save() -> CogSave {
    import(&read("fixtures/data.json"))
        .expect("the fixture imports")
        .cog
}
