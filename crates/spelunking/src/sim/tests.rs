//! Run edges: determinism by seed, party stamina, cancelling and the rejected
//! policies.
//!
//! `crates/spelunking/tests/spelunking.rs` compares whole runs against the upstream
//! calculator's own output; these tests pin the behaviour that has no upstream
//! counterpart (the same seed twice, a cancelled run, a rejected policy).

use super::*;
use crate::{Requested, fixture_inputs, normalize};

fn config(input: &Inputs, requested: Requested) -> Config {
    normalize(input, &requested).expect("the configuration runs")
}

fn run(input: &Inputs, requested: Requested, seed: u32) -> RunResult {
    let config = config(input, requested);
    let mut run = Run::new(input, config, RunOptions { seed }).expect("the run starts");
    while !run.done() {
        run.step(5_000);
    }
    run.result()
}

#[test]
fn the_same_seed_reproduces_the_whole_run() {
    let input = fixture_inputs();
    let requested = Requested {
        cave: Some(1),
        party: Some(vec![0, 1]),
        policy: Some(Policy::Dive),
        elixirs: Some(vec![0, 2, 7]),
        memorized: Some(vec![0, 2]),
        stamina: Some(crate::StaminaMode::Full),
        ..Requested::default()
    };
    let first = run(&input, requested.clone(), 7);
    let second = run(&input, requested.clone(), 7);
    assert_eq!(first, second, "the same inputs and seed diverged");
    let other = run(&input, requested, 8);
    assert_ne!(
        (first.depth, first.rewards, first.seconds),
        (other.depth, other.rewards, other.seconds),
        "a different seed produced the same run"
    );
}

#[test]
fn the_run_starts_where_the_entry_choice_says() {
    let input = fixture_inputs();
    let record = input.depths[0];
    let expected = (50.0 * (crate::js_round(record) / 50.0).floor() - 1.0).max(0.0);
    let entry = |entry| {
        let config = config(
            &input,
            Requested {
                cave: Some(0),
                party: Some(vec![0]),
                manic: Some(false),
                policy: Some(Policy::Dive),
                entry: Some(entry),
                stamina: Some(crate::StaminaMode::Full),
                ..Requested::default()
            },
        );
        Run::new(&input, config, RunOptions { seed: 3 })
            .expect("the run starts")
            .result()
            .start_depth
    };
    assert_eq!(entry(crate::Entry::Shortcut) as f64, expected + 1.0);
    assert_eq!(entry(crate::Entry::Surface), 1);
}

#[test]
fn a_cancelled_run_reports_nothing() {
    let input = fixture_inputs();
    let config = config(
        &input,
        Requested {
            cave: Some(0),
            party: Some(vec![0]),
            policy: Some(Policy::Dive),
            stamina: Some(crate::StaminaMode::Full),
            ..Requested::default()
        },
    );
    let mut reports = 0;
    let outcome = simulate(&input, config, RunOptions { seed: 5 }, &mut |_progress| {
        reports += 1;
        Control::Stop
    })
    .expect("the run starts");
    assert!(outcome.is_none(), "a cancelled run reported a result");
    assert_eq!(reports, 1);
}

#[test]
fn a_run_reports_progress_until_it_settles() {
    let input = fixture_inputs();
    let config = config(
        &input,
        Requested {
            cave: Some(0),
            party: Some(vec![0]),
            policy: Some(Policy::Dive),
            stamina: Some(crate::StaminaMode::Full),
            ..Requested::default()
        },
    );
    let mut fractions = Vec::new();
    let outcome = simulate(&input, config, RunOptions { seed: 5 }, &mut |progress| {
        fractions.push(progress.fraction);
        Control::Continue
    })
    .expect("the run starts")
    .expect("the run finished");
    assert!(fractions.len() > 1, "the run reported once");
    assert!(fractions.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(outcome.status != Status::Running);
}

#[test]
fn the_amber_policy_changes_the_outcome() {
    let input = fixture_inputs();
    let request = |amber: crate::AmberPolicy| Requested {
        cave: Some(1),
        party: Some(vec![0, 1]),
        policy: Some(Policy::AmberFarm),
        elixirs: Some(vec![3, 3, 3, 6, 6]),
        memorized: Some(vec![3, 6]),
        stamina: Some(crate::StaminaMode::Full),
        amber: Some(amber),
        ..Requested::default()
    };
    let patient = run(
        &input,
        request(crate::AmberPolicy {
            dump_depth: 20,
            ..crate::AmberPolicy::default()
        }),
        5,
    );
    let eager = run(
        &input,
        request(crate::AmberPolicy {
            dump_depth: 3,
            finishers: 2,
            reserve_pct: 90,
            harvest_depths: 2,
            rope_reserve_pct: 60,
        }),
        5,
    );
    assert_ne!(
        (patient.depth, patient.gross_rewards[0]),
        (eager.depth, eager.gross_rewards[0]),
        "the two Amber policies produced the same outcome"
    );
    assert_eq!(
        patient.config.amber,
        Some(crate::AmberPolicy {
            dump_depth: 20,
            ..crate::AmberPolicy::default()
        })
    );
    let explained = patient.amber.expect("the Amber policy reports its phases");
    assert!(
        !explained.transitions.is_empty(),
        "the policy never changed phase"
    );
    for transition in &explained.transitions {
        assert!(
            !transition.reason.is_empty(),
            "{} has no reason",
            transition.phase.id()
        );
        assert!(transition.depth >= patient.start_depth);
    }
    assert!(
        explained.transitions.first().map(|first| first.phase) == Some(crate::AmberPhase::Activate),
        "the run should start in buildup"
    );
}

#[test]
fn an_amber_run_is_deterministic_by_seed() {
    let input = fixture_inputs();
    let requested = Requested {
        cave: Some(1),
        party: Some(vec![0, 1]),
        policy: Some(Policy::AmberFarm),
        elixirs: Some(vec![3, 3, 3, 6, 6]),
        memorized: Some(vec![3, 6]),
        stamina: Some(crate::StaminaMode::Full),
        amber: Some(crate::AmberPolicy {
            dump_depth: 3,
            finishers: 2,
            reserve_pct: 90,
            harvest_depths: 2,
            rope_reserve_pct: 60,
        }),
        ..Requested::default()
    };
    let first = run(&input, requested.clone(), 2);
    let second = run(&input, requested, 2);
    assert_eq!(first, second, "the same Amber inputs and seed diverged");
    assert!(first.amber.is_some(), "the Amber phases are reported");
}
