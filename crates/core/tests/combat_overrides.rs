//! The mechanics overrides against upstream's own catalog and outcomes.
//!
//! `fixtures/combat-overrides.json` and `crates/core/data/combat-overrides.json`
//! are written together by `script/combat-overrides-oracle.mjs` from
//! `js/stats/systems/common/combat-sim-toggles.js`, so a case that disagrees here
//! is a port bug, not a stale expectation.

use idleon_core::combat::overrides::{
    self, Entries, Mode, Overrides, Rule, SavedValue, State, Value,
};
use serde_json::Value as Json;

fn fixture() -> Json {
    serde_json::from_str(include_str!("../../../fixtures/combat-overrides.json"))
        .expect("fixtures/combat-overrides.json is valid")
}

fn entries(fixture: &Json) -> Entries {
    serde_json::from_value(fixture["entries"].clone()).expect("the saved inputs parse")
}

fn to_json<T: serde::Serialize>(value: &T) -> Json {
    serde_json::to_value(value).expect("the state serializes")
}

/// The fixture's numbers parse as integers while the port's are `f64`, so both
/// sides are compared as the port's own types.
fn expected_state(case: &Json) -> State {
    serde_json::from_value(case["normalized"].clone()).expect("the normalized state parses")
}

fn expected_overrides(value: &Json) -> Overrides {
    serde_json::from_value(value.clone()).expect("the resolved overrides parse")
}

#[test]
fn the_catalog_keeps_every_upstream_mechanic_and_group() {
    let fixture = fixture();
    let groups: Vec<String> = fixture["groups"]
        .as_array()
        .expect("groups")
        .iter()
        .map(|group| group.as_str().expect("a group id").to_string())
        .collect();
    assert_eq!(overrides::groups(), groups.as_slice());
    assert_eq!(overrides::data().toggles.len(), 68);

    let mut ids: Vec<&str> = overrides::data()
        .toggles
        .iter()
        .map(|toggle| toggle.id.as_str())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), 68, "every mechanic has its own id");

    for toggle in &overrides::data().toggles {
        assert!(
            groups.contains(&toggle.group),
            "{} is in an unknown group {}",
            toggle.id,
            toggle.group
        );
        assert!(!toggle.keys.is_empty(), "{} owns no input", toggle.id);
        assert!(!toggle.modes().is_empty(), "{} has no mode", toggle.id);
        assert!(!toggle.label.is_empty(), "{} has no label", toggle.id);
        assert!(!toggle.desc.is_empty(), "{} has no description", toggle.id);
        assert_eq!(
            overrides::input_key(&toggle.id),
            Some(toggle.key()),
            "{} addresses its own first input",
            toggle.id
        );
    }
}

#[test]
fn the_flag_mechanics_keep_their_own_keys() {
    let fixture = fixture();
    let flags = fixture["cases"]
        .as_array()
        .expect("cases")
        .iter()
        .find(|case| case["name"] == "flags-and-tris")
        .expect("the flags case");
    let saved = entries(&fixture);
    let mut state = State::new();
    state.set("compass3", Rule::new(Mode::On));
    state.set("autoLoot", Rule::new(Mode::Off));
    let resolved = state.build(&saved);

    // Every medallion the upstream list carries lands in the override unchanged.
    let expected = expected_overrides(&flags["overrides"]);
    assert_eq!(resolved["compass3"], expected["compass3"]);
    assert_eq!(resolved["ola83"], expected["ola83"]);
    assert!(!resolved.contains_key("autoLoot"));
}

#[test]
fn every_upstream_case_resolves_the_same() {
    let fixture = fixture();
    let saved = entries(&fixture);
    for case in fixture["cases"].as_array().expect("cases") {
        let name = case["name"].as_str().expect("a case name");
        let state = State::from_json(&case["state"]);
        let normalized = state.normalize();
        assert_eq!(normalized, expected_state(case), "{name}: normalized state");
        assert_eq!(
            state.len(),
            case["active"].as_u64().expect("active") as usize,
            "{name}: active count"
        );
        assert_eq!(
            state.needs_base(),
            case["needsBase"].as_bool().expect("needsBase"),
            "{name}: needs a saved input"
        );
        assert_eq!(
            state.build(&saved),
            expected_overrides(&case["overrides"]),
            "{name}: resolved overrides"
        );
        // Upstream keeps the order the rules were edited in; the editor lists
        // chips in catalog order, so the labels compare as sets.
        let mut described: Vec<String> = case["described"]
            .as_array()
            .expect("described")
            .iter()
            .map(|line| line.as_str().expect("a line").to_string())
            .collect();
        let mut chips = state.describe();
        described.sort();
        chips.sort();
        assert_eq!(chips, described, "{name}: chip labels");
    }
}

#[test]
fn off_and_set_apply_without_the_saved_inputs() {
    let mut state = State::new();
    state.set("dropRate", Rule::new(Mode::Off));
    state.set("damageMax", Rule::with_value(Mode::Set, 1234.0));
    state.set("killPerKill", Rule::with_value(Mode::Scale, 2.0));
    state.set("respawnBonusPct", Rule::new(Mode::Off));

    let resolved = state.build(&Entries::new());
    assert_eq!(resolved["dropRate"], Value::Number(1.0));
    assert_eq!(resolved["damageMax"], Value::Number(1234.0));
    assert_eq!(
        resolved["respawnBonusPct"],
        Value::Object(Default::default())
    );
    // A scale without a base has nothing to multiply, exactly upstream.
    assert!(!resolved.contains_key("killPerKill"));
    assert!(state.needs_base());
}

#[test]
fn reset_restores_the_saved_inputs() {
    let mut state = State::new();
    state.set("dropRate", Rule::new(Mode::Off));
    state.set("monsterCash", Rule::with_value(Mode::Scale, 3.0));
    let saved = Entries::from([("monsterCash".to_string(), SavedValue::Number(2.0))]);
    assert_eq!(
        state.resolve(&saved, &Overrides::default()),
        Overrides::from([
            ("dropRate".to_string(), Value::Number(1.0)),
            ("monsterCash".to_string(), Value::Number(6.0)),
        ])
    );

    state.reset();
    assert!(state.is_empty());
    assert!(state.resolve(&saved, &Overrides::default()).is_empty());
    assert!(state.describe().is_empty());
}

#[test]
fn an_explicit_run_override_wins_over_a_mechanic() {
    let mut state = State::new();
    state.set("dropRate", Rule::new(Mode::Off));
    let base = Overrides::from([("dropRate".to_string(), Value::Number(9.0))]);
    let resolved = state.resolve(&Entries::new(), &base);
    assert_eq!(resolved["dropRate"], Value::Number(9.0));

    // Without overrides the run's own inputs are the answer.
    assert_eq!(State::new().resolve(&Entries::new(), &base), base);
}

#[test]
fn a_project_state_round_trips_and_drops_its_invalid_rows() {
    let mut state = State::new();
    state.set("dropRate", Rule::new(Mode::Off));
    state.set("killPerKill", Rule::with_value(Mode::Scale, 2.0));
    state.set("raw:filterSlots", Rule::with_value(Mode::Set, 9.0));
    let json = to_json(&state);
    let reloaded: State = serde_json::from_value(json).expect("the state reloads");
    assert_eq!(reloaded, state);

    let dirty: State = serde_json::from_value(serde_json::json!({
        "nope": { "mode": "off" },
        "dropRate": { "mode": "on" },
        "monsterCash": { "mode": "set" },
        "raw:": { "mode": "set", "value": 1 },
        "extraMushKillz": { "mode": "peak" },
        "damageMax": { "mode": "set", "value": 1234 },
    }))
    .expect("the invalid rows do not fail the load");
    assert_eq!(dirty.len(), 1);
    assert_eq!(
        dirty.rule("damageMax"),
        Some(Rule::with_value(Mode::Set, 1234.0))
    );
}

#[test]
fn a_picked_mode_seeds_its_value() {
    let current = Rule::with_value(Mode::Set, 5.0);
    assert_eq!(
        Rule::seeded(Mode::Off, Some(&current), Some(7.0)),
        Rule::new(Mode::Off)
    );
    assert_eq!(
        Rule::seeded(Mode::Set, Some(&current), Some(7.0)),
        Rule::with_value(Mode::Set, 5.0)
    );
    assert_eq!(
        Rule::seeded(Mode::Set, None, Some(7.0)),
        Rule::with_value(Mode::Set, 7.0)
    );
    assert_eq!(
        Rule::seeded(Mode::Set, Some(&Rule::new(Mode::Off)), None),
        Rule::with_value(Mode::Set, 0.0)
    );
    assert_eq!(
        Rule::seeded(Mode::Scale, Some(&current), None),
        Rule::with_value(Mode::Scale, 1.0)
    );
}

#[test]
fn raw_input_names_read_as_the_editor_shows_them() {
    assert_eq!(overrides::raw_name("filterSlots"), "Filter Slots");
    assert_eq!(overrides::raw_name("talent468_2"), "Speedy Book (value 2)");
    assert_eq!(
        overrides::raw_name("talentEnh29"),
        "Rares Everywhere! (enhanced)"
    );
    assert_eq!(
        overrides::raw_name("talent29Level"),
        "Rares Everywhere! level"
    );
    assert_eq!(overrides::raw_name("buff29"), "Rares Everywhere! buff");

    let mut state = State::new();
    state.set("raw:filterSlots", Rule::with_value(Mode::Set, 9.0));
    assert_eq!(state.describe(), ["Filter Slots = 9"]);
}
