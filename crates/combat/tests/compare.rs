//! The comparison against the upstream cell and over real encounter runs.
//!
//! `fixtures/combat-compare.json` is written by `script/combat-compare-oracle.mjs`
//! from `combat-sim-setup-compare.js`, so a case that disagrees here is a port
//! bug. The run tests use the stand-in player on two extracted maps: the same
//! map for two loadouts (which the run ignores by design) and two maps.

use idleon_combat::compare::{self, Comparison, Metric, Plan, RowSpec};
use idleon_combat::encounter::{self, Player};
use idleon_combat::loadout::Totals;
use idleon_common::{Control, Progress};
use serde_json::Value as Json;
use std::collections::BTreeMap;

fn fixture() -> Json {
    serde_json::from_str(include_str!("../../../fixtures/combat-compare.json"))
        .expect("fixtures/combat-compare.json is valid")
}

fn spec(character: usize, loadout: &str, map: usize, weapon_power: f64) -> RowSpec {
    RowSpec {
        character,
        character_name: if character == 0 { "Ana" } else { "Bo" }.into(),
        loadout: loadout.into(),
        map,
        totals: Totals {
            weapon_power,
            defence: 5.0,
            main_stat: "STR",
            main_value: 4.0,
            equipped: 2,
            slots: 23,
            talents: 7,
            stats: BTreeMap::new(),
        },
    }
}

fn plan() -> Plan {
    Plan {
        player: Player::StandIn {
            kill_every_ms: 1_500,
        },
        duration_ms: 120_000,
        progress: idleon_combat::progress::Config::default(),
        grants: idleon_combat::progress::Grants::default(),
        rows: vec![
            spec(0, "Imported save", 1, 0.0),
            spec(0, "Working edits", 1, 10.0),
            spec(1, "Imported save", 2, 0.0),
        ],
    }
}

fn run(plan: &Plan) -> Comparison {
    compare::run(plan, &mut |_| Control::Continue)
        .expect("the plan is runnable")
        .expect("the run reaches its end")
}

#[test]
fn every_cell_matches_the_upstream_module() {
    let fixture = fixture();
    let cases = fixture["cases"].as_array().expect("cases");
    for case in cases {
        let name = case["name"].as_str().expect("a case name");
        let rates = |runs: &Json| -> Vec<f64> {
            runs.as_array()
                .expect("runs")
                .iter()
                .map(|run| {
                    let milliseconds = run["timeMs"].as_f64().expect("timeMs");
                    // `_runValue` keeps a run with hours > 0 and returns null
                    // otherwise; NaN is the port's missing value.
                    if milliseconds > 0.0 {
                        run["kills"].as_f64().expect("kills") * 3_600_000.0 / milliseconds
                    } else {
                        f64::NAN
                    }
                })
                .collect()
        };
        let values = rates(&case["values"]);
        let baseline = rates(&case["baseline"]);
        let cell = compare::cell(&values, &baseline);
        let expected = &case["expected"];
        let near = |actual: Option<f64>, key: &str| match (actual, expected[key].as_f64()) {
            (None, None) => {}
            (Some(actual), Some(want)) => assert!(
                (actual - want).abs() < 1e-9,
                "{name}: {key} {actual} vs {want}"
            ),
            (actual, want) => panic!("{name}: {key} {actual:?} vs {want:?}"),
        };
        near(cell.mean, "mean");
        near(cell.delta, "delta");
        near(cell.delta_pct, "deltaPct");
        near(cell.se, "se");
        near(cell.base_mean, "baseMean");
        assert_eq!(
            cell.n,
            expected["n"].as_u64().expect("n") as usize,
            "{name}: n"
        );
    }
}

#[test]
fn two_loadouts_share_a_map_outcome_and_differ_in_their_gear_totals() {
    let comparison = run(&plan());
    assert_eq!(comparison.rows.len(), 3);
    assert!(comparison.rows[0].outcome.kills > 0);
    assert_eq!(
        comparison.rows[0].outcome, comparison.rows[1].outcome,
        "the stand-in run ignores the loadout, so one map reuses its outcome"
    );
    assert_eq!(comparison.rows[1].map_name, comparison.rows[0].map_name);

    let kills = comparison.cell(1, Metric::KillsPerHour);
    assert_eq!(kills.delta, Some(0.0));
    assert_eq!(kills.delta_pct, Some(0.0));
    let weapon = comparison.cell(1, Metric::WeaponPower);
    assert_eq!(weapon.mean, Some(10.0));
    assert_eq!(weapon.base_mean, Some(0.0));
    assert_eq!(weapon.delta, Some(10.0));
    assert_eq!(weapon.delta_pct, None, "a zero baseline has no percentage");
    assert_eq!(comparison.cell(0, Metric::KillsPerHour).delta, None);
}

#[test]
fn the_interval_metric_has_no_value_for_a_run_without_kills() {
    let idle = Plan {
        player: Player::Idle,
        duration_ms: 60_000,
        progress: idleon_combat::progress::Config::default(),
        grants: idleon_combat::progress::Grants::default(),
        rows: vec![
            spec(0, "Imported save", 1, 0.0),
            spec(0, "Imported save", 2, 0.0),
        ],
    };
    let comparison = run(&idle);
    assert!(comparison.rows.iter().all(|row| row.outcome.kills == 0));
    let interval = comparison.cell(1, Metric::KillIntervalSeconds);
    assert_eq!(interval.mean, None);
    assert_eq!(interval.n, 0);
    assert_eq!(interval.delta, None);
}

#[test]
fn the_document_carries_every_row_and_its_delta() {
    let comparison = run(&plan());
    let document = compare::document(&comparison);
    let text = compare::document_json(&comparison);
    assert_eq!(
        serde_json::from_str::<Json>(&text).expect("the export is JSON"),
        document
    );
    assert_eq!(document["section"], "combat-sim");
    assert_eq!(document["kind"], "comparison");
    assert_eq!(document["baseline"], 0);
    assert_eq!(document["player"]["id"], "standIn");
    assert_eq!(document["player"]["killEveryMs"], 1_500);
    let rows = document["rows"].as_array().expect("rows");
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1]["characterName"], "Ana");
    assert_eq!(rows[1]["loadout"], "Working edits");
    assert_eq!(rows[1]["map"], 1);
    assert_eq!(rows[1]["metrics"]["killsPerHour"]["delta"], 0.0);
    assert_eq!(rows[1]["metrics"]["weaponPower"]["delta"], 10.0);
    assert_eq!(rows[2]["map"], 2);
    assert!(rows[2]["outcome"]["kills"].as_u64().expect("kills") > 0);
    assert!(rows[1]["outcome"]["progress"]["forgeSeconds"].is_number());
    assert!(rows[1]["outcome"]["progress"]["bars"].is_object());
    assert!(
        document["metrics"]
            .as_array()
            .expect("metric list")
            .iter()
            .any(|metric| metric["key"] == "killsPerHour")
    );
}

#[test]
fn a_cancelled_comparison_stores_nothing() {
    let outcome = compare::run(&plan(), &mut |_| Control::Stop).expect("the plan is runnable");
    assert!(outcome.is_none());
}

#[test]
fn a_plan_needs_two_runnable_setups() {
    let single = Plan {
        rows: vec![spec(0, "Imported save", 1, 0.0)],
        ..plan()
    };
    assert_eq!(
        single.validate(),
        Err("Pick at least two character, loadout and map setups.".into())
    );
    let mut unsupported = plan();
    unsupported.rows[1].map = 0;
    assert!(unsupported.validate().is_err());
    assert_eq!(
        compare::run(&single, &mut |_| Control::Continue),
        Err("Pick at least two character, loadout and map setups.".into())
    );
}

#[test]
fn progress_reports_each_row() {
    let mut seen: Vec<Progress> = Vec::new();
    compare::run(&plan(), &mut |progress| {
        seen.push(progress);
        Control::Continue
    })
    .expect("runs")
    .expect("finishes");
    assert!(
        seen.iter()
            .any(|progress| progress.message.starts_with("Row 2 of 3")),
        "{seen:?}"
    );
    let last = seen.last().expect("progress");
    assert_eq!(last.fraction, 1.0);
}

#[test]
fn the_maps_the_comparison_runs_are_the_encounter_ones() {
    assert!(encounter::map(1).is_some());
    assert!(encounter::map(2).is_some());
}
