//! The weekly forecast and plan inputs against the synthetic account.

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

#[test]
fn a_selection_outside_the_three_encounters_is_an_error() {
    let imported = import(&wrapped_fixture()).expect("the wrapped fixture imports");
    let input = killroy::PlanInput {
        selections: vec![killroy::Selection {
            week_offset: 0,
            ci: 0,
            slot: 9,
        }],
        seeds: vec![1],
        comparisons: killroy::Comparisons::Off,
        kills_per_second: 3.0,
        keep_buffs: false,
        unlimited_inventory: false,
    };
    let error = killroy::plan(&imported.save, &input).expect_err("no such slot");
    assert_eq!(
        error.to_string(),
        "Choose one of the character's three encounters"
    );
}
