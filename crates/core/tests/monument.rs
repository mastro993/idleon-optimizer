//! Bravery parity against the upstream calculator's own output.
//!
//! `fixtures/monument.json` is written by `script/monument-oracle.mjs` from
//! `js/stats/systems/w5/monument.js`, so the port is checked against the source
//! of truth rather than hand-copied numbers.

use idleon_core::monument::{self, Runtime, Save};
use idleon_core::save::import;
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/monument.json")).expect("oracle JSON")
}

fn fixture() -> Save {
    let imported = import(&read("fixtures/data.json")).expect("the fixture imports");
    Save::from_parts(&imported.save, imported.companion.as_ref())
}

/// Compare one oracle value.
///
/// Numbers use a relative tolerance: the port adds the training sources in a
/// different order than the upstream chain of additions, which can move the
/// last bit of a long `f64`. Everything else must match exactly.
fn assert_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(actual), Value::Number(expected)) => {
            let actual = actual.as_f64().expect("a finite number");
            let expected = expected.as_f64().expect("a finite number");
            let tolerance = 1e-9 * expected.abs().max(1.0);
            assert!(
                (actual - expected).abs() <= tolerance,
                "{path}: {actual} != {expected}"
            );
        }
        (Value::Array(actual), Value::Array(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: length");
            for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
                assert_close(actual, expected, &format!("{path}[{index}]"));
            }
        }
        (Value::Object(actual), Value::Object(expected)) => {
            assert_eq!(actual.len(), expected.len(), "{path}: field count");
            for (key, expected) in expected {
                let actual = actual
                    .get(key)
                    .unwrap_or_else(|| panic!("{path}: missing {key}"));
                assert_close(actual, expected, &format!("{path}.{key}"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

#[test]
fn the_bravery_tab_matches_the_upstream_calculator() {
    let save = fixture();
    let snapshot = monument::snapshot(&save, 0);
    assert!(snapshot.available, "the fixture carries the Monument rows");
    assert_close(
        &serde_json::to_value(&snapshot).expect("the snapshot serialises"),
        &oracle()["snapshot"],
        "snapshot",
    );
}

#[test]
fn every_run_state_matches_the_upstream_odds() {
    let save = fixture();
    let oracle = oracle();
    let cases = oracle["odds"].as_array().expect("oracle run states");
    assert!(cases.len() >= 5, "the oracle covers the run-state paths");
    for case in cases {
        let label = case["label"].as_str().expect("a case label");
        let runtime = Runtime {
            boosts: case["runtime"]["boosts"].as_u64().unwrap_or(0) as u32,
            rerolls: case["runtime"]["rerolls"]
                .as_u64()
                .map(|value| value as u32),
        };
        let odds = monument::bravery_odds(
            &save,
            case["wave"].as_u64().expect("a wave") as u32,
            runtime,
            case["remainingHp"].as_f64(),
        )
        .unwrap_or_else(|error| panic!("{label}: the upstream run is valid: {error}"));
        assert_close(
            &serde_json::to_value(&odds).expect("the odds serialise"),
            &case["result"],
            label,
        );
    }
}

#[test]
fn rejected_inputs_keep_the_upstream_messages() {
    let save = fixture();
    let oracle = oracle();
    let cases = oracle["errors"].as_array().expect("oracle rejections");
    assert!(cases.len() >= 4, "the oracle covers each rejected input");
    for case in cases {
        let label = case["label"].as_str().expect("a case label");
        let runtime = Runtime {
            boosts: case["runtime"]["boosts"].as_u64().unwrap_or(0) as u32,
            rerolls: case["runtime"]["rerolls"]
                .as_u64()
                .map(|value| value as u32),
        };
        let error = monument::bravery_odds(
            &save,
            case["wave"].as_u64().expect("a wave") as u32,
            runtime,
            case["remainingHp"].as_f64(),
        )
        .err()
        .unwrap_or_else(|| panic!("{label}: the port must reject the input"));
        assert_eq!(
            error.to_string(),
            case["message"].as_str().expect("a message"),
            "{label}"
        );
    }
}
