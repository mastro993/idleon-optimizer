//! Invariants every optimizer run must hold.

use idleon_core::model::{
    Effort, GridState, Mag, MagKind, Observations, OptimizeRequest, SimContext, Simulation, Target,
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
        assert!(
            shape::in_bounds(placement),
            "{placement:?} leaves the canvas"
        );
        assert!(
            !shape::cells_at(placement).is_empty(),
            "{placement:?} covers nothing"
        );
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
    let (thorough, _) = kaleidoscope_pool(150);
    for (state, state_ctx) in [(&simulation, &ctx), (&thorough, &ctx)] {
        for effort in [Effort::Quick, Effort::Balanced, Effort::Thorough] {
            let first = run(state, state_ctx, effort);
            let second = run(state, state_ctx, effort);
            assert_eq!(
                first.after.placements, second.after.placements,
                "{effort:?}"
            );
            assert_eq!(first.after.mags, second.after.mags, "{effort:?}");
            assert_eq!(
                first.after.grid.levels(),
                second.after.grid.levels(),
                "{effort:?}"
            );
            assert_eq!(first.after_rate, second.after_rate, "{effort:?}");
            assert_eq!(first.steps, second.steps, "{effort:?}");
        }
    }
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

/// EXP/hour of a whole grid layout, with the magnifier pool the layout's own
/// grants would fill: the same refit the `Optimize` run makes after spending.
fn layout_rate(simulation: &Simulation, ctx: &SimContext, grid: &GridState) -> f64 {
    let mut trial = simulation.clone();
    trial.grid = grid.clone();
    let owned = sim::magnifiers_owned(&trial, ctx) as usize;
    let cap = sim::mag_cap(trial.research_level) as usize;
    let mut slots: Vec<usize> = (0..game().observations.len())
        .filter(|obs| sim::observation_usable(*obs, trial.research_level, &trial.observations))
        .collect();
    slots.sort_by(|a, b| sim::obs_base_exp(*b).total_cmp(&sim::obs_base_exp(*a)));
    while trial.mags.len() < owned {
        let mut placed = false;
        for slot in &slots {
            let held = trial
                .mags
                .iter()
                .filter(|mag| mag.slot == Some(*slot))
                .count();
            if held < cap {
                trial.mags.push(Mag::new(MagKind::Regular, Some(*slot)));
                placed = true;
                break;
            }
        }
        if !placed {
            break;
        }
    }
    sim::exp_rate(&trial, ctx).total
}

#[test]
fn beam_spending_never_loses_to_the_marginal_climb() {
    for level in [60u32, 100, 150] {
        let (simulation, ctx) = seeded(1, level, 4);
        let climb = optimize::grid::respec(&simulation, &ctx, Effort::Quick, None, &mut |_| {});
        let beamed = optimize::grid::respec(&simulation, &ctx, Effort::Thorough, None, &mut |_| {});
        let climbed = layout_rate(&simulation, &ctx, &climb.grid);
        let beam_rate = layout_rate(&simulation, &ctx, &beamed.grid);
        assert!(
            beam_rate >= climbed - 1e-9,
            "level {level}: thorough {beam_rate} < greedy {climbed}"
        );
        assert_eq!(climb.spent, climb.grid.spent());
        assert_eq!(beamed.spent, beamed.grid.spent());
        for outcome in [&climb, &beamed] {
            for cell in outcome.grid.live_cells() {
                let node = game().node_at(cell).expect("a live cell has a node");
                assert!(
                    outcome.grid.level(cell) <= node.max_lv,
                    "cell {cell} over its cap"
                );
                assert!(
                    optimize::grid::unlocked(cell, &outcome.grid),
                    "cell {cell} locked"
                );
            }
        }
    }
}

#[test]
fn beam_spending_honours_an_explicit_budget() {
    let (simulation, ctx) = seeded(1, 150, 4);
    for budget in [0_u32, 12, 40] {
        let outcome = optimize::grid::respec(
            &simulation,
            &ctx,
            Effort::Thorough,
            Some(budget),
            &mut |_| {},
        );
        let spent = outcome.grid.spent().saturating_sub(simulation.grid.spent());
        assert!(spent <= budget, "budget {budget} spent {spent}");
        assert_eq!(outcome.spent, outcome.grid.spent());
    }
}

/// A pool with kaleidoscopes on the bottom row of the observation grid, which
/// is where the richest observations sit.
fn kaleidoscope_pool(level: u32) -> (Simulation, SimContext) {
    let ctx = SimContext::default();
    let mut simulation = Simulation {
        research_level: level,
        grid: GridState::seeded(2),
        observations: Observations::all_at(1),
        ..Simulation::default()
    };
    simulation.grid.set(52, 3);
    simulation.grid.set(72, 4);
    simulation.grid.set(91, 4);
    simulation.mags = vec![
        Mag::new(MagKind::Kaleidoscope, Some(41)),
        Mag::new(MagKind::Kaleidoscope, Some(40)),
        Mag::new(MagKind::Regular, Some(42)),
        Mag::new(MagKind::Regular, Some(3)),
        Mag::new(MagKind::Monocle, Some(1)),
    ];
    (simulation, ctx)
}

#[test]
fn magnifier_repair_never_lowers_the_rate_and_stays_legal() {
    for level in [60u32, 120, 150] {
        let (simulation, ctx) = kaleidoscope_pool(level);
        let quick = optimize::mags::optimize(&simulation, &ctx, Effort::Quick);
        let thorough = optimize::mags::optimize(&simulation, &ctx, Effort::Thorough);
        assert!(
            thorough.rate >= quick.rate - 1e-9,
            "level {level}: repair lowered {} to {}",
            quick.rate,
            thorough.rate
        );
        let owned = sim::magnifiers_owned(&simulation, &ctx) as usize;
        let cap = sim::mag_cap(level) as usize;
        assert!(
            thorough.mags.len() <= owned,
            "level {level} placed beyond the pool"
        );
        for mag in &thorough.mags {
            let Some(slot) = mag.slot else {
                panic!("a placed magnifier has a slot");
            };
            assert!(
                sim::observation_usable(slot, level, &simulation.observations),
                "slot {slot} cannot hold a magnifier at level {level}"
            );
            let held = thorough
                .mags
                .iter()
                .filter(|other| other.slot == Some(slot))
                .count();
            assert!(held <= cap, "slot {slot} holds {held} of {cap}");
        }
        let again = optimize::mags::optimize(&simulation, &ctx, Effort::Thorough);
        assert_eq!(
            thorough.mags, again.mags,
            "level {level} is not deterministic"
        );
        assert_eq!(thorough.rate, again.rate);
    }
}

/// Visit every way to put one magnifier of each of `kinds` on the usable slots,
/// keeping each slot within `cap`: the multiset enumeration, not permutations,
/// because two glasses on the same observation are one assignment.
fn each_assignment(
    usable: &[usize],
    cap: usize,
    kinds: &[MagKind],
    current: &mut Vec<Mag>,
    visit: &mut dyn FnMut(&[Mag]),
) {
    if current.len() == kinds.len() {
        visit(current);
        return;
    }
    let start = current
        .last()
        .and_then(|mag| mag.slot)
        .and_then(|slot| usable.iter().position(|usable| *usable == slot))
        .unwrap_or(0);
    for index in start..usable.len() {
        let held = current
            .iter()
            .filter(|mag| mag.slot == Some(usable[index]))
            .count();
        if held >= cap {
            continue;
        }
        current.push(Mag::new(kinds[current.len()], Some(usable[index])));
        each_assignment(usable, cap, kinds, current, visit);
        current.pop();
    }
}

#[test]
fn magnifier_assignment_matches_brute_force_on_a_small_board() {
    let ctx = SimContext::default();
    let level = 60;
    let mut simulation = Simulation {
        research_level: level,
        grid: GridState::seeded(2),
        observations: Observations::all_at(1),
        ..Simulation::default()
    };
    simulation.grid.set(52, 2);
    simulation.grid.set(72, 3);
    simulation.grid.set(91, 3);
    simulation.mags = vec![
        Mag::new(MagKind::Kaleidoscope, None),
        Mag::new(MagKind::Regular, None),
        Mag::new(MagKind::Monocle, None),
    ];
    let usable: Vec<usize> = (0..game().observations.len())
        .filter(|obs| sim::observation_usable(*obs, level, &simulation.observations))
        .collect();
    let cap = sim::mag_cap(level) as usize;
    let outcome = optimize::mags::optimize(&simulation, &ctx, Effort::Thorough);

    // Every placement of the pool the optimizer chose, over the slots this level
    // unlocks: the search must not miss one that pays more.
    let kinds: Vec<MagKind> = outcome.mags.iter().map(|mag| mag.kind).collect();
    let mut best: f64 = 0.0;
    let mut trial = simulation.clone();
    each_assignment(&usable, cap, &kinds, &mut Vec::new(), &mut |mags| {
        trial.mags = mags.to_vec();
        best = best.max(sim::exp_rate(&trial, &ctx).total);
    });
    assert!(best > 0.0, "the brute force found no legal assignment");
    assert!(
        best <= outcome.rate * (1.0 + 1e-12),
        "a better assignment exists: brute force {best} > optimizer {}",
        outcome.rate
    );
}

#[test]
fn shape_search_only_rotates_where_the_level_unlocks_it() {
    for level in [40u32, 95] {
        let (simulation, ctx) = seeded(1, level, 2);
        let outcome = optimize::shapes::optimize(&simulation, &ctx, Effort::Thorough, &mut |_| {});
        assert!(
            outcome.rate >= sim::exp_rate(&simulation, &ctx).total - 1e-9,
            "level {level}: the search lowered the rate to {}",
            outcome.rate
        );
        assert_eq!(outcome.overlay.len(), game().cells());
        for placement in &outcome.placements {
            assert!(
                shape::in_bounds(placement),
                "{placement:?} leaves the canvas"
            );
            assert!(
                !shape::cells_at(placement).is_empty(),
                "{placement:?} covers nothing"
            );
            if !sim::can_rotate(level) {
                assert_eq!(placement.rot, 0.0, "level {level} cannot rotate");
            }
        }
    }
}

#[test]
fn beam_spending_unlocks_only_squares_it_may() {
    let ctx = SimContext::default();
    let mut simulation = Simulation {
        research_level: 150,
        observations: Observations::all_at(1),
        ..Simulation::default()
    };
    simulation.mags = (0..3)
        .map(|slot| Mag::new(MagKind::Regular, Some(slot)))
        .collect();
    let outcome = optimize::grid::respec(&simulation, &ctx, Effort::Thorough, None, &mut |_| {});
    let earned = sim::grid_points(&simulation, &ctx).earned;
    assert!(
        outcome.grid.spent() <= earned,
        "spent {} of {earned}",
        outcome.grid.spent()
    );
    for cell in outcome.grid.live_cells() {
        let node = game().node_at(cell).expect("a live cell has a node");
        assert!(
            outcome.grid.level(cell) <= node.max_lv,
            "cell {cell} over its cap"
        );
        assert!(
            optimize::grid::unlocked(cell, &outcome.grid),
            "cell {cell} locked"
        );
    }
    assert!(
        outcome.grid.spent() > 0,
        "nothing was spent on an empty grid"
    );
}

#[test]
fn progress_and_steps_report_what_the_run_did() {
    let (simulation, ctx) = seeded(1, 150, 6);
    for effort in [Effort::Quick, Effort::Balanced, Effort::Thorough] {
        let mut fractions = Vec::new();
        let mut messages = Vec::new();
        let request = OptimizeRequest {
            target: Target::Level(155),
            effort,
            respec_grid: effort == Effort::Thorough,
            grid_points: None,
        };
        let result = optimize::run(&simulation, &ctx, &request, &mut |progress| {
            fractions.push(progress.fraction);
            messages.push(progress.message);
        });
        assert!(!fractions.is_empty(), "{effort:?} reported no progress");
        assert!(
            fractions.windows(2).all(|pair| pair[1] >= pair[0]),
            "{effort:?} progress went backwards: {fractions:?}"
        );
        assert_eq!(
            fractions.last().copied(),
            Some(1.0),
            "{effort:?} never finished"
        );
        assert!(messages.iter().all(|message| !message.is_empty()));
        assert!(!result.steps.is_empty(), "{effort:?} reported no steps");
        let last = result.steps.last().expect("a last step");
        assert!(
            last.contains(&sim::format_rate(result.after_rate)),
            "{effort:?} last step does not name the final rate: {last}"
        );
        for step in &result.steps {
            assert!(
                step.ends_with('.'),
                "{effort:?} step is not a sentence: {step}"
            );
        }
    }
}
