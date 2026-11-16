//! The board insight and roll range invariants: the insight score is the
//! optimizer's own score, every contribution is the score lost by removing its
//! cog, and the ranges follow the construction level.

mod common;

use common::cog_save;
use idleon_cog::{
    Goal, Objective, OptimizeOptions, board_insights, compute_totals, goal_score, optimize,
    roll_ranges,
};
use idleon_common::Control;

#[test]
fn the_insight_score_is_the_optimizer_score() {
    let save = cog_save();
    let options = OptimizeOptions {
        iterations: 10,
        ..OptimizeOptions::default()
    };
    for goal in Goal::ALL {
        let insights = board_insights(&save.board, goal).expect("insights");
        assert_eq!(insights.base_score, goal_score(&save.board, goal));
        // The optimizer measures the same board, so its starting score is the
        // insight's own; a result is never worse than it.
        let mut report = |_progress| Control::Continue;
        let objective = Objective::goal(goal);
        let result = optimize(&save.board, &save.shelf, &objective, &options, &mut report)
            .expect("a short run completes");
        assert_eq!(result.initial_score, insights.base_score, "{}", goal.id());
        assert!(result.score >= insights.base_score, "{}", goal.id());

        // Every contribution is the score lost by removing that cog, including
        // the surrounds it gave its neighbours.
        for impact in &insights.impacts {
            let mut without = save.board.clone();
            without[impact.slot] = None;
            let removed = goal_score(&without, goal);
            assert!(
                (insights.base_score - removed - impact.delta).abs() < 1e-9,
                "{}: slot {}",
                goal.id(),
                impact.slot
            );
        }
    }
}

#[test]
fn the_ranges_follow_the_construction_level() {
    let save = cog_save();
    assert!(
        save.max_construction_level > 100,
        "the fixture carries a high construction level"
    );
    let low = roll_ranges(Goal::ConExp, 100);
    let high = roll_ranges(Goal::ConExp, save.max_construction_level);
    for (low, high) in low.iter().zip(&high) {
        assert!(high.per_roll_max > low.per_roll_max, "tier {}", low.tier);
        assert!(high.total_max > low.total_max, "tier {}", low.tier);
        // The surround range is tier-fixed, not level-driven.
        assert_eq!(low.min_surround, high.min_surround);
        assert_eq!(low.max_surround, high.max_surround);
    }
    // The board's totals are unchanged by a range question.
    let totals = compute_totals(&save.board);
    assert!(totals.total_build > 0.0);
}
