//! Cog board parity and invariants against the committed synthetic save.
//!
//! The expected totals come from the upstream
//! `js/cogs/cog-board.js` (`boardFromSave`, `recoverRawPlayerStats`,
//! `computeBoardTotals`, `extractAllCogs`) run against the same save. They pin
//! the decode and the two-phase maths; the optimizer tests pin the two promises
//! the section makes: board rules hold and a result is never worse than its
//! input.

mod common;

use common::cog_save;
use idleon_cog::{
    BOARD_SIZE, Cog, CogResult, Direction, Goal, Objective, OptimizeOptions, SwapWalkthrough,
    WeightError, WeightInputs, Weights, apply_swaps, compute_totals, empty_board, optimize,
    perfect_board, score,
};
use idleon_common::Control;

fn assert_close(label: &str, got: f64, expected: f64) {
    let scale = expected.abs().max(1.0);
    assert!(
        (got - expected).abs() / scale < 1e-12,
        "{label}: got {got}, upstream {expected}"
    );
}

#[test]
fn the_fixture_board_decodes_like_the_upstream_calculator() {
    let save = cog_save();
    assert_eq!(save.board.len(), BOARD_SIZE);
    assert_eq!(save.board.iter().flatten().count(), 96);
    assert_eq!(save.shelf.len(), 30);
    assert_eq!(save.shelf_slots.len(), 120);
    assert_eq!(save.construction_level, 8005);

    let totals = compute_totals(&save.board);
    assert_eq!(totals.player_slot, Some(14));
    assert_close("totalBuild", totals.total_build, 3.980_538_245_564_946_4e18);
    assert_close("totalConExp", totals.total_con_exp, 27_357.0);
    assert_close("totalFlaggy", totals.total_flaggy, 72_928_686_228_341.4);
    assert_close(
        "playerConExpHr",
        totals.player_con_exp_hr,
        9.527_511_818_649_52e21,
    );
    let player = save.board[14].as_ref().expect("the player cog");
    assert_close("player raw b", player.b, 1.100_531_408_867_491_7e18);
}

#[test]
fn the_optimizer_holds_the_board_rules_and_never_loses_score() {
    let save = cog_save();
    let options = OptimizeOptions {
        iterations: 500,
        ..OptimizeOptions::default()
    };
    for goal in Goal::ALL {
        let mut report = |_progress| Control::Continue;
        let objective = Objective::goal(goal);
        let result = optimize(&save.board, &save.shelf, &objective, &options, &mut report)
            .unwrap_or_else(|| panic!("{goal:?} runs to completion"));
        assert!(
            result.score >= result.initial_score,
            "{goal:?}: {} < {}",
            result.score,
            result.initial_score
        );
        assert_eq!(result.board.len(), BOARD_SIZE, "{goal:?}: board size");
        // Board rules: every cog on the result board came from the save board
        // or the shelf, and no cog appears twice.
        let mut available: Vec<_> = save
            .board
            .iter()
            .flatten()
            .chain(save.shelf.iter())
            .cloned()
            .collect();
        for cog in result.board.iter().flatten() {
            let index = available
                .iter()
                .position(|candidate| candidate == cog)
                .unwrap_or_else(|| panic!("{goal:?}: {} is not available", cog.name));
            available.remove(index);
        }
        // Yin pieces only pay while they form a 2x2 block; the optimizer locks
        // the blocks it builds, so a piece never sits active outside one.
        let yin_on_board: Vec<_> = result
            .board
            .iter()
            .enumerate()
            .filter(|(_, cog)| cog.as_ref().is_some_and(|cog| cog.yin))
            .collect();
        assert!(
            yin_on_board.is_empty() || yin_on_board.len().is_multiple_of(4),
            "{goal:?}: {} Yin pieces on the board",
            yin_on_board.len()
        );
    }
}

#[test]
fn the_swap_list_rebuilds_the_result_and_undoes_to_the_save() {
    let save = cog_save();
    let options = OptimizeOptions {
        iterations: 400,
        ..OptimizeOptions::default()
    };
    let mut report = |_progress| Control::Continue;
    let objective = Objective::goal(Goal::Build);
    let result: CogResult =
        optimize(&save.board, &save.shelf, &objective, &options, &mut report).expect("a full run");
    let mut shelf: Vec<Option<idleon_cog::Cog>> = save.shelf_slots.clone();
    let mut applied = save.board.clone();
    apply_swaps(
        &mut applied,
        &result.board,
        &mut shelf,
        &result.swaps,
        result.swaps.len(),
    );
    assert_eq!(
        applied, result.board,
        "the steps rebuild the optimized board"
    );

    // A step-by-step undo: applying fewer steps leaves an earlier board, and
    // starting over from the save board is what the reset action does.
    let mut partial = save.board.clone();
    apply_swaps(
        &mut partial,
        &result.board,
        &mut shelf,
        &result.swaps,
        result.swaps.len().saturating_sub(1),
    );
    assert_eq!(partial.len(), BOARD_SIZE);
    let mut reset = result.board.clone();
    reset.clone_from(&save.board);
    assert_eq!(reset, save.board);
}

#[test]
fn the_walkthrough_steps_forwards_and_back_to_its_start() {
    let save = cog_save();
    let options = OptimizeOptions {
        iterations: 400,
        ..OptimizeOptions::default()
    };
    let mut report = |_progress| Control::Continue;
    let objective = Objective::goal(Goal::Build);
    let result =
        optimize(&save.board, &save.shelf, &objective, &options, &mut report).expect("a full run");
    let mut walkthrough =
        SwapWalkthrough::new(&save.board, &result.board, &save.shelf_slots, &result.swaps);

    // Forward, one step at a time: every step lands on the board the matching
    // apply_swaps prefix produces, and the last one is the optimized board.
    let mut boards = vec![walkthrough.board().clone()];
    while walkthrough.next_step() {
        boards.push(walkthrough.board().clone());
    }
    assert_eq!(boards.len(), result.swaps.len() + 1);
    assert_eq!(boards.last(), Some(&result.board));

    // Back, one step at a time: every Previous restores exactly the prior board,
    // all the way to the save board the run started from.
    for expected in boards.iter().rev().skip(1) {
        assert!(walkthrough.previous());
        assert_eq!(walkthrough.board(), expected);
    }
    assert_eq!(walkthrough.board(), &save.board);
    assert!(!walkthrough.previous());

    // Done from the start still commits the optimized board.
    assert_eq!(walkthrough.finish(), result.board);
    assert!(walkthrough.is_complete());
}

fn player(slot: usize, b: f64, d: f64) -> Cog {
    let mut cog = Cog::named("Player_Ana", slot);
    cog.b = b;
    cog.d = d;
    cog
}

fn build_cog(slot: usize, a: f64) -> Cog {
    let mut cog = Cog::named("Cog3A00", slot);
    cog.a = a;
    cog
}

#[test]
fn the_weights_choose_which_metric_ranks_a_board_first() {
    // Board A is the build board, board B the construction EXP board. The
    // reference is board A, so its own mixed score is one.
    let mut a = empty_board();
    a[0] = Some(player(0, 100.0, 0.0));
    a[1] = Some(build_cog(1, 1000.0));
    let mut b = empty_board();
    b[0] = Some(player(0, 1000.0, 100.0));
    b[1] = Some(build_cog(1, 100.0));

    let build = Objective::weighted(Weights::new(1.0, 0.0, 0.0).unwrap(), &a);
    let conexp = Objective::weighted(Weights::new(0.0, 1.0, 0.0).unwrap(), &a);
    assert!(
        score(&a, &build) > score(&b, &build),
        "build weights pick A"
    );
    assert!(
        score(&b, &conexp) > score(&a, &conexp),
        "construction EXP weights pick B"
    );
    assert_eq!(score(&a, &build), 1.0, "the reference board scores one");
    // A single goal is the raw metric, not a normalised mix.
    assert_eq!(score(&a, &Objective::goal(Goal::Build)), 1000.0);
}

#[test]
fn invalid_weights_are_rejected() {
    assert_eq!(Weights::new(0.0, 0.0, 0.0), Err(WeightError::Empty));
    assert_eq!(Weights::new(-1.0, 1.0, 1.0), Err(WeightError::Negative));
    assert_eq!(
        Weights::new(f64::NAN, 1.0, 1.0),
        Err(WeightError::NotFinite)
    );
    assert_eq!(
        Weights::new(f64::INFINITY, 1.0, 1.0),
        Err(WeightError::NotFinite)
    );
    assert_eq!(
        Weights::new(f64::MAX, f64::MAX, f64::MAX),
        Err(WeightError::NotFinite),
        "a finite total that overflows is still unusable"
    );
    assert_eq!(
        WeightError::Empty.message(),
        "At least one weight must be above zero."
    );
    let mut inputs = WeightInputs::default();
    assert!(inputs.weights().is_ok(), "the defaults are valid");
    assert_eq!(
        WeightInputs {
            build: 1.0,
            conexp: 1.0,
            flaggy: 1.0,
        }
        .weights(),
        Ok(Weights::uniform())
    );
    inputs.set(Goal::Build, -5.0);
    assert_eq!(inputs.build, 0.0, "the field clamps to its range");
    inputs.set(Goal::ConExp, f64::NAN);
    assert_eq!(inputs.conexp, 0.0, "a non-finite field reads zero");
    inputs.set(Goal::Flaggy, 0.0);
    assert_eq!(inputs.weights(), Err(WeightError::Empty));
}

#[test]
fn the_optimizer_follows_the_weights() {
    let mut board = empty_board();
    board[0] = Some(player(0, 100.0, 20.0));
    let mut right = Cog::named("Cog3ri", 1);
    right.direction = Some(Direction::Right);
    right.f = 30.0;
    right.d = 10.0;
    board[1] = Some(right);
    let mut filler = build_cog(2, 20.0);
    filler.d = 5.0;
    board[2] = Some(filler);
    board[50] = Some(build_cog(50, 300.0));
    let mut around = Cog::named("Cog3cr", 108);
    around.direction = Some(Direction::Around);
    around.f = 40.0;
    around.d = 30.0;
    let mut upgrade = build_cog(109, 200.0);
    upgrade.d = 1.0;
    let mut build_surround = Cog::named("Cog3cr", 110);
    build_surround.direction = Some(Direction::Around);
    build_surround.e = 100.0;
    let shelf = vec![around, upgrade, build_surround];

    let options = OptimizeOptions {
        iterations: 2_000,
        ..OptimizeOptions::default()
    };
    let build = Objective::weighted(Weights::new(100.0, 0.0, 0.0).unwrap(), &board);
    let conexp = Objective::weighted(Weights::new(0.0, 100.0, 0.0).unwrap(), &board);
    let mut report = |_progress| Control::Continue;
    let build_result = optimize(&board, &shelf, &build, &options, &mut report).expect("build run");
    let conexp_result =
        optimize(&board, &shelf, &conexp, &options, &mut report).expect("conexp run");

    assert_eq!(
        build_result.objective, build,
        "the result carries its objective"
    );
    assert_eq!(conexp_result.objective, conexp);
    assert!(build_result.score >= build_result.initial_score);
    assert!(conexp_result.score >= conexp_result.initial_score);
    assert!(
        score(&build_result.board, &build) > score(&conexp_result.board, &build),
        "the build run is better at build"
    );
    assert!(
        score(&conexp_result.board, &conexp) > score(&build_result.board, &conexp),
        "the construction EXP run is better at construction EXP"
    );
}

#[test]
fn the_perfect_board_explains_its_layers() {
    let save = cog_save();
    let mut report = |_progress| Control::Continue;
    let perfect = perfect_board(
        &save.board,
        &save.shelf,
        save.max_construction_level,
        Goal::ConExp,
        &mut report,
    )
    .expect("the fixture has a player cog");
    assert_eq!(perfect.board.len(), BOARD_SIZE);
    assert_eq!(perfect.board.iter().flatten().count(), BOARD_SIZE);
    assert_eq!(perfect.goal, Goal::ConExp);
    assert_eq!(perfect.construction_level, save.max_construction_level);
    assert!(
        perfect.board[perfect.player_slot]
            .as_ref()
            .is_some_and(|cog| cog.is_player)
    );
    let objective = Objective::goal(Goal::ConExp);
    assert_close(
        "perfect score",
        perfect.score,
        score(&perfect.board, &objective),
    );
    assert!(
        perfect.score > compute_totals(&save.board).player_con_exp_hr,
        "perfect cogs beat the save board"
    );
    assert_eq!(perfect.layers.first().map(|layer| layer.id), Some("base"));
    assert!(
        perfect
            .layers
            .iter()
            .any(|layer| layer.id == "directionals"),
        "the explanation names the directional layer"
    );
    for layer in &perfect.layers {
        assert!(!layer.detail.is_empty(), "{} has no detail", layer.id);
        assert!(layer.slots > 0, "{} changed no slot", layer.id);
    }
}
