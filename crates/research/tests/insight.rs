//! Insight ROI: the row invariants.

use idleon_common::Control;
use idleon_research::insight::{self, Sort};
use idleon_research::model::{
    Effort, GridState, Mag, MagKind, Observations, SimContext, Simulation,
};
use idleon_research::sim;

fn close(left: f64, right: f64, what: &str) {
    let scale = left.abs().max(right.abs()).max(1.0);
    assert!(
        (left - right).abs() <= 1e-9 * scale,
        "{what}: {left} vs {right}"
    );
}

/// A state with a handful of observations, so the whole analysis runs fast.
fn small() -> (Simulation, SimContext) {
    let mut grid = GridState::seeded(1);
    grid.set(31, 2);
    grid.set(91, 3);
    grid.set(92, 1);
    grid.set(93, 2);
    grid.set(72, 2);
    let mut observations = Observations::all_at(1);
    observations
        .found
        .iter_mut()
        .skip(6)
        .for_each(|found| *found = false);
    observations.insight_level[0] = 3;
    observations.insight_exp[0] = 400.0;
    let simulation = Simulation {
        research_level: 70,
        research_exp: 0.0,
        grid,
        observations,
        mags: vec![
            Mag::new(MagKind::Regular, Some(0)),
            Mag::new(MagKind::Regular, Some(1)),
            Mag::new(MagKind::Regular, Some(2)),
            Mag::new(MagKind::Monocle, Some(0)),
            Mag::new(MagKind::Monocle, Some(1)),
            Mag::new(MagKind::Kaleidoscope, Some(2)),
        ],
        ..Simulation::default()
    };
    (simulation, SimContext::default())
}

#[test]
fn every_usable_observation_gets_a_row_with_every_column() {
    let (simulation, context) = small();
    let outcome = insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    })
    .expect("a quick run is not cancelled");
    let usable = sim::available_slots(simulation.research_level, &simulation.observations);
    assert!(!usable.is_empty());
    assert_eq!(outcome.rows.len(), usable.len());
    assert_eq!(outcome.monocles, 2);
    // The stored order is the default sort: the best break-even first.
    for pair in outcome.rows.windows(2) {
        assert!(
            pair[0].scenarios[0].break_even_hours <= pair[1].scenarios[0].break_even_hours,
            "rows are stored in break-even order"
        );
    }
    // Each row carries the one- and two-level scenarios in that order.
    for row in &outcome.rows {
        assert_eq!(
            row.after_overlay.len(),
            simulation.overlay().len(),
            "{} keeps a full post-grind overlay",
            row.name
        );
        assert_eq!(
            row.scenarios.map(|scenario| scenario.level_gain),
            [1, 2],
            "{} lists +1 then +2",
            row.name
        );
        assert!(!row.name.is_empty());
    }
    // Every sort mode reorders the same rows.
    for sort in Sort::ALL {
        let sorted: Vec<usize> = outcome.sorted(sort).iter().map(|row| row.obs).collect();
        assert_eq!(sorted.len(), outcome.rows.len());
    }
}

#[test]
fn every_scenario_is_built_from_the_insight_formulas() {
    let (simulation, context) = small();
    let quick = match insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    }) {
        Some(outcome) => outcome,
        None => panic!("a quick run is not cancelled"),
    };
    assert!(quick.base_rate > 0.0, "the baseline earns EXP");
    for row in &quick.rows {
        let one = &row.scenarios[0];
        let two = &row.scenarios[1];
        assert_eq!(one.level_gain, 1);
        assert_eq!(two.level_gain, 2);
        if one.worth {
            assert!(one.rate_gain > 0.0 && one.break_even_hours.is_finite());
        }
        assert_eq!(
            one.total_exp_lost,
            if one.exp_lost_per_hour > 0.0 {
                one.exp_lost_per_hour * one.grind_hours
            } else {
                0.0
            }
        );
        assert_eq!(
            one.break_even_hours,
            one.grind_hours + one.recoup_hours,
            "break-even is the grind plus the recoup"
        );
        // The gain is the post-grind rate against the same baseline.
        assert!(
            one.post_rate >= 0.0 && one.grind_rate >= 0.0,
            "{} reports non-negative rates",
            row.name
        );
        if one.rate_gain > 0.0 {
            assert_eq!(one.recoup_hours, one.total_exp_lost / one.rate_gain);
        } else {
            assert!(one.recoup_hours.is_infinite());
        }
        // A two-level grind never finishes sooner than a one-level grind.
        assert!(
            two.grind_hours >= one.grind_hours || two.grind_hours.is_infinite(),
            "{}: +2 takes longer than +1",
            row.name
        );
        // The grind hours are the insight requirement over the insight rate.
        let level = simulation.observations.insight(row.obs);
        let banked = simulation.observations.insight_exp[row.obs];
        if one.insight_rate > 0.0 {
            close(
                one.grind_hours,
                (sim::insight_exp_required(row.obs, level) - banked).max(0.0) / one.insight_rate,
                &format!("{}: the one-level grind", row.name),
            );
        } else {
            assert!(one.grind_hours.is_infinite());
        }
    }
}

#[test]
fn no_monocle_means_no_rows() {
    let (mut simulation, context) = small();
    simulation.mags.retain(|mag| mag.kind != MagKind::Monocle);
    let outcome = insight::analyze(&simulation, &context, Effort::Quick, &mut |_| {
        Control::Continue
    })
    .expect("a quick run is not cancelled");
    assert_eq!(outcome.monocles, 0);
    assert!(outcome.rows.is_empty());
}

#[test]
fn a_cancelled_run_reports_nothing() {
    let (simulation, context) = small();
    assert!(
        insight::analyze(&simulation, &context, Effort::Quick, &mut |_| Control::Stop).is_none(),
        "a cancelled analysis stores no result"
    );
}

#[test]
#[ignore = "long simulation: full-account insight ROI"]
fn the_account_fixture_reports_every_usable_observation() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let save = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_import::import(&save).expect("the fixture imports");
    let outcome = insight::analyze(
        &imported.simulation,
        &imported.context,
        Effort::Quick,
        &mut |_| Control::Continue,
    )
    .expect("the run is not cancelled");
    let usable = sim::available_slots(
        imported.simulation.research_level,
        &imported.simulation.observations,
    );
    assert_eq!(outcome.rows.len(), usable.len());
    assert!(outcome.base_rate > 0.0, "the fixture earns EXP");
    let rows: std::collections::BTreeSet<usize> = outcome.rows.iter().map(|row| row.obs).collect();
    assert_eq!(rows, usable.into_iter().collect());
}
