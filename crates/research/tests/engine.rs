//! The target-aware run: the points each level earns are spent on the way,
//! observations unlock at their research requirement, and the same inputs
//! reproduce the run.

use idleon_common::Control;
use idleon_research::engine::{self, Refit, TargetRun};
use idleon_research::model::{
    Effort, GridState, Mag, MagKind, Observations, SimContext, SimOptions, Simulation, Target,
};
use idleon_research::sim::Event;
use idleon_research::{game, optimize, sim};

/// A legal starting state: one glass on the first observation the level unlocks.
fn start(level: u32, grid: GridState) -> (Simulation, SimContext) {
    let ctx = SimContext::default();
    let mut simulation = Simulation {
        research_level: level,
        grid,
        observations: Observations::all_at(1),
        ..Simulation::default()
    };
    let obs = (0..game().observations.len())
        .find(|obs| sim::observation_usable(*obs, level, &simulation.observations))
        .expect("a usable observation");
    simulation.mags = vec![Mag::new(MagKind::Regular, Some(obs))];
    (simulation, ctx)
}

/// A state whose grid reads insight for EXP, with the monocle on an observation
/// that holds no glass: the refit's cue to move it.
fn insight_state() -> (Simulation, SimContext) {
    let ctx = SimContext::default();
    let mut simulation = Simulation {
        research_level: 120,
        grid: GridState::seeded(2),
        observations: Observations::all_at(0),
        ..Simulation::default()
    };
    simulation.grid.set(72, 1);
    simulation.grid.set(91, 0);
    simulation.grid.set(93, 1);
    simulation.grid.set(94, 0);
    simulation.mags = vec![
        Mag::new(MagKind::Kaleidoscope, Some(41)),
        Mag::new(MagKind::Kaleidoscope, Some(40)),
        Mag::new(MagKind::Regular, Some(42)),
        Mag::new(MagKind::Regular, Some(3)),
        Mag::new(MagKind::Monocle, Some(1)),
    ];
    (simulation, ctx)
}

fn simulate(
    simulation: &Simulation,
    ctx: &SimContext,
    target: Target,
    options: SimOptions,
    refit: Refit,
) -> TargetRun {
    engine::simulate(simulation, ctx, target, options, refit, &mut |_| {
        Control::Continue
    })
    .expect("a run with a no-op reporter is never cancelled")
}

#[test]
fn a_run_spends_the_grid_points_the_levels_earn() {
    let (simulation, ctx) = start(10, GridState::empty());
    let fixed = simulate(
        &simulation,
        &ctx,
        Target::Level(14),
        SimOptions::default(),
        Refit::Fixed,
    );
    let live = simulate(
        &simulation,
        &ctx,
        Target::Level(14),
        SimOptions::default(),
        Refit::Level,
    );

    assert_eq!(fixed.projection.final_level, 14);
    assert_eq!(fixed.state.grid.spent(), 0, "a fixed run must not spend");
    assert_eq!(live.projection.final_level, 14);
    assert!(live.grid_points > 0, "the run spent no points");
    let points = sim::grid_points(&live.state, &ctx);
    assert_eq!(live.state.grid.spent(), points.earned);
    assert_eq!(
        points.free, 0,
        "the run left {} points unspent",
        points.free
    );
    assert!(
        live.projection.hours < fixed.projection.hours,
        "spending the points ({:.1}h) is not faster than keeping them ({:.1}h)",
        live.projection.hours,
        fixed.projection.hours
    );
}

#[test]
fn observations_unlock_at_their_research_requirement() {
    let (mut simulation, ctx) = start(10, GridState::empty());
    simulation.observations = Observations::default();
    simulation.observations.found[0] = true;
    simulation.mags = vec![Mag::new(MagKind::Regular, Some(0))];
    let options = SimOptions {
        assume_observations: true,
        ..SimOptions::default()
    };
    let run = simulate(&simulation, &ctx, Target::Level(40), options, Refit::Level);
    let level = f64::from(run.projection.final_level);
    assert!(level >= 40.0);

    for (obs, found) in run.state.observations.found.iter().enumerate() {
        let expected = obs == 0 || game().observations[obs].roll_req <= level;
        assert_eq!(*found, expected, "observation {obs} at level {level}");
    }
}

#[test]
fn an_insight_level_up_re_fits_the_magnifiers() {
    let (simulation, ctx) = insight_state();
    let fixed = simulate(
        &simulation,
        &ctx,
        Target::Hours(3.0),
        SimOptions::default(),
        Refit::Fixed,
    );
    let live = simulate(
        &simulation,
        &ctx,
        Target::Hours(3.0),
        SimOptions::default(),
        Refit::Level,
    );
    let monocle = |run: &TargetRun| {
        run.state
            .mags
            .iter()
            .find(|mag| mag.kind == MagKind::Monocle)
            .and_then(|mag| mag.slot)
    };
    assert_eq!(
        monocle(&fixed),
        Some(1),
        "a fixed run keeps the assignment it started with"
    );
    assert_eq!(
        monocle(&live),
        Some(42),
        "the refit moves the monocle to the observation whose insight pays"
    );
    assert!(
        sim::exp_rate(&live.state, &ctx).total > sim::exp_rate(&fixed.state, &ctx).total,
        "the refit did not raise the rate"
    );
}

#[test]
fn the_same_inputs_reproduce_the_run() {
    for (simulation, ctx) in [start(10, GridState::empty()), insight_state()] {
        for target in [Target::Hours(3.0), Target::Level(122)] {
            let first = simulate(
                &simulation,
                &ctx,
                target,
                SimOptions::default(),
                Refit::Level,
            );
            let second = simulate(
                &simulation,
                &ctx,
                target,
                SimOptions::default(),
                Refit::Level,
            );
            assert_eq!(first.projection, second.projection, "{target:?}");
            assert_eq!(first.state, second.state, "{target:?}");
            assert_eq!(first.grid_points, second.grid_points, "{target:?}");
        }
    }
}

#[test]
fn grid_spending_keeps_the_layout_that_reaches_the_level_first() {
    let (simulation, ctx) = start(120, GridState::seeded(1));
    let target = Target::Level(124);
    let options = SimOptions::default();
    let project = |outcome: &optimize::grid::GridOutcome| {
        let mut trial = simulation.clone();
        trial.grid = outcome.grid.clone();
        simulate(&trial, &ctx, target, options, Refit::Fixed).projection
    };

    let climb = optimize::grid::respec(&simulation, &ctx, Effort::Quick, None, &mut |_| {
        Control::Continue
    });
    let searched = optimize::grid::respec(&simulation, &ctx, Effort::Balanced, None, &mut |_| {
        Control::Continue
    });
    let chosen = optimize::grid::spend_for_target(
        &simulation,
        &ctx,
        Effort::Balanced,
        None,
        target,
        options,
        &mut |_| Control::Continue,
    )
    .expect("the spend is not cancelled");
    let chosen = project(&chosen);

    assert!(
        !engine::better(&project(&climb), &chosen, target),
        "the marginal climb reaches the level sooner than the chosen layout"
    );
    assert!(
        !engine::better(&project(&searched), &chosen, target),
        "the full search reaches the level sooner than the chosen layout"
    );
}

/// A state with glasses on the richest observation, for the plain projections.
fn filled() -> (Simulation, SimContext) {
    let mut simulation = Simulation {
        research_level: 60,
        grid: GridState::seeded(2),
        observations: Observations::all_at(0),
        ..Simulation::default()
    };
    simulation.grid.set(50, 1);
    simulation.mags = vec![Mag::new(MagKind::Regular, Some(3))];
    (simulation, SimContext::default())
}

#[test]
fn a_projection_reaches_a_level_and_stops() {
    let (simulation, ctx) = filled();
    let run = simulate(
        &simulation,
        &ctx,
        Target::Level(62),
        SimOptions::default(),
        Refit::Fixed,
    );
    assert_eq!(run.projection.final_level, 62);
    assert!(run.projection.hours > 0.0);
    assert!(run.projection.samples.len() >= 2);
}

#[test]
fn a_projection_to_a_far_level_terminates_with_an_honest_hour_count() {
    let (mut simulation, ctx) = filled();
    simulation.research_level = 150;
    let run = simulate(
        &simulation,
        &ctx,
        Target::Level(151),
        SimOptions::default(),
        Refit::Fixed,
    );
    assert_eq!(run.projection.final_level, 151);
    assert!(run.projection.hours > 1e6, "hours {}", run.projection.hours);
    assert!(
        run.projection.samples.len() < 64,
        "samples {}",
        run.projection.samples.len()
    );
}

#[test]
fn an_hour_projection_collects_insight() {
    let (mut simulation, ctx) = filled();
    simulation.mags.push(Mag::new(MagKind::Monocle, Some(3)));
    let run = simulate(
        &simulation,
        &ctx,
        Target::Hours(5.0),
        SimOptions::default(),
        Refit::Fixed,
    );
    assert!((run.projection.hours - 5.0).abs() < 1e-6);
    assert!(run.projection.insight_gained > 0);
}

#[test]
fn assuming_observations_unlocked_raises_the_projected_rate() {
    let (mut simulation, ctx) = filled();
    simulation.observations.found.fill(false);
    simulation.observations.insight_level.fill(0);
    simulation.grid = GridState::empty();
    // See 'Em All: +5% EXP per observation found.
    simulation.grid.set(112, 1);
    simulation.mags = vec![Mag::new(MagKind::Regular, Some(0))];
    let options = |assume_observations| SimOptions {
        assume_observations,
        ..SimOptions::default()
    };

    let plain = simulate(
        &simulation,
        &ctx,
        Target::Level(65),
        options(false),
        Refit::Fixed,
    );
    let assumed = simulate(
        &simulation,
        &ctx,
        Target::Level(65),
        options(true),
        Refit::Fixed,
    );
    assert_eq!(assumed.projection.final_level, 65);
    assert_eq!(plain.projection.final_level, 65);
    let plain_rate = plain.projection.samples.last().expect("a sample").rate;
    let assumed_rate = assumed.projection.samples.last().expect("a sample").rate;
    assert!(
        assumed_rate > plain_rate * 1.1,
        "{assumed_rate} is not above {plain_rate}"
    );
    assert!(
        assumed.projection.hours < plain.projection.hours,
        "{}h is not faster than {}h",
        assumed.projection.hours,
        plain.projection.hours
    );
}

#[test]
fn the_tournament_option_adds_twelve_hours_a_day() {
    let (simulation, ctx) = filled();
    let options = |include_tournament| SimOptions {
        include_tournament,
        ..SimOptions::default()
    };
    let plain = simulate(
        &simulation,
        &ctx,
        Target::Hours(50.0),
        options(false),
        Refit::Fixed,
    );
    let tournament = simulate(
        &simulation,
        &ctx,
        Target::Hours(50.0),
        options(true),
        Refit::Fixed,
    );
    // Two registrations in 50 hours: 24 hours of free research time.
    assert!(
        tournament.projection.total_exp > plain.projection.total_exp * 1.3,
        "{} is not well above {}",
        tournament.projection.total_exp,
        plain.projection.total_exp
    );
    assert!(
        (tournament.projection.hours - 50.0).abs() < 1e-9,
        "{}",
        tournament.projection.hours
    );
}

#[test]
fn every_sample_marks_the_event_that_opened_it() {
    let (simulation, ctx) = filled();
    let tournament = simulate(
        &simulation,
        &ctx,
        Target::Hours(50.0),
        SimOptions {
            include_tournament: true,
            ..SimOptions::default()
        },
        Refit::Fixed,
    );
    let (insight_simulation, insight_ctx) = insight_state();
    let insight = simulate(
        &insight_simulation,
        &insight_ctx,
        Target::Hours(3.0),
        SimOptions::default(),
        Refit::Fixed,
    );

    let (level_simulation, level_ctx) = start(10, GridState::empty());
    let level = simulate(
        &level_simulation,
        &level_ctx,
        Target::Level(14),
        SimOptions::default(),
        Refit::Level,
    );

    let mut seen = Vec::new();
    for (run, last) in [
        (&tournament, Event::End),
        (&insight, Event::End),
        (&level, Event::Level),
    ] {
        let samples = &run.projection.samples;
        assert_eq!(samples[0].event, Event::Start);
        assert_eq!(samples.last().expect("a sample").event, last);
        for pair in samples.windows(2) {
            let (before, after) = (pair[0], pair[1]);
            seen.push(after.event);
            assert!(after.insight >= before.insight, "insight went backwards");
            match after.event {
                Event::Level => assert!(after.level > before.level),
                Event::Insight => assert!(after.insight > before.insight),
                Event::LevelAndInsight => {
                    assert!(after.level > before.level);
                    assert!(after.insight > before.insight);
                }
                Event::Tournament => assert!(after.bonus > 0.0, "a tournament added nothing"),
                Event::Start | Event::End => {}
            }
        }
        assert_eq!(
            samples.last().expect("a sample").insight,
            run.projection.insight_gained
        );
    }
    for event in [Event::Level, Event::Insight, Event::Tournament] {
        assert!(
            seen.contains(&event),
            "{event:?} never marked a sample: {seen:?}"
        );
    }
}
