//! The dungeon totals are pinned against the upstream calculator's own output.

use idleon_core::drop_rate::{self, Runtime, Total};
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/drop-rate.json")).expect("oracle JSON")
}

#[test]
fn every_dungeon_total_matches_the_upstream_calculator() {
    let oracle = oracle();
    let cases = oracle["cases"].as_array().expect("oracle cases");
    assert!(!cases.is_empty(), "the oracle carries dungeon cases");
    for case in cases {
        let map = case["map"].as_u64().expect("map") as usize;
        let luk = case["luk"].as_f64().expect("luk");
        let drop_rate = case["dropRate"].as_f64().expect("drop rate");
        let expected = case["total"].as_f64().expect("total");
        let outcome = drop_rate::total(
            map,
            Runtime {
                luk: Some(luk),
                drop_rate: Some(drop_rate),
            },
        );
        assert_eq!(
            outcome,
            Total::Value(expected),
            "map {map} at {luk}/{drop_rate}"
        );
        assert!(
            outcome.reason().is_none(),
            "a computed total shows no reason"
        );
    }
}

#[test]
fn the_missing_inputs_reason_is_the_upstream_one() {
    let oracle = oracle();
    let map = oracle["missingInputs"]["map"].as_u64().expect("map") as usize;
    let expected = oracle["missingInputs"]["reason"]
        .as_str()
        .expect("upstream reason");
    let outcome = drop_rate::total(map, Runtime::default());
    assert_eq!(outcome, Total::NeedsRuntime);
    assert_eq!(outcome.reason(), Some(expected));
    assert_eq!(expected, drop_rate::NEEDS_RUNTIME);
}

#[test]
fn a_non_dungeon_map_reports_the_missing_account_stats_engine() {
    let oracle = oracle();
    let map = oracle["nonDungeon"]["map"].as_u64().expect("map") as usize;
    assert!(
        oracle["nonDungeon"]["total"].as_f64().expect("reference") > 1.0,
        "the oracle records the total this build cannot compute yet"
    );
    let outcome = drop_rate::total(
        map,
        Runtime {
            luk: Some(1200.0),
            drop_rate: Some(35.0),
        },
    );
    assert_eq!(outcome, Total::Unsupported);
    assert!(
        outcome
            .reason()
            .is_some_and(|reason| reason.contains("shared stat pipeline")),
        "{:?}",
        outcome.reason()
    );
}
