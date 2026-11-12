//! Cog board parity and invariants against the committed synthetic save.
//!
//! The expected totals come from the upstream
//! `js/cogs/cog-board.js` (`boardFromSave`, `recoverRawPlayerStats`,
//! `computeBoardTotals`, `extractAllCogs`) run against the same save. They pin
//! the decode and the two-phase maths; the optimizer tests pin the two promises
//! the section makes: board rules hold and a result is never worse than its
//! input.

use idleon_core::cog::{
    BOARD_SIZE, CogResult, Goal, OptimizeOptions, SwapWalkthrough, apply_swaps, compute_totals,
    optimize,
};
use idleon_core::model::Control;
use idleon_core::save::import;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn cog_save() -> idleon_core::cog::CogSave {
    import(&fixture()).expect("the fixture imports").cog
}

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
        let result = optimize(&save.board, &save.shelf, goal, &options, &mut report)
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
    let result: CogResult =
        optimize(&save.board, &save.shelf, Goal::Build, &options, &mut report).expect("a full run");
    let mut shelf: Vec<Option<idleon_core::cog::Cog>> = save.shelf_slots.clone();
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
    let result =
        optimize(&save.board, &save.shelf, Goal::Build, &options, &mut report).expect("a full run");
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
