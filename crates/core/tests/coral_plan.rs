//! The coral beam planner against the upstream JS.
//!
//! `fixtures/coral-plan.json` was produced by `script/coral-god-rank-oracle.mjs`
//! running `js/stats/systems/w5/coral-god-rank.js` on `fixtures/data.json`, so a
//! drift in the beam search, the offering model or the projection fails here.

use idleon_core::coral::{Action, Goal, Inputs, ProjectionKey, plan, plan_with, project, unlocks};
use idleon_core::save::import;
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/coral-plan.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/coral-plan.json"))
        .expect("the oracle is JSON")
}

fn cases() -> Vec<Value> {
    oracle()["cases"]
        .as_array()
        .expect("the oracle carries cases")
        .clone()
}

fn case(name: &str) -> Value {
    cases()
        .into_iter()
        .find(|case| case["name"] == name)
        .unwrap_or_else(|| panic!("the oracle carries the {name} case"))
}

fn inputs(case: &Value) -> Inputs {
    serde_json::from_value(case["inputs"].clone()).expect("the oracle inputs deserialize")
}

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn at(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

fn metrics_match(name: &str, side: &str, actual: &idleon_core::coral::Metrics, expected: &Value) {
    for (label, key, value) in [
        ("divinity points", "divinityPoints", actual.divinity_points),
        ("divinity exp", "divinityExp", actual.divinity_exp),
        ("class exp", "classExp", actual.class_exp),
        ("minor links", "minorLinks", actual.minor_links),
        ("blessing cap", "blessingCap", actual.blessing_cap),
        (
            "daily reef coral",
            "dailyReefCoral",
            actual.daily_reef_coral,
        ),
    ] {
        assert!(
            (value - at(expected, key)).abs() <= at(expected, key).abs().max(1.0) * 1e-9,
            "{name} {side} {label}: expected {}, got {value}",
            at(expected, key)
        );
    }
}

fn assert_case(case: &Value) {
    let name = case["name"].as_str().expect("a case name");
    let inputs = inputs(case);
    let plan = plan(&inputs);
    let expected = &case["plan"];

    assert_eq!(plan.goal.id(), expected["goal"].as_str().expect("a goal"));
    close(plan.budget, at(expected, "budget"));
    close(plan.spent, at(expected, "spent"));
    close(plan.remaining(), at(expected, "remaining"));
    close(
        plan.whole_plan_confidence,
        at(expected, "wholePlanConfidence"),
    );
    assert_eq!(
        plan.dominance_pruned as u64,
        expected["dominancePruned"].as_u64().expect("a count"),
        "{name}: dominance pruned"
    );

    let current = &expected["current"];
    let levels: [u32; 6] =
        serde_json::from_value(current["levels"].clone()).expect("current levels");
    assert_eq!(plan.current.levels, levels);
    assert_eq!(plan.current.god_rank, at(current, "godRank") as u32);
    metrics_match(name, "current", &plan.current.metrics, current);

    let final_state = &expected["final"];
    let levels: [u32; 6] =
        serde_json::from_value(final_state["levels"].clone()).expect("final levels");
    assert_eq!(plan.best.levels, levels);
    assert_eq!(plan.best.god_rank, at(final_state, "godRank") as u32);
    metrics_match(name, "final", &plan.best.metrics, final_state);

    let expected_actions = expected["actions"].as_array().expect("actions");
    assert_eq!(
        plan.actions.len(),
        expected_actions.len(),
        "{name}: actions"
    );
    for (actual, expected) in plan.actions.iter().zip(expected_actions) {
        match (actual, expected["type"].as_str().expect("an action type")) {
            (Action::CoralKid { index, level, cost }, "coral-kid") => {
                assert_eq!(*index as u64, expected["index"].as_u64().expect("index"));
                assert_eq!(*level as u64, expected["level"].as_u64().expect("level"));
                close(*cost, at(expected, "cost"));
            }
            (
                Action::GodOffering {
                    from_rank,
                    to_rank,
                    attempts,
                    expected_spend,
                    max_paid_spend,
                    confidence,
                    whole_plan_confidence,
                },
                "god-offering",
            ) => {
                assert_eq!(
                    *from_rank as u64,
                    expected["fromRank"].as_u64().expect("from")
                );
                assert_eq!(*to_rank as u64, expected["toRank"].as_u64().expect("to"));
                assert_eq!(
                    *attempts as u64,
                    expected["attempts"].as_u64().expect("attempts")
                );
                close(*expected_spend, at(expected, "expectedSpend"));
                close(*max_paid_spend, at(expected, "maxPaidSpend"));
                close(*confidence, at(expected, "confidence"));
                close(*whole_plan_confidence, at(expected, "wholePlanConfidence"));
            }
            (actual, kind) => panic!("{name}: expected a {kind} action, got {actual:?}"),
        }
    }

    let partial: Vec<String> =
        serde_json::from_value(expected["partial"].clone()).expect("partial tags");
    assert_eq!(plan.partial, partial, "{name}: partial");

    let expected_unlocks = expected["unlocks"].as_array().expect("unlocks");
    assert_eq!(plan.unlocks.len(), expected_unlocks.len());
    for (actual, expected) in plan.unlocks.iter().zip(expected_unlocks) {
        assert_eq!(
            actual.index as u64,
            expected["index"].as_u64().expect("index")
        );
        assert_eq!(
            actual.unlocked,
            expected["unlocked"].as_bool().expect("unlocked")
        );
        assert_eq!(actual.known, expected["known"].as_bool().expect("known"));
        assert_eq!(
            actual.reason.map(str::to_string),
            expected["reason"].as_str().map(str::to_string),
            "{name}: unlock {} reason",
            actual.index
        );
    }
}

#[test]
fn the_fixture_defaults_match_the_upstream_planner_inputs() {
    let imported = import(&fixture()).expect("the fixture imports");
    let from_save = Inputs::from_save(&imported.coral);
    assert_eq!(from_save, inputs(&case("fixture")));
}

#[test]
fn every_upstream_plan_matches_the_port() {
    for case in cases() {
        assert_case(&case);
    }
}

#[test]
fn the_offering_model_matches_upstream() {
    let oracle = oracle();
    let inputs = inputs(&case("fixture"));
    for row in oracle["offering"].as_array().expect("offering rows") {
        let rank = row["godRank"].as_u64().expect("a rank") as u32;
        let offer = idleon_core::coral::offer(&inputs, rank);
        close(offer.odds, at(row, "odds"));
        close(offer.cost, at(row, "cost"));
        close(offer.free_chance, 0.0);
        let confidence = &row["confidence"]["90"];
        assert_eq!(
            offer.attempts as u64,
            confidence["attempts"].as_u64().expect("attempts")
        );
        close(offer.expected_spend, at(confidence, "expectedSpend"));
        close(offer.max_paid_spend, at(confidence, "maxPaidSpend"));
    }
}

#[test]
fn the_plan_respects_the_budget_and_the_level_cap() {
    for case in cases() {
        let inputs = inputs(&case);
        let plan = plan(&inputs);
        assert!(
            plan.spent <= plan.budget + plan.budget.abs() * 1e-12,
            "{}: spent {} over budget {}",
            case["name"],
            plan.spent,
            plan.budget
        );
        assert!(
            plan.actions.len() <= inputs.max_levels as usize,
            "{}: {} actions over the {} cap",
            case["name"],
            plan.actions.len(),
            inputs.max_levels
        );
        assert!(plan.remaining() >= 0.0);
    }
}

#[test]
fn the_plan_is_reproducible_and_reports_every_step() {
    let inputs = inputs(&case("fixture"));
    let first = plan(&inputs);
    assert_eq!(first, plan(&inputs));

    let mut steps = 0;
    let reported = plan_with(&inputs, |progress| {
        steps += 1;
        assert!((0.0..=1.0).contains(&progress.fraction));
        true
    })
    .expect("the reporter continues");
    assert_eq!(reported, first);
    assert_eq!(steps, inputs.max_levels as usize);
}

#[test]
fn a_wider_beam_searches_deeper_and_finds_a_better_plan() {
    let wide = case("narrow-probe-wide");
    let narrow = case("narrow-probe-narrow");
    let wide_plan = plan(&inputs(&wide));
    let narrow_plan = plan(&inputs(&narrow));
    assert_ne!(wide_plan.actions, narrow_plan.actions);
    assert_eq!(wide_plan.current, narrow_plan.current);
    // The objective is the mean of the six ratios against the current state.
    let score = |plan: &idleon_core::coral::Plan| {
        plan.projection()
            .into_iter()
            .map(|row| row.projected / row.current)
            .sum::<f64>()
            / 6.0
    };
    assert!(
        score(&wide_plan) > score(&narrow_plan),
        "wide {} vs narrow {}",
        score(&wide_plan),
        score(&narrow_plan)
    );
}

#[test]
fn a_zero_level_cap_keeps_the_current_state() {
    let inputs = Inputs {
        max_levels: 0,
        ..inputs(&case("fixture"))
    };
    let plan = plan(&inputs);
    assert!(plan.actions.is_empty());
    assert_eq!(plan.current, plan.best);
    assert_eq!(plan.spent, 0.0);
}

#[test]
fn the_projection_has_a_row_per_metric_in_upstream_order() {
    let inputs = inputs(&case("fixture"));
    let plan = plan(&inputs);
    let rows = plan.projection();
    assert_eq!(
        rows.map(|row| row.key),
        ProjectionKey::ALL,
        "the projection order"
    );
    assert_eq!(rows, project(&plan.current.metrics, &plan.best.metrics));
}

#[test]
fn the_goal_selects_the_metric_the_plan_grows() {
    let inputs = Inputs {
        goal: Goal::ClassExp,
        ..inputs(&case("narrow-probe-wide"))
    };
    let class_exp = plan(&inputs);
    let expected = &case("narrow-probe-wide");
    assert_eq!(class_exp.goal, Goal::ClassExp, "{}", expected["name"]);
    assert!(class_exp.best.metrics.class_exp >= class_exp.current.metrics.class_exp);
}

#[test]
fn the_plan_flags_the_unlock_state_it_had_to_assume() {
    let inputs = Inputs {
        total_divinity_levels: None,
        ..inputs(&case("fixture"))
    };
    let plan = plan(&inputs);
    assert_eq!(plan.partial, ["Coral Kid unlock state"]);
    let rows = unlocks(&inputs);
    assert!(rows.iter().all(|row| row.unlocked && !row.known));
}
