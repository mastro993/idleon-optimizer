//! Parity against the upstream calculator: every character and stat in
//! `fixtures/data.json`, against the tree `script/stats-oracle.mjs` recorded.
//!
//! Every total must be fully justified: it matches upstream exactly and its
//! `missing` list is empty. A term can therefore never be wrong silently.

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
            assert!(
                total.is_complete(),
                "character {character} {}: still missing {:?}",
                stat.id(),
                total.missing
            );
            assert!(
                close(total.computed, want),
                "character {character} {}: got {}, upstream {want}",
                stat.id(),
                total.computed
            );
        }
    }
}
