//! The board insights: every movable cog's marginal contribution to the
//! objective, and what to roll to replace the weakest ones.
//!
//! Ported from `computeBoardInsights` in `cog-optimizer.html`. A cog's
//! contribution is measured the way the game's surround pass makes it: the cog
//! is removed from a copy of the board and the objective is scored again, so a
//! directional cog's value includes the surrounds it gave its neighbours.
//!
//! Player, Yin and Yang cogs are not movable advice targets — the optimizer
//! repositions them, but there is nothing to re-roll on them — so the insight
//! table skips them exactly as upstream does.

use super::{
    Board, Cog, CogKind, Goal, affects_player, classify, goal_score, slot_to_pos, stat_tier,
};

/// How much one board cog contributes to the objective: removing it changes the
/// score by `delta`.
#[derive(Clone, Debug, PartialEq)]
pub struct Insight {
    pub slot: usize,
    pub cog: Cog,
    /// The score lost by removing the cog, so a negative value means the cog is
    /// a net drag on the board.
    pub delta: f64,
    /// The same drop as a share of the board's score.
    pub pct: f64,
}

/// One replacement the weakest cogs suggest: which slot, why it is weak and
/// what a better roll would carry.
#[derive(Clone, Debug, PartialEq)]
pub struct RollTarget {
    pub slot: usize,
    pub col: usize,
    pub row: usize,
    /// The display name of the cog the target replaces.
    pub cog: String,
    /// The measured reason the cog is weak, e.g. `d = 201, non-directional`.
    pub why: String,
    /// What to roll for, e.g. `Roll Ulti with d > 201`.
    pub roll: String,
    pub delta: f64,
    pub pct: f64,
}

/// The board's weakest and strongest movable cogs, every contribution and the
/// replacement targets the section lists.
#[derive(Clone, Debug, PartialEq)]
pub struct BoardInsights {
    /// The objective's score for the board the insights were measured on.
    pub base_score: f64,
    /// Every movable cog, sorted by its contribution, weakest first.
    pub impacts: Vec<Insight>,
    pub weakest: Insight,
    pub best: Insight,
    /// Up to three replacements, drawn from the five weakest cogs.
    pub roll_targets: Vec<RollTarget>,
}

/// The board insights for `goal`, or `None` when the board scores nothing or
/// has nothing movable to re-roll.
pub fn board_insights(board: &Board, goal: Goal) -> Option<BoardInsights> {
    let base_score = goal_score(board, goal);
    if base_score <= 0.0 {
        return None;
    }
    let mut working = board.clone();
    let mut impacts = Vec::new();
    for slot in 0..working.len() {
        let Some(cog) = working[slot].clone() else {
            continue;
        };
        if matches!(
            classify(&cog.name),
            CogKind::Player(_) | CogKind::Excogia { .. } | CogKind::Yellow
        ) {
            continue;
        }
        working[slot] = None;
        let delta = base_score - goal_score(&working, goal);
        working[slot] = Some(cog.clone());
        impacts.push(Insight {
            slot,
            cog,
            delta,
            pct: delta / base_score * 100.0,
        });
    }
    if impacts.is_empty() {
        return None;
    }
    impacts.sort_by(|left, right| left.delta.total_cmp(&right.delta));
    let weakest = impacts[0].clone();
    let best = impacts[impacts.len() - 1].clone();
    let roll_targets = impacts
        .iter()
        .take(5)
        .map(|impact| roll_target(board, goal, impact))
        .take(3)
        .collect();
    Some(BoardInsights {
        base_score,
        impacts,
        weakest,
        best,
        roll_targets,
    })
}

/// The replacement one measured weak cog suggests, in upstream's wording.
fn roll_target(board: &Board, goal: Goal, insight: &Insight) -> RollTarget {
    let cog = &insight.cog;
    // The goal's base and surround stat keys, as the page prints them.
    let (base_key, surround_key, surround_label) = match goal {
        Goal::Build => ("a", "e", "Build Speed Surround"),
        Goal::ConExp => ("d", "f", "Construction EXP Surround"),
        Goal::Flaggy => ("c", "g", "Flaggy Rate Surround"),
    };
    let base = match goal {
        Goal::Build => cog.a,
        Goal::ConExp => cog.d,
        Goal::Flaggy => cog.c,
    };
    let surround = match goal {
        Goal::Build => cog.e,
        Goal::ConExp => cog.f,
        Goal::Flaggy => cog.g,
    };
    let direction = cog.direction.map(|direction| direction.name());
    let player_adjacent = affects_player(board, insight.slot);
    let tier = stat_tier(&cog.name);
    let (why, roll) = if let Some(tier) = tier
        && tier < 3
    {
        if player_adjacent {
            (
                format!("T{tier} cog (low tier)"),
                format!("Roll Ulti directional with {surround_label} ≥ 15%"),
            )
        } else {
            (
                format!("T{tier} cog (low tier)"),
                format!("Roll any Ulti cog with high {base_key}"),
            )
        }
    } else if player_adjacent && surround <= 0.0 {
        let direction = direction.unwrap_or_default();
        (
            format!("has {direction} direction but no {surround_label}"),
            format!("Roll Ulti {direction} with {surround_label}"),
        )
    } else if player_adjacent && surround < 15.0 {
        let direction = direction.unwrap_or_default();
        (
            format!("{surround_key} = {surround}% (low)"),
            format!("Roll Ulti {direction} with {surround_key} > {surround}%"),
        )
    } else if direction.is_none() {
        (
            format!("{base_key} = {}, non-directional", fmt_big(base)),
            format!("Roll Ulti with {base_key} > {}", fmt_big(base)),
        )
    } else {
        let mut roll = format!("Roll Ulti with {base_key} > {}", fmt_big(base));
        if player_adjacent && surround > 0.0 {
            roll.push_str(&format!(" + {surround_key} > {surround}%"));
        }
        (format!("{base_key} = {} (low)", fmt_big(base)), roll)
    };
    let (col, row) = slot_to_pos(insight.slot);
    RollTarget {
        slot: insight.slot,
        col,
        row,
        cog: cog.display_name(),
        why,
        roll,
        delta: insight.delta,
        pct: insight.pct,
    }
}

/// The page's own `fmtBig` formatter, used in the roll targets' wording.
///
/// It rounds up at each suffix rather than to the nearest, so `12 345` reads
/// `12.4K`; the app's own formatter deliberately differs and is not used here.
fn fmt_big(n: f64) -> String {
    if n == 0.0 {
        return "0".into();
    }
    let magnitude = n.abs();
    let sign = if n < 0.0 { "-" } else { "" };
    if magnitude < 100.0 {
        return if n.fract() == 0.0 {
            format!("{}", n as i64)
        } else {
            format!("{n:.2}")
        };
    }
    if magnitude < 1e3 {
        return format!("{sign}{}", magnitude.floor());
    }
    // The page rounds up at a tenth or a hundredth before naming the suffix:
    // `Math.ceil(v / 10) / 100 + 'K'` for 1 000-9 999, and so on.
    let (ceil_divisor, post_divisor, suffix) = if magnitude < 1e4 {
        (10.0, 100.0, "K")
    } else if magnitude < 1e5 {
        (100.0, 10.0, "K")
    } else if magnitude < 1e6 {
        (1e3, 1.0, "K")
    } else if magnitude < 1e7 {
        (1e4, 100.0, "M")
    } else if magnitude < 1e8 {
        (1e5, 10.0, "M")
    } else if magnitude < 1e10 {
        (1e6, 1.0, "M")
    } else if magnitude < 1e13 {
        (1e9, 1.0, "B")
    } else if magnitude < 1e16 {
        (1e12, 1.0, "T")
    } else if magnitude < 1e19 {
        (1e15, 1.0, "Q")
    } else if magnitude < 1e22 {
        (1e18, 1.0, "QQ")
    } else if magnitude < 1e24 {
        (1e21, 1.0, "QQQ")
    } else {
        let exponent = magnitude.log10().floor() as i32;
        let mantissa = magnitude / 10_f64.powi(exponent);
        return format!("{sign}{mantissa:.2}e{exponent:+}");
    };
    let value = (magnitude / ceil_divisor).ceil() / post_divisor;
    format!("{sign}{value}{suffix}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Direction, empty_board};

    fn player(slot: usize, b: f64) -> Cog {
        let mut cog = Cog::named("Player_Ana", slot);
        cog.b = b;
        cog.c = 500.0;
        cog
    }

    fn directional(name: &str, slot: usize, direction: Direction, f: f64) -> Cog {
        let mut cog = Cog::named(name, slot);
        cog.direction = Some(direction);
        cog.f = f;
        cog
    }

    /// A player, a strong tier-3 cog beside it, a weak one and the two special
    /// cogs the insights never target.
    fn board() -> Board {
        let mut board = empty_board();
        board[14] = Some(player(14, 100.0));
        let mut best = directional("Cog3A00", 13, Direction::Adjacent, 40.0);
        best.d = 300.0;
        board[13] = Some(best);
        let mut weak = directional("Cog3B00", 15, Direction::Right, 0.0);
        weak.d = 5.0;
        board[15] = Some(weak);
        let mut yin = Cog::named("CogZA00", 16);
        yin.yin = true;
        yin.d = 50.0;
        board[16] = Some(yin);
        let mut yang = Cog::named("CogY", 17);
        yang.d = 25.0;
        board[17] = Some(yang);
        board
    }

    #[test]
    fn the_insights_measure_each_removal_like_the_optimizer_scores() {
        let board = board();
        let insights = board_insights(&board, Goal::ConExp).expect("insights");
        assert_eq!(insights.base_score, goal_score(&board, Goal::ConExp));
        // The player, the Yin and the Yang cogs are not re-roll targets.
        assert!(!insights.impacts.iter().any(|impact| impact.slot >= 16));
        // The strongest cog carries the most EXP, so its removal costs most.
        assert_eq!(insights.best.slot, 13);
        // The weakest contributes nothing at all.
        assert_eq!(insights.weakest.slot, 15);
        assert!(insights.weakest.delta <= insights.best.delta);
        for impact in &insights.impacts {
            let mut without = board.clone();
            without[impact.slot] = None;
            assert!(
                (insights.base_score - goal_score(&without, Goal::ConExp) - impact.delta).abs()
                    < 1e-9
            );
            assert!((impact.delta / insights.base_score * 100.0 - impact.pct).abs() < 1e-12);
        }
    }

    #[test]
    fn a_low_tier_cog_suggests_an_ulti_roll() {
        let mut board = board();
        // Tier 1, pointing at the player.
        let mut low = directional("Cog1ri", 13, Direction::Right, 10.0);
        low.a = 3.0;
        board[13] = Some(low);
        board[14] = Some(player(14, 100.0));
        let insights = board_insights(&board, Goal::Build).expect("insights");
        let target = insights
            .roll_targets
            .iter()
            .find(|target| target.slot == 13)
            .expect("the tier 1 cog is a target");
        assert_eq!(target.why, "T1 cog (low tier)");
        assert_eq!(
            target.roll,
            "Roll Ulti directional with Build Speed Surround ≥ 15%"
        );

        // The same cog away from the player only asks for a better base stat.
        board[14] = None;
        board[0] = Some(player(0, 100.0));
        let insights = board_insights(&board, Goal::Build).expect("insights");
        let target = insights
            .roll_targets
            .iter()
            .find(|target| target.slot == 13)
            .expect("the tier 1 cog is still a target");
        assert_eq!(target.roll, "Roll any Ulti cog with high a");
    }

    #[test]
    fn targets_name_the_direction_the_surround_and_the_base_stat() {
        // A tier 3 cog pointing right at the player (slot 1 → slot 2).
        let mut board = empty_board();
        board[2] = Some(player(2, 100.0));
        board[1] = Some(directional("Cog3ri", 1, Direction::Right, 0.0));
        let target = board_insights(&board, Goal::ConExp)
            .expect("insights")
            .roll_targets
            .remove(0);
        assert_eq!(
            target.why,
            "has right direction but no Construction EXP Surround"
        );
        assert_eq!(
            target.roll,
            "Roll Ulti right with Construction EXP Surround"
        );

        // A low surround is named with its value.
        board[1] = Some(directional("Cog3ri", 1, Direction::Right, 9.0));
        let target = board_insights(&board, Goal::ConExp)
            .expect("insights")
            .roll_targets
            .remove(0);
        assert_eq!(target.why, "f = 9% (low)");
        assert_eq!(target.roll, "Roll Ulti right with f > 9%");

        // A non-directional cog is judged on its base stat.
        let mut loose = Cog::named("Cog3A00", 1);
        loose.d = 2_500.0;
        board[1] = Some(loose);
        let target = board_insights(&board, Goal::ConExp)
            .expect("insights")
            .roll_targets
            .remove(0);
        assert_eq!(target.why, "d = 2.5K, non-directional");
        assert_eq!(target.roll, "Roll Ulti with d > 2.5K");

        // A directional cog beside the player carries both bounds.
        let mut strong = directional("Cog3ri", 1, Direction::Right, 40.0);
        strong.a = 12.0;
        strong.e = 40.0;
        board[1] = Some(strong);
        let target = board_insights(&board, Goal::Build)
            .expect("insights")
            .roll_targets
            .remove(0);
        assert_eq!(target.why, "a = 12 (low)");
        assert_eq!(target.roll, "Roll Ulti with a > 12 + e > 40%");
    }

    #[test]
    fn a_non_directional_cog_next_to_the_player_is_judged_on_its_base_stat() {
        // The player-adjacent branch only reads a cog that has a direction, so
        // a non-directional cog is named by its base stat even beside one.
        let mut board = empty_board();
        board[14] = Some(player(14, 100.0));
        board[13] = Some(Cog::named("Cog3A00", 13));
        let target = board_insights(&board, Goal::Flaggy)
            .expect("insights")
            .roll_targets
            .remove(0);
        assert_eq!(target.slot, 13);
        assert_eq!(target.why, "c = 0, non-directional");
        assert_eq!(target.roll, "Roll Ulti with c > 0");
    }

    #[test]
    fn a_board_with_nothing_to_roll_has_no_insights() {
        let mut board = empty_board();
        board[0] = Some(player(0, 100.0));
        assert!(board_insights(&board, Goal::ConExp).is_none());
        assert!(board_insights(&empty_board(), Goal::Build).is_none());
    }

    #[test]
    fn the_target_formatter_rounds_up_at_each_suffix() {
        assert_eq!(fmt_big(0.0), "0");
        assert_eq!(fmt_big(12.0), "12");
        assert_eq!(fmt_big(12.347), "12.35");
        assert_eq!(fmt_big(845.7), "845");
        assert_eq!(fmt_big(2_500.0), "2.5K");
        assert_eq!(fmt_big(12_345.0), "12.4K");
        assert_eq!(fmt_big(99_999.0), "100K");
        assert_eq!(fmt_big(1_234_567.0), "1.24M");
        assert_eq!(fmt_big(-1_500.0), "-1.5K");
    }
}
