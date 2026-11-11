//! Phase 1: the greedy construction that gives the annealer a strong start.
//!
//! Ported from `_greedyConstruct` in `js/cogs/cog-optimizer.js`: collect every
//! movable cog, rebuild the board around each possible player slot for
//! construction EXP, place Yin blocks, directionals and non-directionals, then
//! refine with pairwise and board↔shelf swaps.

use crate::model::{Control, Progress};

use super::super::{
    BOARD_H, BOARD_SIZE, BOARD_W, Board, Cog, Goal, compute_totals, empty_board, pos_to_slot,
    score, slot_to_pos,
};
use super::swap_slot_pool;
/// Phase 1: collect every movable cog, clear the board and rebuild it.
pub(super) fn greedy_construct(
    board: &mut Board,
    pool: &mut Vec<Option<Cog>>,
    goal: Goal,
    report: &mut dyn FnMut(Progress) -> Control,
) -> bool {
    let mut all: Vec<Cog> = Vec::new();
    for slot in 0..BOARD_SIZE {
        if let Some(cog) = board.get_mut(slot).and_then(Option::take) {
            all.push(cog);
        }
    }
    all.extend(pool.drain(..).flatten());
    if all.is_empty() {
        return true;
    }

    let mut players = Vec::new();
    let mut directional = Vec::new();
    let mut non_dir = Vec::new();
    let mut yin_by_piece: [Vec<Cog>; 4] = std::array::from_fn(|_| Vec::new());
    for cog in all {
        if cog.is_player {
            players.push(cog);
        } else if cog.yin {
            match cog.yin_piece() {
                Some(piece) if piece < 4 => yin_by_piece[piece].push(cog),
                _ => non_dir.push(cog),
            }
        } else if cog.direction.is_some() && cog.goal_surround(goal) > 0.0 {
            directional.push(cog);
        } else {
            non_dir.push(cog);
        }
    }
    let block_count = yin_by_piece.iter().map(Vec::len).min().unwrap_or(0);
    let mut yin_blocks: Vec<[Cog; 4]> = Vec::new();
    for _ in 0..block_count {
        yin_blocks.push([
            yin_by_piece[0].remove(0),
            yin_by_piece[1].remove(0),
            yin_by_piece[2].remove(0),
            yin_by_piece[3].remove(0),
        ]);
    }
    for bucket in yin_by_piece {
        non_dir.extend(bucket);
    }
    directional.sort_by(|a, b| b.goal_surround(goal).total_cmp(&a.goal_surround(goal)));
    non_dir.sort_by(|a, b| b.goal_base(goal).total_cmp(&a.goal_base(goal)));
    players.sort_by(|a, b| b.b.total_cmp(&a.b));

    if goal == Goal::ConExp && !players.is_empty() {
        let main_player = players[0].clone();
        let other_players = players[1..].to_vec();
        let mut best: Option<(f64, Board, Vec<Option<Cog>>)> = None;
        for player_slot in 0..BOARD_SIZE {
            if player_slot % 8 == 0 {
                let progress = Progress {
                    fraction: (0.15 * player_slot as f64 / BOARD_SIZE as f64) as f32,
                    message: format!(
                        "Greedy construction · player slot {player_slot}/{BOARD_SIZE}"
                    ),
                };
                if report(progress) == Control::Stop {
                    return false;
                }
            }
            let mut candidate = empty_board();
            let mut candidate_pool = Vec::new();
            candidate[player_slot] = Some(main_player.clone());
            for cog in &other_players {
                place_best(&mut candidate, &mut candidate_pool, cog.clone(), goal);
            }
            place_yin_blocks(
                &mut candidate,
                &mut candidate_pool,
                &yin_blocks,
                &[false; BOARD_SIZE],
                goal,
            );
            place_greedy_list(
                &mut candidate,
                &mut candidate_pool,
                directional.clone(),
                goal,
                &[false; BOARD_SIZE],
            );
            place_by_rearrangement(&mut candidate, &mut candidate_pool, &non_dir, goal);
            let value = score(&candidate, goal);
            if best
                .as_ref()
                .is_none_or(|(best_score, ..)| value > *best_score)
            {
                best = Some((value, candidate, candidate_pool));
            }
        }
        if let Some((_, candidate, candidate_pool)) = best {
            *board = candidate;
            *pool = candidate_pool;
        }
    } else {
        place_greedy_list(board, pool, players, goal, &[false; BOARD_SIZE]);
        place_yin_blocks(board, pool, &yin_blocks, &[false; BOARD_SIZE], goal);
        place_greedy_list(board, pool, directional, goal, &[false; BOARD_SIZE]);
        place_by_rearrangement(board, pool, &non_dir, goal);
    }

    let mut locked = [false; BOARD_SIZE];
    lock_yin_blocks(board, &mut locked);
    refine(board, pool, &locked, goal, 6);
    true
}

/// Place one cog at the empty slot that scores best, or send it to the pool.
fn place_best(board: &mut Board, pool: &mut Vec<Option<Cog>>, cog: Cog, goal: Goal) {
    let mut best_slot = None;
    let mut best_score = f64::NEG_INFINITY;
    for slot in 0..BOARD_SIZE {
        if board[slot].is_some() {
            continue;
        }
        board[slot] = Some(cog.clone());
        let value = score(board, goal);
        board[slot] = None;
        if value > best_score {
            best_score = value;
            best_slot = Some(slot);
        }
    }
    match best_slot {
        Some(slot) => board[slot] = Some(cog),
        None => pool.push(Some(cog)),
    }
}

/// Place a list of cogs one at a time at its best empty slot; overflow goes to
/// the pool.
fn place_greedy_list(
    board: &mut Board,
    pool: &mut Vec<Option<Cog>>,
    cogs: impl IntoIterator<Item = Cog>,
    goal: Goal,
    locked: &[bool; BOARD_SIZE],
) {
    for cog in cogs {
        let mut best_slot = None;
        let mut best_score = f64::NEG_INFINITY;
        for slot in 0..BOARD_SIZE {
            if locked[slot] || board[slot].is_some() {
                continue;
            }
            board[slot] = Some(cog.clone());
            let value = score(board, goal);
            board[slot] = None;
            if value > best_score {
                best_score = value;
                best_slot = Some(slot);
            }
        }
        match best_slot {
            Some(slot) => board[slot] = Some(cog),
            None => pool.push(Some(cog)),
        }
    }
}

/// Place non-directional cogs by the rearrangement inequality: the highest base
/// stat takes the slot the board already surrounds most for this goal.
fn place_by_rearrangement(
    board: &mut Board,
    pool: &mut Vec<Option<Cog>>,
    cogs: &[Cog],
    goal: Goal,
) {
    let totals = compute_totals(board);
    let mut empty: Vec<(usize, f64)> = (0..BOARD_SIZE)
        .filter(|slot| board[*slot].is_none())
        .map(|slot| {
            let surround = match goal {
                Goal::Build => totals.per_slot[slot].surround_build,
                Goal::ConExp => totals.per_slot[slot].surround_con_exp,
                Goal::Flaggy => totals.per_slot[slot].surround_flaggy,
            };
            (slot, surround)
        })
        .collect();
    empty.sort_by(|a, b| b.1.total_cmp(&a.1));
    let placed = empty.len().min(cogs.len());
    for (index, cog) in cogs.iter().take(placed).enumerate() {
        board[empty[index].0] = Some(cog.clone());
    }
    for cog in cogs.iter().skip(placed) {
        pool.push(Some(cog.clone()));
    }
}

/// One 2x2 Yin placement candidate: the clear rows/columns, the distance from
/// the players, the score and the eight cells saved for the trial.
type YinChoice = (usize, i32, f64, [usize; 4], [Option<Cog>; 4]);

/// Place Yin pieces as complete 2x2 blocks, preferring positions that leave
/// player rows and columns open for row/column directionals.
fn place_yin_blocks(
    board: &mut Board,
    pool: &mut Vec<Option<Cog>>,
    blocks: &[[Cog; 4]],
    locked: &[bool; BOARD_SIZE],
    goal: Goal,
) {
    let mut player_rows = [false; BOARD_H];
    let mut player_cols = [false; BOARD_W];
    for (slot, cog) in board.iter().enumerate().take(BOARD_SIZE) {
        if cog.as_ref().is_some_and(|cog| cog.is_player) {
            let (col, row) = slot_to_pos(slot);
            player_rows[row] = true;
            player_cols[col] = true;
        }
    }

    for block in blocks {
        let mut best: Option<YinChoice> = None;
        for row in 0..BOARD_H - 1 {
            for col in 0..BOARD_W - 1 {
                let slots = [
                    pos_to_slot(col, row),
                    pos_to_slot(col + 1, row),
                    pos_to_slot(col, row + 1),
                    pos_to_slot(col + 1, row + 1),
                ];
                if slots.iter().any(|slot| locked[*slot]) {
                    continue;
                }
                if slots.iter().any(|slot| {
                    board[*slot]
                        .as_ref()
                        .is_some_and(|cog| cog.is_player || cog.yin)
                }) {
                    continue;
                }
                let clear = usize::from(!player_rows[row])
                    + usize::from(!player_rows[row + 1])
                    + usize::from(!player_cols[col])
                    + usize::from(!player_cols[col + 1]);
                let distance = slots
                    .iter()
                    .map(|slot| {
                        let (col, row) = slot_to_pos(*slot);
                        (0..BOARD_SIZE)
                            .filter_map(|other| {
                                board[other].as_ref().filter(|cog| cog.is_player).map(|_| {
                                    let (other_col, other_row) = slot_to_pos(other);
                                    (col as i32 - other_col as i32)
                                        .abs()
                                        .max((row as i32 - other_row as i32).abs())
                                })
                            })
                            .min()
                            .unwrap_or(99)
                    })
                    .min()
                    .unwrap_or(99);
                let saved = slots.map(|slot| board[slot].clone());
                for (index, slot) in slots.iter().enumerate() {
                    board[*slot] = Some(block[index].clone());
                }
                let value = score(board, goal);
                for (index, slot) in slots.iter().enumerate() {
                    board[*slot] = saved[index].clone();
                }
                let better = match &best {
                    None => true,
                    Some((best_clear, best_distance, best_value, ..)) => {
                        clear > *best_clear
                            || (clear == *best_clear && distance > *best_distance)
                            || (clear == *best_clear
                                && distance == *best_distance
                                && value > *best_value)
                    }
                };
                if better {
                    best = Some((clear, distance, value, slots, saved));
                }
            }
        }
        match best {
            Some((_, _, _, slots, saved)) => {
                for (index, cog) in saved.into_iter().enumerate() {
                    if let Some(cog) = cog {
                        pool.push(Some(cog));
                    }
                    board[slots[index]] = Some(block[index].clone());
                }
            }
            None => pool.extend(block.iter().cloned().map(Some)),
        }
    }
}

/// Lock the slots of every valid 2x2 Yin block so the annealer cannot scatter
/// it; the game only pays its surround in that arrangement.
pub(super) fn lock_yin_blocks(board: &Board, locked: &mut [bool; BOARD_SIZE]) {
    for row in 0..BOARD_H - 1 {
        for col in 0..BOARD_W - 1 {
            let slots = [
                pos_to_slot(col, row),
                pos_to_slot(col + 1, row),
                pos_to_slot(col, row + 1),
                pos_to_slot(col + 1, row + 1),
            ];
            let pieces = slots.map(|slot| {
                board[slot]
                    .as_ref()
                    .filter(|cog| cog.yin)
                    .and_then(Cog::yin_piece)
            });
            if pieces == [Some(0), Some(1), Some(2), Some(3)] {
                for slot in slots {
                    locked[slot] = true;
                }
            }
        }
    }
}

/// Exhaustive pairwise and board↔shelf swaps, up to `max_passes`.
fn refine(
    board: &mut Board,
    pool: &mut [Option<Cog>],
    locked: &[bool; BOARD_SIZE],
    goal: Goal,
    max_passes: usize,
) {
    let mut improved = true;
    let mut passes = 0;
    while improved && passes < max_passes {
        improved = false;
        passes += 1;
        for a in 0..BOARD_SIZE {
            if locked[a] {
                continue;
            }
            let mut current = score(board, goal);
            for b in a + 1..BOARD_SIZE {
                if locked[b] || (board[a].is_none() && board[b].is_none()) {
                    continue;
                }
                board.swap(a, b);
                let value = score(board, goal);
                if value > current {
                    current = value;
                    improved = true;
                } else {
                    board.swap(a, b);
                }
            }
        }
        if !pool.is_empty() {
            for (slot, locked_slot) in locked.iter().enumerate().take(BOARD_SIZE) {
                if *locked_slot {
                    continue;
                }
                let base = score(board, goal);
                let mut best_delta = 0.0;
                let mut best_index = None;
                for index in 0..pool.len() {
                    swap_slot_pool(board, pool, slot, index);
                    let value = score(board, goal);
                    swap_slot_pool(board, pool, slot, index);
                    if value - base > best_delta {
                        best_delta = value - base;
                        best_index = Some(index);
                    }
                }
                if let Some(index) = best_index {
                    swap_slot_pool(board, pool, slot, index);
                    improved = true;
                }
            }
        }
    }
}
