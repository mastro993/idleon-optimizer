//! The coordinator and its bounded full-board search.
//!
//! `fixtures/jelly.json` carries the upstream `jellySearchAssignment` and
//! `jellySelectSharedElites` rows for the committed save, so the coordinator's
//! focus rotation, its refine override and its elite selection are compared
//! field for field. The search's own rules are held here: the recommendation is
//! never worse than the board it was given, it is legal and applies to the
//! layout, the same options reproduce the same board, and a stopped run stores
//! nothing.

mod common;

use common::{close, jelly};
use idleon_jelly::board::{self, Layout};
use idleon_jelly::optimizer::{self, Focus, Optimization, Options};
use idleon_jelly::planner::Objective;
use idleon_jelly::surrogate::{self, Objective as SurrogateObjective};
use serde_json::Value;
use std::collections::BTreeMap;

fn layout_of(value: &Value) -> Layout {
    value
        .as_object()
        .expect("a layout object")
        .iter()
        .map(|(anchor, cell_type)| {
            (
                anchor.parse().expect("a numeric anchor"),
                cell_type.as_u64().expect("a cell type") as usize,
            )
        })
        .collect::<BTreeMap<usize, usize>>()
}

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

fn start_of(row: &Value) -> optimizer::Start {
    optimizer::Start {
        start: row["start"].as_u64().expect("a start") as usize,
        focus: Focus::from_id(row["assignment"]["focus"].as_str().expect("a focus"))
            .expect("a known focus"),
        layout: layout_of(&row["result"]["layout"]),
        score: row["result"]["score"].as_f64().expect("a score"),
        elapsed_ms: row["elapsedMs"].as_f64().expect("an elapsed measure"),
        baseline_score: row["assignment"]["baselineScore"].as_f64(),
    }
}

fn coordinator_options() -> Options {
    Options {
        iterations: 20,
        saved_refinement_rounds: 3,
        ..Options::default()
    }
}

#[test]
fn the_coordinator_matches_upstream() {
    let (jelly, case) = jelly("optimizer-coordinator");
    let base: Vec<optimizer::Start> = case["coordinatorRows"]
        .as_array()
        .expect("the fixture lists the rows")
        .iter()
        .map(start_of)
        .collect();

    let expected_elites = case["coordinatorElites"]
        .as_array()
        .expect("the fixture lists the elites");
    let elites = optimizer::shared_elites(&base, optimizer::SHARED_ELITES);
    assert_eq!(elites.len(), expected_elites.len(), "elite count");
    for (elite, expected) in elites.iter().zip(expected_elites) {
        assert_eq!(
            elite.source_start,
            expected["sourceStart"].as_u64().expect("a start") as usize,
            "elite source"
        );
        assert_eq!(elite.layout, layout_of(&expected["layout"]), "elite layout");
    }

    let expected = case["coordinator"]
        .as_array()
        .expect("the fixture lists the assignments");
    let mut running: Vec<optimizer::Start> = Vec::new();
    for (start, expected) in expected.iter().enumerate() {
        let assignment = optimizer::assignment(&running, start, &coordinator_options());
        assert_eq!(
            assignment.focus,
            Focus::from_id(expected["focus"].as_str().expect("a focus")).expect("a known focus"),
            "start {start}: focus"
        );
        assert_eq!(
            assignment.iterations,
            expected["overrides"]["iterations"]
                .as_u64()
                .expect("build rounds") as usize,
            "start {start}: build rounds"
        );
        assert_eq!(
            assignment.saved_refinement_rounds,
            expected["overrides"]["savedRefinementRounds"]
                .as_u64()
                .expect("saved-board rounds") as usize,
            "start {start}: saved-board rounds"
        );
        assert_eq!(
            assignment.active_exact_node_budget,
            Some(
                expected["overrides"]["activeExactNodeBudget"]
                    .as_u64()
                    .expect("the repair budget") as usize
            )
            .filter(|budget| *budget > 0),
            "start {start}: repair budget"
        );
        assert_eq!(
            assignment.source_starts,
            expected["sourceStarts"]
                .as_array()
                .expect("the source starts")
                .iter()
                .map(|start| start.as_u64().expect("a start") as usize)
                .collect::<Vec<_>>(),
            "start {start}: source starts"
        );
        match expected["baselineScore"].as_f64() {
            Some(score) => close(
                assignment.baseline_score.expect("a baseline"),
                score,
                "the baseline score",
            ),
            None => assert!(assignment.baseline_score.is_none()),
        }
        let mut row = base[start % base.len()].clone();
        row.start = start;
        row.focus = assignment.focus;
        running.push(row);
    }
    let _ = jelly;
}

#[test]
fn the_ranking_score_matches_upstream() {
    let (jelly, case) = jelly("optimizer-coordinator");
    let scores = case["optimizerScores"]
        .as_array()
        .expect("the fixture lists the scores");
    assert!(!scores.is_empty());
    for row in scores {
        let objective = row["objective"].as_str().expect("an objective");
        let objective = match objective {
            "dps" => SurrogateObjective::Dps,
            "bloodcells" => SurrogateObjective::Bloodcells,
            "exp" => SurrogateObjective::Exp,
            "survival" => SurrogateObjective::Survival,
            "clear" => SurrogateObjective::Clear,
            other => panic!("unknown objective {other}"),
        };
        let layout = layout_of(&row["layout"]);
        close(
            surrogate::proxy_score(&jelly, &layout, objective, None, None),
            row["score"].as_f64().expect("a score"),
            &format!("{objective:?} proxy score"),
        );
    }
}

#[test]
fn the_optimizer_finds_a_board_at_least_as_good_as_the_saved_one() {
    let (jelly, _) = jelly("save");
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
    let (jelly, _) = jelly("optimizer-coordinator");
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
    let (jelly, _) = jelly("save");
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
    let (jelly, _) = jelly("save");
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
    let (mut jelly, _) = jelly("save");
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
    let (jelly, _) = jelly("save");
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
    let (jelly, _) = jelly("save");
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
    let (jelly, _) = jelly("save");
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
