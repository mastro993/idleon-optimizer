//! The Worship TD simulator: the wave fixture, determinism and the totem rules.

use idleon_core::worship::{self, EndReason, Inputs, Run};

/// Trial levels for a fully unlocked account, so every type can be placed.
fn unlocked() -> Inputs {
    Inputs {
        tower_levels: [50; 9],
        start_pts: 100_000.0,
        totem_hp: 50.0,
        ..Inputs::default()
    }
}

/// The first legal spot on the first trial's first placeable platform.
fn first_spot(run: &Run) -> (usize, f64) {
    let trial = run.trial();
    for node in worship::placeable_nodes(trial) {
        if let Some(&(a, _)) = run.node_spans(node).first() {
            return (node, a);
        }
    }
    panic!("trial 0 has no placeable spot");
}

#[test]
fn the_wave_fixture_matches_the_upstream_rows() {
    // Rows transcribed from `WorshipInfo` for the Glublin trial, plus the tier
    // scaling `tdWaveRow` applies at waves 31 and 61.
    let glublin = worship::trial(0).expect("trial 0");
    let rows: [(u32, f64, f64, f64); 5] = [
        (1, 2.0, 0.0, 2.0),
        (2, 5.0, 0.0, 2.0),
        (30, 1.0, 2.0, 4.0),
        (31, 3.0, 2.0, 2.0),
        (61, 4.0, 4.0, 2.0),
    ];
    for (wave, count, skull, size) in rows {
        let row = worship::wave_row(glublin, wave);
        assert_eq!(
            (row.count, row.skull, row.size),
            (count, skull, size),
            "wave {wave}"
        );
    }
    assert_eq!(worship::wave_row(glublin, 31).tier, 1.0);
    assert_eq!(worship::wave_row(glublin, 61).tier, 2.0);
    assert_eq!(worship::mob_max_hp(glublin, 2.0, 0.0), 3.0);
    assert_eq!(worship::mob_max_hp(glublin, 4.0, 2.0), 350.0);
}

#[test]
fn the_same_seed_and_actions_resolve_the_same_waves() {
    let play = |seed: u64| {
        let mut run = Run::new(unlocked(), 0, seed);
        let (node, x) = first_spot(&run);
        run.place(0, node, x).expect("a pulse tower fits");
        run.step(1_200);
        let view = run.view();
        (
            view.wave,
            view.pts,
            view.totem,
            view.mobs
                .iter()
                .map(|mob| (mob.id, mob.hp_frac, (mob.x * 1000.0) as i64))
                .collect::<Vec<_>>(),
            view.towers
                .iter()
                .map(|tower| tower.level)
                .collect::<Vec<_>>(),
        )
    };
    let first = play(7);
    let second = play(7);
    assert_eq!(first, second, "the same seed diverged");
    assert!(first.0 >= 1, "a wave started");
    assert!(!first.3.is_empty(), "mobs are on the field");
}

#[test]
fn a_restart_builds_a_clean_run() {
    let mut run = Run::new(unlocked(), 0, 3);
    let (node, x) = first_spot(&run);
    run.place(0, node, x).expect("a pulse tower fits");
    run.step(200);
    assert!(run.tick() > 0);
    let restarted = Run::new(unlocked(), 0, 3);
    assert_eq!(restarted.tick(), 0);
    assert_eq!(restarted.view().wave, 0);
    assert!(restarted.view().mobs.is_empty());
    assert!(restarted.view().towers.is_empty());
    assert_eq!(restarted.points(), unlocked().start_pts);
}

#[test]
fn every_type_fires_on_its_own_rules() {
    let mut run = Run::new(unlocked(), 4, 11);
    let trial = run.trial();
    let mut placed = 0;
    for node in trial.path.iter().copied() {
        for (a, b) in run.node_spans(node) {
            let mut x = a;
            while x <= b && placed < 9 {
                if run.place(placed, node, x).is_ok() {
                    placed += 1;
                }
                x += 62.0;
            }
            if placed >= 9 {
                break;
            }
        }
        if placed >= 9 {
            break;
        }
    }
    assert_eq!(placed, 9, "all nine tower types fit on the first trial");
    run.step(3_000);
    let view = run.view();
    assert!(view.towers.len() == 9, "no tower was sold");
    assert!(!view.done, "the run is still going");
    let result = run.result();
    assert!(result.kills > 0 || result.totem_left < 5.0);
    assert!(result.time_sec > 0.0);
    // Damage numbers and at least one projectile effect reached the renderer.
    assert!(view.effects.len() < 500);
}

#[test]
fn leaks_drain_the_totem_and_end_the_run() {
    let mut run = Run::new(Inputs::default(), 0, 1);
    // No towers: the first five leaks destroy the base 5 totem.
    run.step(20_000);
    let result = run.result();
    assert_eq!(result.leaks, 5);
    assert_eq!(result.end_reason, EndReason::Totem);
    assert!(result.totem_left <= 0.0);
}

#[test]
fn a_skip_awards_its_reward_once() {
    let mut run = Run::new(unlocked(), 0, 1);
    run.step(610);
    let before = run.points();
    let reward = run.skip_wave();
    assert!(reward > 0.0);
    assert_eq!(run.points(), before + reward);
    assert_eq!(run.skip_wave(), 0.0, "the countdown already ran out");
}

#[test]
fn the_fixture_supplies_the_live_inputs() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = idleon_core::save::import(&text).expect("the fixture imports");
    let inputs = imported.worship;
    assert_eq!(
        inputs.tower_levels,
        [183, 158, 150, 194, 192, 194, 196, 200, 114]
    );
    assert_eq!(inputs.best_waves, [170, 190, 140, 160, 130, 140, 140, 100]);
    assert_eq!(inputs.dream7, 40.0);
    assert!((inputs.salt_lick_x - 1.1).abs() < 1e-12);
    assert_eq!(inputs.slow_roast_pre_cook_sec, 1200.0);
    assert_eq!(inputs.totem_hp, 20.0);
    assert_eq!(inputs.first_trait_multi, 1.0);
}

#[test]
fn every_trial_has_slots_and_reaches_a_wave() {
    for trial_index in 0..worship::data().trials.len() {
        let trial = worship::trial(trial_index).expect("a declared trial");
        let slots = worship::legal_slots(trial, 62.0);
        assert!(!slots.is_empty(), "trial {trial_index} has no legal slot");
        let mut run = Run::new(unlocked(), trial_index, 5);
        let spot = slots[0];
        run.place(0, spot.node, spot.x)
            .unwrap_or_else(|error| panic!("trial {trial_index}: {}", error.label()));
        run.step(900);
        let view = run.view();
        assert!(view.wave >= 1, "trial {trial_index} never started a wave");
        assert!(view.t >= 80.0, "trial {trial_index} stopped early");
    }
}

#[test]
fn the_glublin_run_matches_the_upstream_digest() {
    // The upstream `createWorshipTdRun` with one Pulse Mage at platform 3, x 561,
    // trial levels [1; 9], 1000 start points, seed 7, stepped 1200 ticks.
    let inputs = Inputs {
        tower_levels: [1; 9],
        start_pts: 1000.0,
        totem_hp: 5.0,
        ..Inputs::default()
    };
    let mut run = Run::new(inputs, 0, 7);
    run.place(0, 3, 561.0).expect("the upstream spot is legal");
    run.step(1_200);
    let view = run.view();
    assert_eq!(view.wave, 4);
    assert_eq!(view.pts, 999.0);
    assert_eq!(view.totem, 0.0);
    assert_eq!(view.mobs.len(), 16);
    let first = &view.mobs[0];
    assert_eq!(first.id, 1);
    assert!((first.hp_frac - 2.0 / 3.0).abs() < 1e-9);
    assert!((first.x - 830.638_6).abs() < 1e-3, "{}", first.x);
    assert_eq!(first.y, 786.0);
    let last = view.mobs.last().expect("mobs are alive");
    assert_eq!(last.id, 19);
    assert!((last.x - 218.938_4).abs() < 1e-3, "{}", last.x);
    let result = run.result();
    assert_eq!(result.kills, 3);
    assert_eq!(result.exp, 3.0);
    assert_eq!(result.leaks, 5);
    assert!((result.time_sec - 57.8).abs() < 1e-9);
    assert_eq!(result.end_reason, EndReason::Totem);
}

/// A plan with plenty of points, so placement is not what limits a test.
fn rich() -> Inputs {
    Inputs {
        start_pts: 1_000_000_000_000.0,
        ..unlocked()
    }
}

#[test]
fn a_plan_run_places_its_stage_layout() {
    let trial = worship::trial(0).expect("trial 0");
    let mut plan = worship::heuristic_plan(&rich(), trial);
    // Cap every tower so the policy's upgrade bids cannot starve the queue.
    for tower in &mut plan.towers {
        tower.max_level = Some(1);
    }
    let stage = worship::plan_stages(trial, &plan)
        .into_iter()
        .next()
        .expect("the initial stage");
    assert!(stage.rejected.is_empty(), "{:?}", stage.rejected);

    let mut run = Run::with_plan(rich(), 0, 7, plan);
    run.step(1);
    let placed: Vec<(usize, i64)> = run
        .view()
        .towers
        .iter()
        .map(|tower| (tower.node, (tower.x * 100.0) as i64))
        .collect();
    let expected: Vec<(usize, i64)> = stage
        .towers
        .iter()
        .map(|tower| (tower.node, (tower.x * 100.0) as i64))
        .collect();
    assert_eq!(placed, expected, "the run did not place the stage layout");
}

#[test]
fn a_plan_phase_sells_and_rebuilds() {
    let (node, x) = first_spot(&Run::new(rich(), 0, 1));
    let plan = worship::Plan {
        towers: vec![worship::PlanTower {
            type_index: 0,
            slot: worship::Spot { node, x },
            weight: 1.0,
            traits: String::new(),
            max_level: Some(1),
        }],
        phases: vec![worship::PlanPhase {
            label: "Boss DPS".into(),
            when: worship::PhaseWhen {
                min_wave: Some(1),
                ..worship::PhaseWhen::default()
            },
            sell: worship::PhaseSell {
                types: vec![0],
                ..worship::PhaseSell::default()
            },
            towers: vec![worship::PlanTower {
                type_index: 1,
                slot: worship::Spot { node, x },
                weight: 1.0,
                traits: String::new(),
                max_level: Some(1),
            }],
            skip_remaining: true,
        }],
        skip_policy: worship::SkipPolicy::Never,
        roots: worship::RootPolicy::Off,
    };
    let mut run = Run::with_plan(rich(), 0, 3, plan);
    run.step(200);
    let result = run.result();
    assert_eq!(result.phases.len(), 1, "the phase did not fire");
    assert_eq!(result.phases[0].sold, 1);
    assert!(result.phases[0].refund > 0.0);
    assert_eq!(result.phases[0].queued, 1);
    let towers = run.view().towers;
    assert_eq!(towers.len(), 1, "the phase did not rebuild");
    assert_eq!(
        towers[0].type_index, 1,
        "the rebuilt tower is not a Fireball"
    );
    assert_eq!(towers[0].plan_index, Some(1));
}

#[test]
fn a_plan_run_only_places_stage_positions() {
    let trial = worship::trial(0).expect("trial 0");
    let plan = worship::guide_preset("gublin-230")
        .expect("the guide preset")
        .plan();
    let stages = worship::plan_stages(trial, &plan);
    let positions: Vec<(usize, usize, i64)> = stages
        .iter()
        .flat_map(|stage| stage.towers.iter())
        .map(|tower| (tower.plan_index, tower.node, (tower.x * 100.0) as i64))
        .collect();

    let mut run = Run::with_plan(rich(), 0, 5, plan);
    run.step(600);
    let view = run.view();
    assert!(!view.towers.is_empty(), "the plan placed nothing");
    for tower in &view.towers {
        let plan_index = tower.plan_index.expect("a plan-built tower");
        let position = positions
            .iter()
            .find(|(index, _, _)| *index == plan_index)
            .unwrap_or_else(|| panic!("plan #{plan_index} has no stage position"));
        assert_eq!(
            (position.1, position.2),
            (tower.node, (tower.x * 100.0) as i64),
            "plan #{plan_index} stands somewhere the stage does not"
        );
    }
}

#[test]
fn a_plan_run_is_deterministic() {
    let plan = worship::guide_preset("gublin-230")
        .expect("the guide preset")
        .plan();
    let play = || {
        let mut run = Run::with_plan(unlocked(), 0, 11, plan.clone());
        run.step(1_200);
        let view = run.view();
        (
            view.wave,
            view.pts,
            view.totem,
            view.towers
                .iter()
                .map(|tower| (tower.type_index, tower.level, (tower.x * 100.0) as i64))
                .collect::<Vec<_>>(),
        )
    };
    assert_eq!(play(), play(), "the same plan and seed diverged");
}

#[test]
fn the_assumptions_are_listed_and_not_empty() {
    assert!(worship::ASSUMPTIONS.len() >= 10);
    assert!(
        worship::ASSUMPTIONS
            .iter()
            .all(|line| !line.trim().is_empty())
    );
}
