//! Invariants every optimizer run must hold.

use idleon_core::model::{
    Effort, GridState, MagKind, Observations, OptimizeRequest, SimContext, Simulation, Target,
};
use idleon_core::{game, optimize, shape, sim};
use std::time::Instant;

fn seeded(level: u32, research_level: u32, glasses: usize) -> (Simulation, SimContext) {
    let _ = level;
    let ctx = SimContext::default();
    let mut simulation = Simulation {
        research_level,
        grid: GridState::seeded(1),
        observations: Observations::all_at(1),
        ..Simulation::default()
    };
    simulation.grid.set(50, 1);
    let mut usable: Vec<usize> = (0..game().observations.len())
        .filter(|obs| sim::observation_usable(*obs, research_level, &simulation.observations))
        .collect();
    usable.sort_by(|a, b| sim::obs_base_exp(*b).total_cmp(&sim::obs_base_exp(*a)));
    // Seeded legally: no observation over its cap, never more than are owned.
    let cap = sim::mag_cap(research_level) as usize;
    let owned = sim::magnifiers_owned(&simulation, &ctx) as usize;
    'place: for _ in 0..glasses.min(owned) {
        for slot in &usable {
            let count = simulation
                .mags
                .iter()
                .filter(|mag| mag.slot == Some(*slot))
                .count();
            if count < cap {
                simulation
                    .mags
                    .push(idleon_core::model::Mag::new(MagKind::Regular, Some(*slot)));
                continue 'place;
            }
        }
    }
    (simulation, ctx)
}

fn run(
    simulation: &Simulation,
    ctx: &SimContext,
    effort: Effort,
) -> idleon_core::model::OptimizeResult {
    let request = OptimizeRequest {
        target: Target::Level(simulation.research_level + 5),
        effort,
        respec_grid: effort == Effort::Thorough,
        grid_points: None,
    };
    optimize::run(simulation, ctx, &request, &mut |_| {})
}

#[test]
fn no_effort_makes_the_rate_worse() {
    for level in [1, 60, 90, 150] {
        for glasses in [0, 2, 5] {
            let (simulation, ctx) = seeded(1, level, glasses);
            for effort in [Effort::Quick, Effort::Balanced, Effort::Thorough] {
                let result = run(&simulation, &ctx, effort);
                assert!(
                    result.after_rate >= result.before_rate - 1e-9,
                    "level {level}, {glasses} glasses, {effort:?}: {} < {}",
                    result.after_rate,
                    result.before_rate
                );
            }
        }
    }
}

#[test]
fn shapes_stay_on_the_canvas_and_within_the_unlock_count() {
    let (simulation, ctx) = seeded(1, 120, 2);
    let result = run(&simulation, &ctx, Effort::Balanced);
    assert!(result.after.placements.len() <= sim::shapes_owned(120, &ctx));
    for placement in &result.after.placements {
        assert!(shape::in_bounds(placement), "{placement:?} leaves the canvas");
        assert!(!shape::cells_at(placement).is_empty(), "{placement:?} covers nothing");
    }
}

#[test]
fn magnifiers_respect_the_cap_and_the_owned_count() {
    let (simulation, ctx) = seeded(1, 120, 3);
    let result = run(&simulation, &ctx, Effort::Balanced);
    let owned = sim::magnifiers_owned(&result.after, &ctx);
    assert!(result.after.mags.len() as u32 <= owned);
    let cap = sim::mag_cap(result.after.research_level) as usize;
    for obs in 0..game().observations.len() {
        let count = result
            .after
            .mags
            .iter()
            .filter(|mag| mag.slot == Some(obs))
            .count();
        assert!(count <= cap, "observation {obs} holds {count} > {cap}");
    }
}

#[test]
fn grid_spending_stays_within_the_earned_points() {
    let (simulation, ctx) = seeded(1, 120, 2);
    let result = run(&simulation, &ctx, Effort::Thorough);
    let earned = sim::grid_points(&result.after, &ctx).earned;
    assert!(
        result.after.grid.spent() <= earned,
        "spent {} of {}",
        result.after.grid.spent(),
        earned
    );
    for cell in result.after.grid.live_cells() {
        assert!(game().node_at(cell).is_some(), "cell {cell} has no node");
        assert!(
            optimize::grid::unlocked(cell, &result.after.grid),
            "cell {cell} is leveled but locked"
        );
    }
}

#[test]
fn runs_are_deterministic() {
    let (simulation, ctx) = seeded(1, 90, 2);
    let first = run(&simulation, &ctx, Effort::Balanced);
    let second = run(&simulation, &ctx, Effort::Balanced);
    assert_eq!(first.after.placements, second.after.placements);
    assert_eq!(first.after.mags, second.after.mags);
    assert_eq!(first.after.grid.levels(), second.after.grid.levels());
    assert_eq!(first.after_rate, second.after_rate);
}

#[test]
fn thorough_runs_finish_quickly() {
    let (simulation, ctx) = seeded(1, 150, 6);
    let started = Instant::now();
    let result = run(&simulation, &ctx, Effort::Thorough);
    let elapsed = started.elapsed();
    assert!(result.after_rate >= result.before_rate);
    assert!(
        elapsed.as_secs_f64() < 20.0,
        "thorough took {elapsed:?}, which is too slow for a desktop run"
    );
}
