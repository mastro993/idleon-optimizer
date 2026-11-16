//! RG-04: the optimizer, the sandbox and the unit EXP plan on the full fixture.
//!
//! A recommendation is proved against the sandbox: its moves replay to its
//! state. The sandbox refuses illegal moves, and every planner stops when the
//! caller cancels it.

use idleon_import::import;
use idleon_royal_guardian::{
    Caps, Move, Objective, Request, RoyalGuardian, UnitExpRequest, UnitTargets, sandbox, validate,
};

fn fixture() -> RoyalGuardian {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&save)
        .expect("the fixture imports")
        .royal
        .expect("the fixture carries the Royal Guardian rows")
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
fn an_illegal_move_is_refused() {
    let royal = fixture();
    assert_eq!(royal.allowed_types(), vec![0, 1, 2]);
    assert_eq!(royal.outpost_type(1), 0.0, "map 1 holds a Normal outpost");
    assert!(royal.assignable_slots(2) <= 6, "map 2 has slot 6 locked");
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
    let royal = fixture();
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
    let royal = fixture();
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
