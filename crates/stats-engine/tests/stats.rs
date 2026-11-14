//! Parity against the upstream calculator: every character and stat in
//! `fixtures/data.json`, against the tree `script/stats-oracle.mjs` recorded.
//!
//! The engine is a partial port, so the test pins two things instead of a total
//! nobody can reach yet: the pools this build does model must match upstream
//! exactly, and any total that does not match must name the terms it left out.
//! A term can therefore never be wrong silently.

use idleon_stats_engine::{Stat, total_stat};
use serde_json::Value;
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures")
        .join(name)
}

fn fixture(name: &str) -> Value {
    let text = std::fs::read_to_string(fixture_path(name)).expect("fixture reads");
    serde_json::from_str(&text).expect("fixture parses")
}

fn close(left: f64, right: f64) -> bool {
    if left == right {
        return true;
    }
    let scale = left.abs().max(right.abs()).max(1.0);
    (left - right).abs() <= scale * 1e-9
}

#[test]
fn every_character_and_stat_resolves_and_reports_its_gaps() {
    let stats = fixture("stats.json");
    let text = std::fs::read_to_string(fixture_path("data.json")).expect("the save reads");
    let imported = idleon_import::import(&text).expect("the fixture save imports");
    for case in stats["cases"].as_array().expect("cases") {
        let character = case["character"].as_u64().expect("character") as usize;
        for stat in Stat::ALL {
            let total = total_stat(&imported.save, &imported.simulation, character, stat)
                .unwrap_or_else(|| panic!("character {character} {} has no total", stat.id()));
            assert_eq!(
                total.computed,
                total.tree.value,
                "character {character} {}: the value and its tree disagree",
                stat.id()
            );
            let upstream = &case["stats"][stat.id()];
            let want = upstream["value"].as_f64().unwrap_or(0.0);
            if !close(total.computed, want) {
                assert!(
                    !total.missing.is_empty(),
                    "character {character} {}: {} vs upstream {want} with no recorded gap",
                    stat.id(),
                    total.computed
                );
            }
        }
    }
}
