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
fn the_wisdom_tab_matches_the_upstream_calculator() {
    let save = fixture();
    let snapshot = monument::wisdom_snapshot(&save, 0);
    assert!(snapshot.available, "the fixture carries the Monument rows");
    assert_close(
        &serde_json::to_value(&snapshot).expect("the snapshot serialises"),
        &oracle()["wisdom"],
        "wisdom",
    );
}

#[test]
fn every_reward_charge_matches_the_upstream_calculator() {
    let save = fixture();
    let oracle = oracle();
    let cases = oracle["charges"].as_array().expect("oracle charges");
    assert_eq!(cases.len(), 3, "one charge per monument");
    for case in cases {
        let monument = case["monument"].as_u64().expect("a monument") as usize;
        for (key, extra) in [
            ("charge", 0.0),
            ("tomorrow", 86_400.0),
            ("extraWait", 3_600.0),
        ] {
            let charge = monument::reward_charge(&save, monument, 0, extra)
                .unwrap_or_else(|error| panic!("monument {monument}: {error}"));
            assert_close(
                &serde_json::to_value(&charge).expect("the charge serialises"),
                &case[key],
                &format!("charges[{monument}].{key}"),
            );
        }
    }
}

#[test]
fn rejected_charge_inputs_keep_the_upstream_message() {
    let save = fixture();
    let oracle = oracle();
    let cases = oracle["chargeErrors"]
        .as_array()
        .expect("oracle rejections");
    for case in cases {
        let label = case["label"].as_str().expect("a case label");
        let monument = case["monument"].as_u64().expect("a monument") as usize;
        let extra = case["extraSeconds"].as_f64().expect("an extra wait");
        assert_eq!(
            monument::reward_charge(&save, monument, 0, extra)
                .err()
                .unwrap_or_else(|| panic!("{label}: the port must reject the input"))
                .to_string(),
            case["message"].as_str().expect("a message"),
            "{label}"
        );
    }
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

/// One worksheet state from an oracle entry.
fn justice_state(entry: &Value, label: &str) -> monument::justice::State {
    serde_json::from_value(entry["state"].clone())
        .unwrap_or_else(|error| panic!("{label}: the oracle state decodes: {error}"))
}

fn justice_case_id(entry: &Value, label: &str) -> usize {
    entry["caseId"]
        .as_u64()
        .unwrap_or_else(|| panic!("{label}: a case id")) as usize
}

#[test]
fn the_justice_snapshot_matches_the_upstream_calculator() {
    let save = fixture();
    assert_close(
        &serde_json::to_value(monument::justice_snapshot(&save, 0))
            .expect("the snapshot serialises"),
        &oracle()["justice"]["snapshot"],
        "justice.snapshot",
    );
}

#[test]
fn the_justice_worksheet_matches_the_upstream_advisor() {
    let save = fixture();
    let oracle = oracle();
    let initial = monument::justice::initial_state(&save);
    assert_close(
        &serde_json::to_value(&initial).expect("the state serialises"),
        &oracle["justice"]["initial"],
        "justice.initial",
    );

    let reports = oracle["justice"]["reports"]
        .as_array()
        .expect("oracle reports");
    assert!(reports.len() >= 90, "the oracle covers every case");
    for entry in reports {
        let label = entry["label"].as_str().expect("a case label");
        let state = justice_state(entry, label);
        let id = justice_case_id(entry, label);
        let report = monument::justice::responses(&state, id)
            .unwrap_or_else(|error| panic!("{label}: the upstream case is valid: {error}"));
        assert_close(
            &serde_json::to_value(&report).expect("the report serialises"),
            &entry["report"],
            label,
        );
    }
}

#[test]
fn opening_a_justice_case_matches_the_upstream_timers() {
    let oracle = oracle();
    let cases = oracle["justice"]["opened"]
        .as_array()
        .expect("oracle cases");
    assert!(cases.len() >= 15, "the oracle covers every settlement");
    for entry in cases {
        let label = entry["label"].as_str().expect("a case label");
        let state = justice_state(entry, label);
        let opened = monument::justice::open_case(&state, justice_case_id(entry, label))
            .unwrap_or_else(|error| panic!("{label}: the upstream case opens: {error}"));
        assert_close(
            &serde_json::to_value(&opened).expect("the opened case serialises"),
            &entry["opened"],
            label,
        );
    }
}

#[test]
fn the_delayed_justice_effects_match_upstream() {
    let oracle = oracle();
    let states = oracle["justice"]["delayed"]
        .as_array()
        .expect("oracle states");
    let reports = oracle["justice"]["reports"]
        .as_array()
        .expect("oracle reports");
    // The first delayed entry is the fresh story, the second the worked sheet.
    for (index, expected) in states.iter().enumerate() {
        let entry = if index == 0 {
            &reports[0]
        } else {
            reports
                .iter()
                .find(|entry| entry["label"] == "worked worksheet, a gift")
                .expect("the worked worksheet")
        };
        let label = entry["label"].as_str().expect("a case label");
        let state = justice_state(entry, label);
        assert_close(
            &serde_json::to_value(monument::justice::delayed_effects(&state))
                .expect("the effects serialise"),
            expected,
            label,
        );
    }
}

#[test]
fn rejected_justice_inputs_keep_the_upstream_messages() {
    let oracle = oracle();
    let cases = oracle["justice"]["errors"]
        .as_array()
        .expect("oracle rejections");
    assert!(cases.len() >= 4, "the oracle covers each rejected input");
    for entry in cases {
        let label = entry["label"].as_str().expect("a case label");
        let state = justice_state(entry, label);
        let id = justice_case_id(entry, label);
        let error = match entry["kind"].as_str().expect("a kind") {
            "open" => monument::justice::open_case(&state, id).err(),
            _ => monument::justice::responses(&state, id).err(),
        }
        .unwrap_or_else(|| panic!("{label}: the port must reject the input"));
        assert_eq!(
            error.to_string(),
            entry["message"].as_str().expect("a message"),
            "{label}"
        );
    }
}
