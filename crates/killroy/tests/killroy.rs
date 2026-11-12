//! The weekly forecast is pinned against the upstream planner's own output.

use idleon_import::import;
use idleon_killroy as killroy;
use serde_json::{Value, json};

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

/// The synthetic account as an `it.json` wrapper: the forecast needs the
/// server's Killroy rotation beside `data`.
fn wrapped_fixture() -> String {
    let save: Value = serde_json::from_str(&read("fixtures/data.json")).expect("fixture JSON");
    json!({ "data": save, "serverVars": { "KillroySwap": 7 } }).to_string()
}

/// JSON does not distinguish `2240` from `2240.0`; the oracle and the Rust
/// `f64` fields can, so compare every number as a float.
fn floats(value: Value) -> Value {
    match value {
        Value::Number(number) => Value::from(number.as_f64().expect("a finite number")),
        Value::Array(values) => Value::Array(values.into_iter().map(floats).collect()),
        Value::Object(fields) => Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, floats(value)))
                .collect(),
        ),
        value => value,
    }
}

#[test]
fn the_forecast_matches_the_upstream_planner() {
    let expected: Value =
        serde_json::from_str(&read("fixtures/killroy.json")).expect("oracle JSON");
    let weeks = expected["weeks"].as_u64().expect("oracle week count") as usize;
    let imported = import(&wrapped_fixture()).expect("the wrapped fixture imports");
    let forecast = killroy::forecast(&imported.save, weeks).expect("the forecast builds");

    assert_eq!(
        floats(serde_json::to_value(&forecast).expect("forecast serializes")),
        floats(expected["forecast"].clone()),
        "the port must reproduce the upstream forecast field for field"
    );
    assert_eq!(forecast.weeks.len(), weeks + 1);
}

#[test]
fn the_week_range_validates_before_reading_the_save() {
    let imported = import(&wrapped_fixture()).expect("the wrapped fixture imports");
    for weeks in [0, 53] {
        let error = killroy::forecast(&imported.save, weeks).expect_err("out of range");
        assert_eq!(error.to_string(), "Choose 1-52 upcoming weeks");
    }
}

#[test]
fn a_save_without_the_server_rotation_says_so() {
    let flat: Value = serde_json::from_str(&read("fixtures/data.json")).expect("fixture JSON");
    let imported = import(&flat.to_string()).expect("the flat fixture imports");
    let error = killroy::forecast(&imported.save, 1).expect_err("no server rotation");
    assert_eq!(
        error.to_string(),
        "Missing or invalid server Killroy rotation (import a wrapped save)"
    );
}

#[test]
fn a_save_without_characters_asks_for_one() {
    let imported = import(
        &json!({
            "Research": "[]",
            "TimeAway": "{\"GlobalTime\":1794296810.952,\"ShopRestock\":62019.836}",
            "OptLacc": { "39": 5 },
            "serverVars": { "KillroySwap": 7 },
        })
        .to_string(),
    )
    .expect("minimal save imports");
    let error = killroy::forecast(&imported.save, 1).expect_err("no characters");
    assert_eq!(error.to_string(), "Load a save with characters");
}
