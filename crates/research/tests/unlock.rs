//! Observation Unlocks: the port against the upstream calculator, on the
//! committed account fixture.
//!
//! `fixtures/obs-unlock.json` is written by `script/obs-unlock-oracle.mjs` from
//! the upstream `computeObsUnlockPriority` fed by its own save loader on
//! `fixtures/data.json`, so the ranking, the roll maths and the rate gain are
//! compared with the calculator's own output rather than numbers picked here.

use idleon_common::Control;
use idleon_research::model::Effort;
use idleon_research::unlock::{self, Filter};
use serde_json::Value;

fn fixture_save() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/obs-unlock.json"
    );
    let text = std::fs::read_to_string(path).expect("fixtures/obs-unlock.json is committed");
    serde_json::from_str(&text).expect("the oracle fixture is valid JSON")
}

fn close(left: f64, right: f64, what: &str) {
    let scale = left.abs().max(right.abs()).max(1.0);
    assert!(
        (left - right).abs() <= 1e-9 * scale,
        "{what}: port {left} vs upstream {right}"
    );
}

/// A rate or a number derived from one, within the fixture's documented
/// tolerance: the Tome star talent slot is not modelled, which moves the
/// research rate by 2e-5 relative (see `crates/import/tests/fixture.rs`).
fn close_rate(left: f64, right: f64, what: &str) {
    let scale = left.abs().max(right.abs()).max(1.0);
    assert!(
        (left - right).abs() <= 1e-4 * scale,
        "{what}: port {left} vs upstream {right}"
    );
}

#[test]
fn the_roll_state_matches_upstream() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let oracle = oracle();
    let outcome = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    let rolls = outcome.rolls;
    let summary = &oracle["summary"];
    close(
        rolls.min_roll,
        summary["minRoll"].as_f64().unwrap(),
        "min roll",
    );
    close(
        rolls.max_roll,
        summary["maxRoll"].as_f64().unwrap(),
        "max roll",
    );
    assert_eq!(
        rolls.per_day as u64,
        summary["rollsPerDay"].as_u64().unwrap(),
        "rolls per day"
    );
    assert_eq!(
        rolls.remaining.map(u64::from),
        Some(summary["rollsRemaining"].as_u64().unwrap()),
        "rolls left today"
    );
    assert_eq!(
        rolls.smart_eye as u64,
        summary["smartEyeLv"].as_u64().unwrap(),
        "Smart Eye level"
    );
    assert_eq!(
        rolls.smart_eye_cap as u64,
        summary["smartEyeCap"].as_u64().unwrap(),
        "Smart Eye cap"
    );
    close_rate(
        outcome.current_rate,
        summary["currentTotal"].as_f64().unwrap(),
        "current EXP/hr",
    );
    assert!(
        outcome.missing.is_empty(),
        "the fixture carries every roll field: {:?}",
        outcome.missing
    );
}

#[test]
fn the_priority_table_matches_upstream() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let oracle = oracle();
    let outcome = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    let expected = oracle["results"].as_array().expect("the upstream rows");
    assert_eq!(
        outcome.rows.len(),
        expected.len(),
        "one row per undiscovered observation"
    );
    for (row, want) in outcome.rows.iter().zip(expected) {
        let name = want["name"].as_str().unwrap();
        assert_eq!(
            row.obs,
            want["idx"].as_u64().unwrap() as usize,
            "{name} idx"
        );
        assert_eq!(row.name, name, "row names");
        close(
            row.threshold,
            want["rollThreshold"].as_f64().unwrap(),
            &format!("{name} roll threshold"),
        );
        close(
            row.required_level,
            want["requiredRLv"].as_f64().unwrap(),
            &format!("{name} research requirement"),
        );
        assert_eq!(
            row.usable,
            want["canUseNow"].as_bool().unwrap(),
            "{name} usability"
        );
        close(
            row.chance_today.unwrap(),
            want["pUnlockToday"].as_f64().unwrap(),
            &format!("{name} chance today"),
        );
        close(
            row.expected_days.unwrap(),
            want["expectedDays"].as_f64().unwrap(),
            &format!("{name} expected days"),
        );
        close_rate(
            row.exp_gain,
            want["expGain"].as_f64().unwrap(),
            &format!("{name} EXP/hr gain"),
        );
        close_rate(
            row.new_rate,
            want["newRate"].as_f64().unwrap(),
            &format!("{name} EXP/hr with it found"),
        );
        close_rate(
            row.score.unwrap(),
            want["score"].as_f64().unwrap(),
            &format!("{name} score"),
        );
    }
    // The upstream order is the table's order: usable rows first, then the score.
    assert_eq!(
        outcome
            .rows
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|row| row["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        "the priority ordering"
    );
}

#[test]
fn the_filters_split_the_usable_rows() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let outcome = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    let all = outcome.filtered(Filter::All).len();
    let usable = outcome.filtered(Filter::Usable).len();
    let waiting = outcome.filtered(Filter::Waiting).len();
    assert_eq!(all, outcome.rows.len());
    assert_eq!(usable + waiting, all);
    assert_eq!(usable, outcome.rows.iter().filter(|row| row.usable).count());
    // Every priority the table shows is drawn from the best score.
    let best = outcome.best_score();
    assert!(best > 0.0, "the fixture ranks at least one observation");
    assert!(
        outcome
            .rows
            .iter()
            .any(|row| row.priority(best) == unlock::Priority::Best)
    );
}

/// Without the save's roll fields the estimate reports them instead of
/// assuming a full day of rolls and a fresh streak.
#[test]
fn missing_roll_data_is_reported() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let mut context = imported.context.clone();
    context.observation_rolls_remaining = None;
    context.observation_failed_rolls = None;
    let outcome = unlock::analyze(
        &imported.simulation,
        &context,
        Effort::Balanced,
        &mut |_| Control::Continue,
    )
    .expect("the analysis finishes");
    assert_eq!(outcome.missing.len(), 2, "{:?}", outcome.missing);
    assert!(outcome.missing[0].starts_with("Research[7][2]: "));
    assert!(outcome.missing[1].starts_with("OptLacc[514]: "));
    assert!(
        outcome.rows.iter().all(|row| row.chance_today.is_none()
            && row.expected_days.is_none()
            && row.score.is_none()),
        "no row guesses a probability without the roll data"
    );
    assert_eq!(outcome.best_score(), 0.0);
    assert!(
        outcome
            .rows
            .iter()
            .all(|row| row.priority(0.0) != unlock::Priority::Best)
    );
}

/// The run stops cleanly when the task runner asks it to.
#[test]
fn a_cancelled_run_stores_nothing() {
    let imported = idleon_import::import(&fixture_save()).expect("the fixture imports");
    let stopped = unlock::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Quick,
        &mut |_| Control::Stop,
    );
    assert!(stopped.is_none());
}
