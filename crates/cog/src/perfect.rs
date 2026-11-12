//! The perfect board: the theoretical best placement of perfect cogs, and the
//! perfect stat a roll can reach at a construction level.
//!
//! Ported from `showPerfectBoard` in `cog-optimizer.html` and
//! `perfectCogStats` in `js/cogs/cog-judge.js`. Upstream builds one candidate
//! per player slot with a layered eviction: a perfect T3 base stat in every
//! slot, the player, T3 directionals, crystal fillers and crystal surrounds,
//! Excogia 2x2 blocks, then Yang cogs around the player. The best candidate by
//! the objective's raw metric wins, and the layers that built it are returned
//! so the section can explain the result.
//!
//! Two things are left out because this section does not decode them: the
//! tiny-cog board stays untouched, and the Yin/Yang inventory count comes from
//! the board and shelf only (upstream also counts the twelve off-board player
//! slots).

use idleon_common::Control;
use idleon_common::Progress;
use idleon_common::formula::ValueFormat;

use super::{
    BOARD_SIZE, Board, Cog, Direction, Goal, block_slots, goal_score, max_rolls, raw_magnitude,
    slot_to_pos, stat_from_value,
};

/// The perfect roll at a tier: max rolls, every roll at three times the raw
/// magnitude and every roll in the one stat.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PerfectStats {
    /// Perfect base build speed.
    pub a: f64,
    /// Perfect base Flaggy rate.
    pub c: f64,
    /// Perfect construction EXP percentage.
    pub d: f64,
    /// The highest surround the tier can roll.
    pub surround: f64,
    /// Three times the raw magnitude before rounding.
    pub mag: f64,
    /// How many stat rolls the tier gets.
    pub rolls: u32,
}

/// The perfect cog stats at `tier` and `construction_level`.
///
/// Crystal cogs (tier 4) roll their base stats at tier 4 and cap their surround
/// at `40 + 23 × 5 = 155`; Ulti cogs (tier 3) cap at 65.
pub fn perfect_stats(tier: usize, construction_level: u32) -> PerfectStats {
    let mag = 3.0 * raw_magnitude(tier, f64::from(construction_level));
    let rolls = max_rolls(tier);
    PerfectStats {
        a: stat_from_value(mag, Goal::Build) * rolls as f64,
        c: stat_from_value(mag, Goal::Flaggy) * rolls as f64,
        d: stat_from_value(mag, Goal::ConExp) * rolls as f64,
        surround: if tier >= 4 {
            155.0
        } else if tier == 3 {
            65.0
        } else {
            0.0
        },
        mag,
        rolls: rolls as u32,
    }
}

/// One layer of the perfect board's eviction order, in application order.
#[derive(Clone, Debug, PartialEq)]
pub struct PerfectLayer {
    /// The stable id the UI keys its row on.
    pub id: &'static str,
    /// The layer's name.
    pub label: &'static str,
    /// What it placed, with the numbers that explain the score.
    pub detail: String,
    /// How many slots it changed.
    pub slots: usize,
}

/// A finished perfect board.
#[derive(Clone, Debug, PartialEq)]
pub struct PerfectBoard {
    /// The perfect placement.
    pub board: Board,
    /// The goal the board maximises.
    pub goal: Goal,
    /// The raw goal metric of [`PerfectBoard::board`].
    pub score: f64,
    /// The player slot the winning candidate surrounds.
    pub player_slot: usize,
    /// The construction level the perfect stats were read at.
    pub construction_level: u32,
    /// How many Excogia 2x2 blocks the board places.
    pub excogia_blocks: usize,
    /// How many Yang cogs the board places around the player.
    pub yang_placed: usize,
    /// The layers that built the winning board, in order.
    pub layers: Vec<PerfectLayer>,
}

/// The first player cog on `board`, which the perfect board surrounds.
pub fn player_slot(board: &Board) -> Option<usize> {
    board
        .iter()
        .position(|cog| cog.as_ref().is_some_and(|cog| cog.is_player))
}

/// Build the perfect board for `goal`, reading the player and the Yin/Yang
/// inventory from `board` and `shelf`.
///
/// Returns `None` when the board has no player cog or the run is cancelled.
/// The player is swept across all 96 slots, as upstream does.
pub fn perfect_board(
    board: &Board,
    shelf: &[Cog],
    construction_level: u32,
    goal: Goal,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<PerfectBoard> {
    let player_slot = player_slot(board)?;
    let player = board.get(player_slot)?.clone()?;
    let yin_count = board
        .iter()
        .flatten()
        .filter(|cog| cog.name.starts_with("CogZA"))
        .count()
        + shelf
            .iter()
            .filter(|cog| cog.name.starts_with("CogZA"))
            .count();
    let yang_count = board
        .iter()
        .flatten()
        .filter(|cog| cog.name == "CogY")
        .count()
        + shelf.iter().filter(|cog| cog.name == "CogY").count();
    let context = Context {
        goal,
        construction_level,
        perf3: perfect_stats(3, construction_level),
        perf4: perfect_stats(4, construction_level),
        excogia_blocks: yin_count / 4,
        yang_count,
    };

    let mut best: Option<(f64, Candidate)> = None;
    for slot in 0..BOARD_SIZE {
        if slot % 8 == 0 {
            let progress = Progress {
                fraction: (slot as f64 / BOARD_SIZE as f64) as f32,
                message: format!("Perfect board · player slot {slot}/{BOARD_SIZE}"),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }
        let candidate = build_candidate(slot, &player, &context);
        let value = goal_score(&candidate.board, context.goal);
        if best
            .as_ref()
            .is_none_or(|(best_score, _)| value > *best_score)
        {
            best = Some((value, candidate));
        }
    }
    let (value, candidate) = best?;
    Some(PerfectBoard {
        board: candidate.board,
        goal,
        score: value,
        player_slot: candidate.player_slot,
        construction_level,
        excogia_blocks: candidate.excogia_blocks,
        yang_placed: candidate.yang_placed,
        layers: candidate.layers,
    })
}

/// What every candidate shares: the goal, the perfect stats and the inventory
/// counts.
struct Context {
    goal: Goal,
    construction_level: u32,
    perf3: PerfectStats,
    perf4: PerfectStats,
    excogia_blocks: usize,
    yang_count: usize,
}

struct Candidate {
    board: Board,
    player_slot: usize,
    excogia_blocks: usize,
    yang_placed: usize,
    layers: Vec<PerfectLayer>,
}

/// The base stat a goal reads from a perfect roll.
fn base_for(goal: Goal, stats: &PerfectStats) -> f64 {
    match goal {
        Goal::Build => stats.a,
        Goal::ConExp => stats.d,
        Goal::Flaggy => stats.c,
    }
}

/// A cog with only the goal's base stat set.
fn base_cog(name: &str, slot: usize, goal: Goal, base: f64) -> Cog {
    let mut cog = Cog::named(name, slot);
    match goal {
        Goal::Build => cog.a = base,
        Goal::ConExp => cog.d = base,
        Goal::Flaggy => cog.c = base,
    }
    cog
}

/// Set the goal's surround stat.
fn set_surround(cog: &mut Cog, goal: Goal, value: f64) {
    match goal {
        Goal::Build => cog.e = value,
        Goal::ConExp => cog.f = value,
        Goal::Flaggy => cog.g = value,
    }
}

/// The Yin block's tier at a construction level.
fn yin_tier(construction_level: u32) -> usize {
    if construction_level < 15 {
        0
    } else if construction_level < 40 {
        1
    } else if construction_level < 70 {
        2
    } else {
        3
    }
}

/// The Yin base stats at a construction level. Upstream derives the tier and
/// the budget from the level, then applies the same stat conversions.
fn yin_stats(construction_level: u32) -> (f64, f64, f64) {
    let level = f64::from(construction_level);
    let tier = yin_tier(construction_level);
    let budget = 2.5 * raw_magnitude(tier, level) * (1.5 + 0.5 * tier as f64);
    (
        budget.floor(),
        budget.powf(0.8).floor(),
        (budget.powf(0.4) + 10.0 * budget.log10() - 5.0)
            .floor()
            .max(2.0),
    )
}

/// One Yin piece of a 2x2 block: `everything` surround, `e = 1.25`, `f = 20`.
fn yin_cog(piece: usize, slot: usize, a: f64, c: f64, d: f64) -> Cog {
    let mut cog = Cog::named(&format!("CogZA0{piece}"), slot);
    cog.a = a;
    cog.c = c;
    cog.d = d;
    cog.e = 1.25;
    cog.f = 20.0;
    cog.direction = Some(Direction::Everything);
    cog.yin = true;
    cog
}

/// The Yang surround at a construction level: it scales with the level and is
/// the same on every Yang cog.
fn yang_surround(construction_level: u32) -> (f64, f64) {
    let level = f64::from(construction_level);
    let share = level / (level + 80.0);
    (
        (50.0 + share * 120.0).floor(),
        (50.0 + share * 160.0).floor(),
    )
}

fn yang_cog(slot: usize, e: f64, f: f64) -> Cog {
    let mut cog = Cog::named("CogY", slot);
    cog.e = e;
    cog.f = f;
    cog.direction = Some(Direction::Around);
    cog
}

/// Whether `direction` from `from` reaches `target`, as upstream `canTarget`.
fn can_target(from: usize, direction: Direction, target: usize) -> bool {
    let (from_col, from_row) = slot_to_pos(from);
    let (to_col, to_row) = slot_to_pos(target);
    match direction {
        Direction::Everything => true,
        Direction::Row => from_row == to_row,
        Direction::Column => from_col == to_col,
        _ => direction.offsets().iter().any(|(dc, dr)| {
            from_col as i32 + dc == to_col as i32 && from_row as i32 + dr == to_row as i32
        }),
    }
}

/// The T3 directional names upstream uses, with the direction each one is.
const T3_DIRECTIONS: [(Direction, &str); 7] = [
    (Direction::Row, "Cog3ro"),
    (Direction::Column, "Cog3co"),
    (Direction::Up, "Cog3up"),
    (Direction::Down, "Cog3do"),
    (Direction::Left, "Cog3le"),
    (Direction::Right, "Cog3ri"),
    (Direction::Corners, "Cog3cn"),
];

/// Build the perfect board with the player at `player_slot`, applying the seven
/// layers in upstream order.
fn build_candidate(player_slot: usize, player: &Cog, context: &Context) -> Candidate {
    let goal = context.goal;
    let base3 = base_for(goal, &context.perf3);
    let base4 = base_for(goal, &context.perf4);
    let mut board: Board = (0..BOARD_SIZE)
        .map(|slot| Some(base_cog("Cog3", slot, goal, base3)))
        .collect();
    let mut player = player.clone();
    player.slot = player_slot;
    board[player_slot] = Some(player);
    let mut layers = Vec::new();

    // Layer 1: a perfect T3 base stat in every slot.
    layers.push(PerfectLayer {
        id: "base",
        label: "T3 base fill",
        detail: format!(
            "Perfect T3 {} {} in all {BOARD_SIZE} slots",
            goal.label(),
            ValueFormat::Raw.format(base3)
        ),
        slots: BOARD_SIZE,
    });

    // Layer 3: T3 directionals, the best direction per slot if it improves.
    let mut directional_slots = 0;
    for slot in 0..BOARD_SIZE {
        if slot == player_slot {
            continue;
        }
        let original = board[slot].clone();
        let base_score = goal_score(&board, context.goal);
        let mut best: Option<(f64, Cog)> = None;
        for (direction, name) in T3_DIRECTIONS {
            let mut candidate = base_cog(name, slot, goal, base3);
            candidate.direction = Some(direction);
            set_surround(&mut candidate, goal, context.perf3.surround);
            board[slot] = Some(candidate.clone());
            let value = goal_score(&board, context.goal);
            if best
                .as_ref()
                .is_none_or(|(best_value, _)| value > *best_value)
            {
                best = Some((value, candidate));
            }
        }
        board[slot] = original;
        if let Some((value, candidate)) = best
            && value > base_score
        {
            board[slot] = Some(candidate);
            directional_slots += 1;
        }
    }
    if directional_slots > 0 {
        layers.push(PerfectLayer {
            id: "directionals",
            label: "T3 directionals",
            detail: format!(
                "Best of {} directions per slot, +{} surround",
                T3_DIRECTIONS.len(),
                context.perf3.surround
            ),
            slots: directional_slots,
        });
    }

    // Layer 4: crystal fillers replace any remaining non-directional T3 slot
    // when their perfect T4 base stat is higher.
    let mut crystal_fill = 0;
    for slot in 0..BOARD_SIZE {
        if slot == player_slot
            || board[slot]
                .as_ref()
                .is_some_and(|cog| cog.direction.is_some())
        {
            continue;
        }
        let original = board[slot].clone();
        let filler = base_cog("CogCry5", slot, goal, base4);
        board[slot] = Some(filler.clone());
        let value = goal_score(&board, context.goal);
        board[slot] = original;
        if value > goal_score(&board, context.goal) {
            board[slot] = Some(filler);
            crystal_fill += 1;
        }
    }
    if crystal_fill > 0 {
        layers.push(PerfectLayer {
            id: "crystal-fill",
            label: "Crystal fillers",
            detail: format!(
                "Perfect T4 {} {} where it beats the T3 fill",
                goal.label(),
                ValueFormat::Raw.format(base4)
            ),
            slots: crystal_fill,
        });
    }

    // Layer 5: crystal row/column surrounds. Crystal cogs cannot roll a Flaggy
    // surround, so the layer only exists for build and construction EXP.
    let mut crystal_surround = 0;
    if goal != Goal::Flaggy {
        for slot in 0..BOARD_SIZE {
            if slot == player_slot {
                continue;
            }
            let original = board[slot].clone();
            let base_score = goal_score(&board, context.goal);
            let mut best: Option<(f64, Cog)> = None;
            for direction in [Direction::Row, Direction::Column] {
                let mut candidate = base_cog("CogCry5", slot, goal, base4);
                candidate.direction = Some(direction);
                set_surround(&mut candidate, goal, context.perf4.surround);
                board[slot] = Some(candidate.clone());
                let value = goal_score(&board, context.goal);
                if best
                    .as_ref()
                    .is_none_or(|(best_value, _)| value > *best_value)
                {
                    best = Some((value, candidate));
                }
            }
            board[slot] = original;
            if let Some((value, candidate)) = best
                && value > base_score
            {
                board[slot] = Some(candidate);
                crystal_surround += 1;
            }
        }
    }
    if crystal_surround > 0 {
        layers.push(PerfectLayer {
            id: "crystal-surround",
            label: "Crystal surrounds",
            detail: format!("Row or column, +{} surround", context.perf4.surround),
            slots: crystal_surround,
        });
    }

    // Layer 6: complete Excogia blocks, each placed against the board as it
    // stands so the eviction cost is the real one.
    let (yin_a, yin_c, yin_d) = yin_stats(context.construction_level);
    let mut blocks = 0;
    for _ in 0..context.excogia_blocks {
        let mut best: Option<(f64, [usize; 4])> = None;
        for slots in block_slots() {
            if slots.contains(&player_slot) {
                continue;
            }
            if slots.iter().any(|slot| {
                board[*slot]
                    .as_ref()
                    .is_some_and(|cog| cog.name.starts_with("CogZA"))
            }) {
                continue;
            }
            let saved = slots.map(|slot| board[slot].clone());
            for (piece, slot) in slots.into_iter().enumerate() {
                board[slot] = Some(yin_cog(piece, slot, yin_a, yin_c, yin_d));
            }
            let value = goal_score(&board, context.goal);
            for (slot, cog) in slots.into_iter().zip(saved) {
                board[slot] = cog;
            }
            if best
                .as_ref()
                .is_none_or(|(best_value, _)| value > *best_value)
            {
                best = Some((value, slots));
            }
        }
        let Some((value, slots)) = best else {
            break;
        };
        if value <= goal_score(&board, context.goal) {
            break;
        }
        for (piece, slot) in slots.into_iter().enumerate() {
            board[slot] = Some(yin_cog(piece, slot, yin_a, yin_c, yin_d));
        }
        blocks += 1;
    }
    if blocks > 0 {
        layers.push(PerfectLayer {
            id: "excogia",
            label: "Excogia blocks",
            detail: format!(
                "2x2 Yin blocks, everything +20% con EXP, T{} base stats",
                yin_tier(context.construction_level)
            ),
            slots: blocks * 4,
        });
    }

    // Layer 7: Yang cogs, each taking the `around` slot with the best gain.
    let (yang_e, yang_f) = yang_surround(context.construction_level);
    let mut yang_placed = 0;
    for _ in 0..context.yang_count {
        let mut best: Option<(f64, usize)> = None;
        for slot in 0..BOARD_SIZE {
            if slot == player_slot {
                continue;
            }
            if board[slot]
                .as_ref()
                .is_some_and(|cog| cog.name == "CogY" || cog.name.starts_with("CogZA"))
            {
                continue;
            }
            if !can_target(slot, Direction::Around, player_slot) {
                continue;
            }
            let original = board[slot].clone();
            board[slot] = Some(yang_cog(slot, yang_e, yang_f));
            let value = goal_score(&board, context.goal);
            board[slot] = original;
            let gain = value - goal_score(&board, context.goal);
            if best.as_ref().is_none_or(|(best_gain, _)| gain > *best_gain) {
                best = Some((gain, slot));
            }
        }
        let Some((gain, slot)) = best else {
            break;
        };
        if gain <= 0.0 {
            break;
        }
        board[slot] = Some(yang_cog(slot, yang_e, yang_f));
        yang_placed += 1;
    }
    if yang_placed > 0 {
        layers.push(PerfectLayer {
            id: "yang",
            label: "Yang cogs",
            detail: format!(
                "Around the player, +{} build / +{} con EXP surround",
                ValueFormat::Raw.format(yang_e),
                ValueFormat::Raw.format(yang_f)
            ),
            slots: yang_placed,
        });
    }

    Candidate {
        board,
        player_slot,
        excogia_blocks: blocks,
        yang_placed,
        layers,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{empty_board, pos_to_slot};

    fn cog(name: &str, slot: usize) -> Cog {
        Cog::named(name, slot)
    }

    #[test]
    fn perfect_stats_match_the_upstream_formula() {
        // `perfectCogStats(3, 42)` from `cog-judge.js`: mag = 3 × rawMag(3, 42).
        let stats = perfect_stats(3, 42);
        let raw = raw_magnitude(3, 42.0);
        assert!((stats.mag - 3.0 * raw).abs() < 1e-9);
        assert_eq!(stats.rolls, 4);
        assert_eq!(stats.surround, 65.0);
        assert_eq!(stats.a, (3.0 * raw).round() * 4.0);
        assert_eq!(stats.c, (3.0 * raw).powf(0.8).round() * 4.0);
        // Crystal cogs use tier 4 and the crystal surround cap.
        let crystal = perfect_stats(4, 42);
        assert_eq!(crystal.surround, 155.0);
        assert!(crystal.a > stats.a);
        // Tier 0/1 get three rolls; lower tiers cannot roll a surround.
        assert_eq!(perfect_stats(0, 42).rolls, 3);
        assert_eq!(perfect_stats(1, 42).surround, 0.0);
    }

    #[test]
    fn perfect_stats_match_the_upstream_js_values() {
        // `perfectCogStats(tier, conLv)` from `js/cogs/cog-judge.js`, run on the
        // upstream checkout: (tier, conLv, a, c, d, surround).
        let expected = [
            (0, 42, 105.0, 51.0, 42.0, 0.0),
            (0, 8005, 140_964.0, 16_395.0, 345.0, 0.0),
            (1, 42, 138.0, 63.0, 48.0, 0.0),
            (1, 8005, 209_148.0, 22_479.0, 387.0, 0.0),
            (2, 42, 276.0, 120.0, 72.0, 0.0),
            (2, 8005, 413_776.0, 41_100.0, 584.0, 0.0),
            (3, 42, 516.0, 196.0, 92.0, 65.0),
            (3, 8005, 614_044.0, 56_360.0, 660.0, 65.0),
            (4, 42, 724.0, 256.0, 100.0, 155.0),
            (4, 8005, 911_020.0, 77_276.0, 748.0, 155.0),
        ];
        for (tier, level, a, c, d, surround) in expected {
            let stats = perfect_stats(tier, level);
            assert_eq!(stats.a, a, "tier {tier} level {level} a");
            assert_eq!(stats.c, c, "tier {tier} level {level} c");
            assert_eq!(stats.d, d, "tier {tier} level {level} d");
            assert_eq!(
                stats.surround, surround,
                "tier {tier} level {level} surround"
            );
        }
    }

    #[test]
    fn the_candidate_places_the_player_and_directionals() {
        let mut board = empty_board();
        let mut player = cog("Player_Ana", 0);
        player.b = 100.0;
        player.d = 20.0;
        board[0] = Some(player);
        let context = Context {
            goal: Goal::ConExp,
            construction_level: 42,
            perf3: perfect_stats(3, 42),
            perf4: perfect_stats(4, 42),
            excogia_blocks: 0,
            yang_count: 0,
        };
        let candidate = build_candidate(
            pos_to_slot(5, 5),
            board[0].as_ref().expect("the player"),
            &context,
        );
        assert_eq!(candidate.player_slot, pos_to_slot(5, 5));
        assert_eq!(
            candidate.board[candidate.player_slot]
                .as_ref()
                .map(|cog| cog.name.as_str()),
            Some("Player_Ana")
        );
        let layers: Vec<&str> = candidate.layers.iter().map(|layer| layer.id).collect();
        assert_eq!(
            layers,
            ["base", "directionals", "crystal-fill", "crystal-surround"]
        );
        assert!(
            candidate
                .board
                .iter()
                .flatten()
                .any(|cog| cog.direction.is_some()),
            "at least one directional reaches the player"
        );
        assert_eq!(
            candidate.board.iter().flatten().count(),
            BOARD_SIZE,
            "the perfect board fills every slot"
        );
    }

    #[test]
    fn yang_cogs_take_the_best_slot_around_the_player() {
        let mut board = empty_board();
        let mut player = cog("Player_Ana", 0);
        player.b = 100.0;
        player.d = 20.0;
        board[0] = Some(player);
        let mut shelf = Vec::new();
        for piece in 0..4 {
            shelf.push(cog(&format!("CogZA0{piece}"), 108 + piece));
        }
        shelf.push(cog("CogY", 115));
        let context = Context {
            goal: Goal::ConExp,
            construction_level: 42,
            perf3: perfect_stats(3, 42),
            perf4: perfect_stats(4, 42),
            excogia_blocks: 1,
            yang_count: 1,
        };
        let candidate = build_candidate(0, board[0].as_ref().expect("the player"), &context);
        assert_eq!(candidate.yang_placed, 1);
        let yang = candidate
            .board
            .iter()
            .flatten()
            .find(|cog| cog.name == "CogY")
            .expect("the Yang cog is placed");
        assert_eq!(yang.direction, Some(Direction::Around));
        assert!(can_target(yang.slot, Direction::Around, 0));
        // The T3 base fill beats the Yin block's base stats in the full
        // candidate, so upstream's layer keeps no block here; the layer only
        // appears when the block improves the score.
        assert_eq!(candidate.excogia_blocks, 0);
        assert_eq!(
            candidate
                .board
                .iter()
                .flatten()
                .filter(|cog| cog.yin)
                .count(),
            0
        );
    }

    #[test]
    fn yin_stats_follow_the_upstream_budget() {
        assert_eq!(yin_tier(14), 0);
        assert_eq!(yin_tier(15), 1);
        assert_eq!(yin_tier(39), 1);
        assert_eq!(yin_tier(40), 2);
        assert_eq!(yin_tier(70), 3);
        // The upstream `showPerfectBoard` formulas at two levels:
        // (conLv, yin a, yin c, yin d, yang e, yang f).
        let expected = [
            (42, 144.0, 53.0, 23.0, 91.0, 105.0),
            (100, 646.0, 177.0, 36.0, 116.0, 138.0),
        ];
        for (level, a, c, d, e, f) in expected {
            assert_eq!(yin_stats(level), (a, c, d), "level {level}");
            assert_eq!(yang_surround(level), (e, f), "level {level}");
        }
    }

    #[test]
    fn a_board_without_a_player_has_no_perfect_board() {
        let mut board = empty_board();
        board[0] = Some(cog("Cog3A00", 0));
        let mut report = |_progress| Control::Continue;
        assert!(perfect_board(&board, &[], 42, Goal::ConExp, &mut report).is_none());
    }

    #[test]
    fn the_full_sweep_returns_the_best_player_slot_and_fills_the_board() {
        let mut board = empty_board();
        let mut player = cog("Player_Ana", 0);
        player.b = 100.0;
        player.d = 20.0;
        board[0] = Some(player);
        let mut report = |_progress| Control::Continue;
        let perfect = perfect_board(&board, &[], 42, Goal::ConExp, &mut report)
            .expect("a perfect board needs only a player");
        assert_eq!(perfect.board.iter().flatten().count(), BOARD_SIZE);
        assert!(perfect.score > 0.0);
        assert_eq!(perfect.goal, Goal::ConExp);
        assert!(!perfect.layers.is_empty());
        assert_eq!(
            perfect.board[perfect.player_slot].as_ref().unwrap().name,
            "Player_Ana"
        );
    }

    #[test]
    fn a_cancelled_sweep_returns_nothing() {
        let mut board = empty_board();
        board[0] = Some(cog("Player_Ana", 0));
        let mut stop = |_progress| Control::Stop;
        assert!(perfect_board(&board, &[], 42, Goal::Build, &mut stop).is_none());
    }
}
