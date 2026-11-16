//! The coral beam planner on the fixture save and a small probe account.

mod common;

use common::fixture;
use idleon_coral::{Goal, Inputs, ProjectionKey, plan, plan_with, project, unlocks};
use idleon_import::import;

/// The planner inputs `fixtures/data.json` fills.
fn fixture_inputs() -> Inputs {
    let imported = import(&fixture()).expect("the fixture imports");
    Inputs::from_save(&imported.coral)
}

/// A small account where the beam width changes the plan.
fn probe(beam_size: u32) -> Inputs {
    Inputs {
        budget: 1e9,
        max_levels: 8,
        beam_size,
        reef_upgrade_count: 10.0,
        mine_floor: 0.0,
        vault76_bonus: 0.0,
        tier: 0,
        levels: [10; 6],
        total_divinity_levels: Some(10_000),
        ..fixture_inputs()
    }
}

/// Every planner input this file exercises, by name.
fn cases() -> [(&'static str, Inputs); 4] {
    [
        ("fixture", fixture_inputs()),
        (
            "fixture-narrow-beam",
            Inputs {
                beam_size: 1,
                ..fixture_inputs()
            },
        ),
        ("narrow-probe-wide", probe(100)),
        ("narrow-probe-narrow", probe(1)),
    ]
}

#[test]
fn the_plan_respects_the_budget_and_the_level_cap() {
    for (name, inputs) in cases() {
        let plan = plan(&inputs);
        assert!(
            plan.spent <= plan.budget + plan.budget.abs() * 1e-12,
            "{}: spent {} over budget {}",
            name,
            plan.spent,
            plan.budget
        );
        assert!(
            plan.actions.len() <= inputs.max_levels as usize,
            "{}: {} actions over the {} cap",
            name,
            plan.actions.len(),
            inputs.max_levels
        );
        assert!(plan.remaining() >= 0.0);
    }
}

#[test]
fn the_plan_is_reproducible_and_reports_every_step() {
    let inputs = fixture_inputs();
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
    let wide_plan = plan(&probe(100));
    let narrow_plan = plan(&probe(1));
    assert_ne!(wide_plan.actions, narrow_plan.actions);
    assert_eq!(wide_plan.current, narrow_plan.current);
    // The objective is the mean of the six ratios against the current state.
    let score = |plan: &idleon_coral::Plan| {
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
        ..fixture_inputs()
    };
    let plan = plan(&inputs);
    assert!(plan.actions.is_empty());
    assert_eq!(plan.current, plan.best);
    assert_eq!(plan.spent, 0.0);
}

#[test]
fn the_projection_has_a_row_per_metric_in_display_order() {
    let inputs = fixture_inputs();
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
        ..probe(100)
    };
    let class_exp = plan(&inputs);
    assert_eq!(class_exp.goal, Goal::ClassExp);
    assert!(class_exp.best.metrics.class_exp >= class_exp.current.metrics.class_exp);
}

#[test]
fn the_plan_flags_the_unlock_state_it_had_to_assume() {
    let inputs = Inputs {
        total_divinity_levels: None,
        ..fixture_inputs()
    };
    let plan = plan(&inputs);
    assert_eq!(plan.partial, ["Coral Kid unlock state"]);
    let rows = unlocks(&inputs);
    assert!(rows.iter().all(|row| row.unlocked && !row.known));
}
