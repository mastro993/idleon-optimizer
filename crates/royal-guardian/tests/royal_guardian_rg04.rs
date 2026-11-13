//! RG-04: the optimizer, the unit EXP plan and the shelf plan.
//!
//! The optimizer's starting figures and the unit EXP plan are compared against
//! the upstream JS on `fixtures/royal-guardian-plan.json`, a small save whose
//! cross-system inputs are all zero, so both sides read the same rows without an
//! override. Every plan is also proved against the sandbox: a recommendation's
//! moves replay to its state, a unit EXP step only retargets a slot the outpost
//! fields, and a shelf plan's purchases are the ones its balance paid for, in
//! order.

use idleon_import::import;
use idleon_royal_guardian::{
    Caps, Move, Objective, Request, RoyalGuardian, ShelfAction, ShelfRequest, UnitExpRequest,
    UnitTargets, sandbox, validate,
};
use serde_json::Value;

fn plan_save() -> RoyalGuardian {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/royal-guardian-plan.json"
    );
    let save = std::fs::read_to_string(path).expect("the plan save is committed");
    import(&save)
        .expect("the plan save imports")
        .royal
        .expect("the plan save carries the Royal Guardian rows")
}

fn fixture() -> RoyalGuardian {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&save)
        .expect("the fixture imports")
        .royal
        .expect("the fixture carries the Royal Guardian rows")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/royal-guardian.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/royal-guardian.json"))
        .expect("the oracle is JSON")
}

/// Compare a figure the oracle printed, naming it when it drifts.
#[track_caller]
fn near(label: &str, got: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (got - expected).abs() <= tolerance,
        "{label}: got {got}, upstream {expected}"
    );
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

/// The oracle's `reachedAt` object as the plan's target order.
fn reached(value: &Value) -> [Option<f64>; 3] {
    ["trading", "command", "military"].map(|key| value.get(key).and_then(Value::as_f64))
}

#[test]
fn the_kingdom_metrics_match_upstream() {
    let royal = plan_save();
    let oracle = oracle();
    let expected = &oracle["plan"]["metrics"];
    let metrics = royal.metrics();
    near(
        "resources per hour",
        metrics.resources_per_hour,
        number(expected, "resourcesPerHour"),
    );
    near(
        "rank EXP per hour",
        metrics.rank_exp_per_hour,
        number(expected, "rankExpPerHour"),
    );
    near("range", metrics.range, number(expected, "range"));
}

#[test]
fn the_armory_totals_match_upstream() {
    let royal = plan_save();
    let oracle = oracle();
    assert_eq!(
        royal.armory_unlocked_count(),
        number(&oracle["plan"]["armory"], "unlocked") as usize
    );
    near(
        "Armory levels",
        royal.armory_total_levels(),
        number(&oracle["plan"]["armory"], "total"),
    );
}

/// Compare one unit EXP plan against the oracle's figure for it.
#[track_caller]
fn compare_unit_exp(request: &UnitExpRequest, expected: &Value) {
    let royal = &request.royal;
    let plan = royal
        .plan_unit_exp(request, |_| idleon_common::Control::Continue)
        .expect("the targets are reachable");
    near("pool", plan.pool, number(expected, "pool"));
    near(
        "current traders",
        plan.current_traders,
        number(expected, "currentTraders"),
    );
    assert_eq!(
        plan.surveyors_unlocked,
        expected["surveyorsUnlocked"].as_bool().unwrap_or(false)
    );
    near(
        "banked hours",
        plan.banked_hours,
        number(expected, "bankedHours"),
    );
    for (bar, saved) in plan.saved_ranks.iter().enumerate() {
        near(
            &format!("saved rank {bar}"),
            *saved,
            expected["savedRanks"][bar].as_f64().unwrap(),
        );
    }
    let plan_expected = &expected["plan"];
    match (plan.plan.eta_hours, plan_expected["etaHours"].as_f64()) {
        (Some(got), Some(want)) => near("plan ETA", got, want),
        (None, None) => {}
        (got, want) => panic!("plan ETA: got {got:?}, upstream {want:?}"),
    }
    assert_eq!(
        plan.plan.status.id(),
        plan_expected["status"].as_str().unwrap()
    );
    assert_eq!(plan.plan.reached_at, reached(&plan_expected["reachedAt"]));
    for (bar, rank) in plan.plan.final_ranks.iter().enumerate() {
        near(
            &format!("final rank {bar}"),
            *rank,
            plan_expected["finalRanks"][bar].as_f64().unwrap(),
        );
    }
    let steps = plan_expected["steps"].as_array().expect("the step list");
    assert_eq!(plan.plan.steps.len(), steps.len(), "every step is compared");
    for (step, expected) in plan.plan.steps.iter().zip(steps) {
        near("step hour", step.hours, number(expected, "hours"));
        near("step traders", step.traders, number(expected, "traders"));
        near(
            "step surveyors",
            step.surveyors,
            number(expected, "surveyors"),
        );
        assert_eq!(step.reason.id(), expected["reason"].as_str().unwrap());
        near("step packed row", step.packed, number(expected, "packed"));
        let changes = expected["changes"].as_array().expect("the changes");
        assert_eq!(step.changes.len(), changes.len());
        for (change, expected) in step.changes.iter().zip(changes) {
            assert_eq!(change.slot, number(expected, "slot") as usize);
            assert_eq!(change.from, number(expected, "from") as usize);
            assert_eq!(change.to, number(expected, "to") as usize);
        }
    }
    match (
        plan.current.eta_hours,
        expected["current"]["etaHours"].as_f64(),
    ) {
        (Some(got), Some(want)) => near("saved split ETA", got, want),
        (None, None) => {}
        (got, want) => panic!("saved split ETA: got {got:?}, upstream {want:?}"),
    }
    assert_eq!(
        plan.current.status.id(),
        expected["current"]["status"].as_str().unwrap()
    );
    if let Some(saved) = expected["savedHours"].as_f64() {
        near("saved hours", plan.saved_hours.unwrap_or(f64::NAN), saved);
    }
    match (&plan.transients, expected.get("transients")) {
        (Some(transients), Some(expected)) => {
            for (kind, count) in &transients.counts {
                near(
                    &format!("transient {kind} count"),
                    *count,
                    expected["counts"][kind.to_string()].as_f64().unwrap(),
                );
            }
            let rows = expected["rows"].as_array().expect("the transient rows");
            assert_eq!(transients.rows.len(), rows.len());
            for (row, expected) in transients.rows.iter().zip(rows) {
                assert_eq!(row.kind, number(expected, "type") as i64);
                near(
                    "transient current",
                    row.current,
                    number(expected, "current"),
                );
                near(
                    "transient available",
                    row.available,
                    number(expected, "available"),
                );
                near("transient best", row.best, number(expected, "best"));
            }
        }
        (None, None) => {}
        _ => panic!("the transient pass differs from upstream"),
    }
    // Every step's changes are moves the game accepts, and the step's packed
    // row is the row they reach.
    let map = plan.map;
    let mut current = sandbox::Sandbox::new(royal);
    for step in &plan.plan.steps {
        assert!(
            (step.traders + step.surveyors - plan.pool).abs() < 1e-9,
            "every step keeps the movable pool whole"
        );
        for change in &step.changes {
            current = sandbox::apply(
                &current,
                &Move::Profession {
                    map,
                    slot: change.slot,
                    profession: change.to,
                },
            )
            .unwrap_or_else(|errors| panic!("{change:?} is legal: {errors:?}"));
        }
        near(
            "packed row after the step",
            current.state().map_cell_at(map, 11),
            step.packed,
        );
    }
}

#[test]
fn the_unit_exp_plan_matches_upstream() {
    let royal = plan_save();
    let oracle = oracle();
    let expected = &oracle["plan"]["unitExp"]["mixed"];
    let request = UnitExpRequest {
        royal,
        map: number(expected, "map") as usize,
        targets: UnitTargets {
            trading: Some(2),
            command: Some(2),
            military: None,
        },
        move_transients: true,
    };
    compare_unit_exp(&request, expected);
}

#[test]
fn a_single_target_plan_matches_upstream() {
    let royal = plan_save();
    let oracle = oracle();
    let expected = &oracle["plan"]["unitExp"]["trading"];
    let request = UnitExpRequest {
        royal,
        map: number(expected, "map") as usize,
        targets: UnitTargets {
            trading: Some(3),
            command: None,
            military: None,
        },
        move_transients: true,
    };
    compare_unit_exp(&request, expected);
}

#[test]
fn an_illegal_move_is_refused() {
    let royal = plan_save();
    let start = sandbox::Sandbox::new(&royal);
    // A Normal outpost may step to Support, not jump straight to Savage.
    let errors = sandbox::apply(&start, &Move::Type { map: 1, kind: 2 })
        .expect_err("a two-step type change is illegal");
    assert!(
        errors.iter().any(|error| error.contains("adjacent")),
        "the refusal names the step: {errors:?}"
    );
    // A slot past the assignable count holds no movable unit.
    let errors = sandbox::apply(
        &start,
        &Move::Profession {
            map: 2,
            slot: 6,
            profession: 0,
        },
    )
    .expect_err("an inactive slot holds no movable unit");
    assert!(
        errors.iter().any(|error| error.contains("inactive")),
        "the refusal names the slot: {errors:?}"
    );
}

#[test]
fn a_cancelled_unit_exp_run_stops() {
    let royal = plan_save();
    let request = UnitExpRequest {
        royal,
        map: 2,
        targets: UnitTargets {
            trading: Some(2),
            command: Some(2),
            military: None,
        },
        move_transients: true,
    };
    assert!(
        request
            .royal
            .plan_unit_exp(&request, |_| idleon_common::Control::Stop)
            .is_err(),
        "the split search stops at its first report"
    );
}

#[test]
fn a_cancelled_optimizer_run_stores_nothing() {
    let royal = plan_save();
    let mut request = Request::new(&royal, Objective::Balanced);
    request.caps = Caps {
        iterations: 2,
        candidates: 80,
        beam: 2,
    };
    assert!(
        royal
            .optimize(&request, |_| idleon_common::Control::Stop)
            .is_none(),
        "the run stops at its first report"
    );
}

#[test]
fn the_optimizer_recommends_a_legal_layout_that_replays() {
    let royal = fixture();
    let mut request = Request::new(&royal, Objective::Resources);
    request.caps = Caps {
        iterations: 2,
        candidates: 40,
        beam: 2,
    };
    let outcome = royal
        .optimize(&request, |_| idleon_common::Control::Continue)
        .expect("the run is not cancelled");
    let best = outcome.best().expect("a move improves the layout");
    let start = outcome.start;
    assert!(
        match outcome.objective {
            Objective::Resources => best.metrics.resources_per_hour >= start.resources_per_hour,
            Objective::Rank => best.metrics.rank_exp_per_hour >= start.rank_exp_per_hour,
            Objective::Balanced => {
                best.metrics.resources_per_hour
                    + best.metrics.rank_exp_per_hour
                    + best.metrics.range
                    >= start.resources_per_hour + start.rank_exp_per_hour + start.range
            }
        },
        "a recommendation never lowers the objective"
    );
    let mut replayed = sandbox::Sandbox::new(&royal);
    for change in &best.layout {
        replayed = sandbox::apply(&replayed, change)
            .unwrap_or_else(|errors| panic!("{change:?} is legal: {errors:?}"));
    }
    assert_eq!(
        replayed.state(),
        &best.state,
        "the layout edits reach the recommended state"
    );
    assert_eq!(validate(&replayed), Vec::<String>::new());
}

#[test]
fn the_shelf_plan_buys_the_levels_toward_the_next_shelf() {
    let royal = plan_save();
    let plan = royal
        .plan_shelf(&ShelfRequest::new(&royal, 3), |_| {
            idleon_common::Control::Continue
        })
        .expect("the plan completes");
    assert_eq!(
        plan.levels_to_next_threshold,
        number(&oracle()["plan"]["armory"], "levelsToNext") as usize,
    );
    assert_eq!(plan.purchases_needed, 3);
    assert!(
        !plan.complete,
        "three purchases do not reach the next shelf"
    );
    assert_eq!(
        plan.reason.as_deref(),
        Some("the purchase limit stopped the plan")
    );
    for step in &plan.steps {
        match &step.action {
            ShelfAction::Layout { moves, .. } => assert!(!moves.is_empty()),
            ShelfAction::Purchase {
                level_before,
                level_after,
                cost,
                ..
            } => {
                assert_eq!(*level_after, level_before + 1.0);
                assert!(*cost >= 0.0);
            }
            ShelfAction::Reset { .. } => {}
            ShelfAction::Collect { currency, .. } => {
                assert!(!currency.is_empty(), "a collect step carries its gains");
            }
        }
    }
    let replayed = plan
        .replay(&royal)
        .unwrap_or_else(|errors| panic!("the plan replays: {errors:?}"));
    // The layout and purchase steps reach the projected Armory, balances and
    // rows; node progress is the projection's own drain, not a step.
    assert_eq!(validate(&replayed), Vec::<String>::new());
    assert_eq!(
        replayed.state().armory_total_levels(),
        royal.armory_total_levels() + 3.0,
        "three purchases add three levels"
    );
    for index in 0..83 {
        assert_eq!(
            replayed.state().armory_level(index),
            plan.final_state.armory_level(index),
            "Armory {index} reaches the projected level"
        );
    }
    for slot in -1..32 {
        assert_eq!(
            replayed.state().currency_balance(slot),
            plan.final_state.currency_balance(slot),
            "slot {slot} reaches the projected balance"
        );
    }
    for map in 0..plan.final_state.outpost_count() {
        for cell in 0..13 {
            assert_eq!(
                replayed.state().map_cell_at(map, cell),
                plan.final_state.map_cell_at(map, cell),
                "map {map} cell {cell} reaches the projected value"
            );
        }
    }
}

#[test]
fn a_shelf_plan_stops_when_the_purchase_limit_is_reached() {
    let royal = plan_save();
    let plan = royal
        .plan_shelf(&ShelfRequest::new(&royal, 1), |_| {
            idleon_common::Control::Continue
        })
        .expect("the plan completes");
    assert_eq!(plan.purchases_needed, 1);
    assert!(!plan.complete);
    assert!(plan.eta_hours.is_finite());
}
