//! The Worship TD strategy optimizer: the seeded search, its baselines and its
//! cancellation.

use idleon_core::model::Control;
use idleon_core::worship::{self, Inputs, Mode, Options, Outcome, Run, SkipPolicy};

/// Trial levels for a fully unlocked account, so every type can be placed.
fn unlocked() -> Inputs {
    Inputs {
        tower_levels: [50; 9],
        start_pts: 1_000_000_000_000.0,
        totem_hp: 50.0,
        ..Inputs::default()
    }
}

/// The plan the section shows: the guide preset when the trial has one, so the
/// coverage heuristic stays a distinct baseline.
fn current_plan(trial: usize) -> worship::Plan {
    worship::guide_presets_for_trial(trial)
        .next()
        .map(|preset| preset.plan())
        .unwrap_or_else(|| worship::heuristic_plan(&unlocked(), worship::trial(trial).unwrap()))
}

/// A small search: one screening run per plan and a shared final comparison.
fn options(trial: usize, budget: usize, max_wave: u32) -> Options {
    Options {
        trial,
        mode: Mode::Quick,
        seeds: 1,
        base_seed: 424_242,
        max_wave,
        budget: Some(budget),
        seed_plan: Some(current_plan(trial)),
        ..Options::default()
    }
}

fn search(inputs: &Inputs, options: &Options) -> worship::Search {
    match worship::optimize(inputs, options, &mut |_| Control::Continue) {
        Outcome::Found(search) => *search,
        other => panic!("the search returned {other:?}"),
    }
}

#[test]
fn the_same_seed_searches_the_same_plans() {
    // Enough budget to refine a family: the parallel screening batch must not
    // change which plans the beam keeps.
    let options = options(0, 10, 60);
    let first = search(&unlocked(), &options);
    let second = search(&unlocked(), &options);
    assert_eq!(first.plan, second.plan, "the same seed found another plan");
    assert_eq!(first.score, second.score);
    assert_eq!(first.label, second.label);
    assert_eq!(first.evaluations, second.evaluations);
    assert_eq!(first.finalists, second.finalists);
    assert_eq!(first.baselines, second.baselines);
}

#[test]
fn another_seed_compares_on_another_seed_list() {
    let mut options = options(0, 4, 60);
    let first = search(&unlocked(), &options);
    options.base_seed += 1;
    let second = search(&unlocked(), &options);
    assert_ne!(first.seeds, second.seeds);
}

#[test]
fn the_recommended_plan_never_scores_below_a_baseline() {
    // A cap the guide preset clears, so the comparison separates plans instead
    // of tying every candidate at the stop wave.
    let search = search(&unlocked(), &options(0, 4, 140));
    let baseline = search
        .best_baseline()
        .expect("the comparison ran a baseline");
    assert!(
        search.beats_baseline(),
        "the recommended plan clears {} waves, the best baseline {} ({})",
        search.score.primary,
        baseline.score.primary,
        baseline.label
    );
    assert!(
        search
            .baselines
            .iter()
            .any(|baseline| baseline.label == "Coverage heuristic"),
        "the coverage heuristic is the baseline every trial has"
    );
    assert!(
        search
            .finalists
            .iter()
            .any(|finalist| finalist.label == search.label && finalist.score == search.score),
        "the winner is one of the compared finalists"
    );
}

#[test]
fn the_recommended_plan_places_every_tower() {
    let trial = worship::trial(0).expect("trial 0");
    let search = search(&unlocked(), &options(0, 4, 140));
    assert!(!search.plan.towers.is_empty(), "the plan has no towers");
    let stages = worship::plan_stages(trial, &search.plan);
    for stage in &stages {
        assert!(
            stage.rejected.is_empty(),
            "{}: {:?}",
            stage.label,
            stage.rejected
        );
    }
    let placed = stages[0].towers.len();
    assert_eq!(
        placed,
        search.plan.towers.len(),
        "the opening stage drops towers the plan lists"
    );

    // Upgrades compete with placements for the same points, so the opening
    // stage fills over the first few seconds rather than the first tick.
    let mut run = Run::with_plan(unlocked(), 0, 7, search.plan.clone());
    run.step(200);
    let mut standing: Vec<(usize, i64)> = run
        .view()
        .towers
        .iter()
        .map(|tower| (tower.node, (tower.x * 100.0) as i64))
        .collect();
    standing.sort_unstable();
    let mut expected: Vec<(usize, i64)> = stages[0]
        .towers
        .iter()
        .map(|tower| (tower.node, (tower.x * 100.0) as i64))
        .collect();
    expected.sort_unstable();
    assert_eq!(standing, expected, "the run did not build the plan");
}

#[test]
fn a_cancelled_search_stores_nothing() {
    let mut reports = 0;
    let outcome = worship::optimize(&unlocked(), &options(0, 4, 60), &mut |_| {
        reports += 1;
        Control::Stop
    });
    assert_eq!(outcome, Outcome::Cancelled);
    assert_eq!(reports, 1, "the run stopped at its first report");
}

#[test]
fn an_account_without_tower_levels_is_refused() {
    let outcome = worship::optimize(&Inputs::default(), &options(0, 2, 60), &mut |_| {
        Control::Continue
    });
    assert_eq!(
        outcome,
        Outcome::Refused("No unlocked tower types; import a save first")
    );
}

#[test]
fn the_plan_on_screen_joins_the_baselines() {
    let mut with_seed = options(0, 1, 60);
    with_seed.seed_plan = Some(current_plan(0));
    let search = search(&unlocked(), &with_seed);
    assert!(
        search
            .baselines
            .iter()
            .any(|baseline| baseline.label == "Current plan"),
        "the plan on screen is compared: {:?}",
        search.baselines
    );
}

#[test]
fn the_trials_read_the_plan_engine() {
    let inputs = unlocked();
    let trial = worship::trial(0).expect("trial 0");
    let plan = worship::Plan {
        skip_policy: SkipPolicy::Never,
        ..worship::heuristic_plan(&inputs, trial)
    };
    let options = Options {
        trial: 0,
        max_wave: 60,
        ..Options::default()
    };
    let trials = worship::simulate_trials(&inputs, &plan, &options, &[3, 9]);
    assert_eq!(trials.runs.len(), 2);
    for (seed, run) in [3, 9].into_iter().zip(&trials.runs) {
        let mut direct = Run::with_plan(inputs.clone(), 0, seed, plan.clone())
            .with_limits(options.max_wave, options.max_time_sec);
        while !direct.done() {
            direct.step(1_000);
        }
        let direct = direct.result();
        assert_eq!(run.best_wave, direct.best_wave, "seed {seed}");
        assert_eq!(run.highest_wave, direct.highest_wave, "seed {seed}");
        assert_eq!(run.towers, direct.towers, "seed {seed}");
    }
    assert_eq!(
        trials.mean_best_wave,
        (f64::from(trials.min_best_wave) + f64::from(trials.max_best_wave)) / 2.0,
        "two runs average to their midpoint"
    );
}

#[test]
fn a_run_stops_at_its_wave_cap() {
    let inputs = unlocked();
    let plan = worship::heuristic_plan(&inputs, worship::trial(0).expect("trial 0"));
    let mut run = Run::with_plan(inputs, 0, 1, plan).with_limits(40, 4.0 * 3600.0);
    while !run.done() {
        run.step(1_000);
    }
    let result = run.result();
    assert_eq!(result.end_reason, worship::EndReason::MaxWave);
    assert!(result.highest_wave >= 40, "reached {}", result.highest_wave);
    assert!(result.time_sec < 4.0 * 3600.0, "the wave cap ended it");
}
