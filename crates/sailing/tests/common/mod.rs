//! Helpers shared by the sailing oracle tests.

#![allow(dead_code)]

use idleon_import::import;
use idleon_sailing::{Fleet, RouteContext};
use serde_json::Value;

/// A committed fixture, by its path from the repository root.
pub fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

/// A committed JSON oracle, by its path from the repository root.
pub fn oracle(path: &str) -> Value {
    serde_json::from_str(&read(path)).expect("oracle JSON")
}

pub fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number in the oracle, got {}", value[key]))
}

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
pub fn close(actual: f64, expected: f64, field: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{field}: expected {expected}, got {actual}"
    );
}

/// The oracle's save imported, with its talent value and no other bonuses.
pub fn imported(oracle: &Value) -> (Fleet, RouteContext) {
    let save = serde_json::to_string(&oracle["save"]).expect("the oracle save serialises");
    let imported = import(&save).expect("the oracle save imports");
    let mut context = imported.sailing;
    context.unending_loot_search = number(oracle, "unendingLootSearch");
    (imported.fleet, context)
}
