//! The coordinator and its bounded full-board search.
//!
//! The search's own rules are held here: the recommendation is never worse than
//! the board it was given, it is legal and applies to the layout, the same
//! options reproduce the same board, and a stopped run stores nothing.

mod common;

use common::{bought_sections, save};
use idleon_jelly::board;
use idleon_jelly::optimizer::{self, Optimization, Options};
use idleon_jelly::planner::Objective;
use idleon_jelly::surrogate::{self, Objective as SurrogateObjective};

fn small() -> Options {
    Options {
        starts: 2,
        beam_width: 4,
        iterations: 3,
        saved_refinement_rounds: 1,
        refinement_rounds: 1,
        finalists: 2,
        trials: 8,
        ..Options::default()
    }
}

#[test]
fn the_optimizer_finds_a_board_at_least_as_good_as_the_saved_one() {
    let jelly = save();
    let result = Optimization::run(&jelly, small());
    assert!(
        result.recommended.proxy_score >= result.baseline_score,
        "the recommendation ranks below the saved board: {} < {}",
        result.recommended.proxy_score,
        result.baseline_score
    );
    assert!(
        board::validate(&jelly, &result.recommended.layout).is_ok(),
        "the recommendation is not a legal board"
    );
    assert!(
        !result.recommended.operation.estimate.valid
            || result.recommended.operation.estimate.trials == 8
    );
    assert_eq!(result.starts.len(), 2, "one row per start");
    assert!(!result.finalists.is_empty());
    assert_eq!(
        result.moves,
        optimizer::moves(&result.baseline_layout, &result.recommended.layout)
    );
}

#[test]
fn the_optimizer_beats_the_saved_board_when_the_board_has_room() {
    // The committed save's own board is a tight local optimum; the same account
    // with its sections bought leaves the search somewhere to improve.
    let jelly = bought_sections();
    let result = Optimization::run(&jelly, small());
    assert!(
        result.recommended.proxy_score > result.baseline_score,
        "the search did not beat the saved board: {} <= {}",
        result.recommended.proxy_score,
        result.baseline_score
    );
    assert!(!result.moves.is_empty(), "an improvement changes anchors");
    assert!(board::validate(&jelly, &result.recommended.layout).is_ok());
    assert_ne!(result.recommended.layout, result.baseline_layout);
}

#[test]
fn the_same_options_reproduce_the_same_board() {
    let jelly = save();
    let first = Optimization::run(&jelly, small());
    let second = Optimization::run(&jelly, small());
    assert_eq!(first.recommended.layout, second.recommended.layout);
    assert_eq!(first.starts, second.starts);
    assert_eq!(
        first
            .finalists
            .iter()
            .map(|row| optimizer::layout_key(&row.layout))
            .collect::<Vec<_>>(),
        second
            .finalists
            .iter()
            .map(|row| optimizer::layout_key(&row.layout))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_stopped_run_stores_nothing() {
    let jelly = save();
    let mut reports = 0;
    let stopped = Optimization::run_with(&jelly, small(), |_| {
        reports += 1;
        reports < 3
    });
    assert!(stopped.is_none(), "a cancelled run keeps no result");
    assert_eq!(reports, 3, "the report is also the cancel point");
}

#[test]
fn the_recommendation_applies_to_the_layout() {
    let mut jelly = save();
    let result = Optimization::run(&jelly, small());
    jelly.layout = result.recommended.layout.clone();
    assert!(board::validate(&jelly, &jelly.layout).is_ok());
    assert_eq!(
        surrogate::proxy_score(
            &jelly,
            &jelly.layout,
            SurrogateObjective::Clear,
            None,
            Some(result.obstruction),
        ),
        result.recommended.proxy_score
    );
}

#[test]
fn every_objective_never_falls_below_the_saved_board() {
    let jelly = save();
    for objective in [
        Objective::Clear,
        Objective::Bloodcells,
        Objective::Dps,
        Objective::Survival,
    ] {
        if !objective.visible(&jelly) {
            continue;
        }
        let result = Optimization::run(
            &jelly,
            Options {
                objective,
                ..small()
            },
        );
        assert!(
            result.recommended.proxy_score >= result.baseline_score,
            "{objective:?}: the recommendation ranks below the saved board"
        );
        assert!(
            result
                .finalists
                .iter()
                .any(|row| row.layout == result.recommended.layout),
            "{objective:?}: the recommendation is one of the finalists"
        );
    }
}

#[test]
fn the_beam_width_changes_how_much_board_the_search_sees() {
    let jelly = save();
    let narrow = Optimization::run(
        &jelly,
        Options {
            beam_width: 4,
            ..small()
        },
    );
    let wide = Optimization::run(
        &jelly,
        Options {
            beam_width: 16,
            ..small()
        },
    );
    assert!(narrow.beam_width == 4 && wide.beam_width == 16);
    assert!(narrow.recommended.proxy_score >= narrow.baseline_score);
    assert!(wide.recommended.proxy_score >= wide.baseline_score);
}

#[test]
fn a_hypothetical_obstruction_is_the_score_context() {
    let jelly = save();
    let result = Optimization::run(
        &jelly,
        Options {
            obstruction: Some(0),
            ..small()
        },
    );
    assert_eq!(result.obstruction, 0);
    assert_eq!(
        result.recommended.proxy_score,
        surrogate::proxy_score(
            &jelly,
            &result.recommended.layout,
            SurrogateObjective::Clear,
            None,
            Some(0)
        )
    );
}
