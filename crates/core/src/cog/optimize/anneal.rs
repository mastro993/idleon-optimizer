//! Phase 2 to 4: the annealer's moves, its goal-aware bias and the finishing
//! refinements.
//!
//! Ported from the smart-move, bias-slot, greedy-finish and swap-back passes of
//! `js/cogs/cog-optimizer.js`.

use super::super::{
    BOARD_H, BOARD_SIZE, BOARD_W, Board, Cog, Direction, Goal, affects_player, compute_totals,
    pos_to_slot, score, slot_to_pos,
};
use super::swap_slot_pool;

/// A seeded xorshift64* generator, so a run is reproducible.
pub(super) struct Rng(u64);

impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next_u64(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    pub(super) fn unit(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }

    pub(super) fn below(&mut self, limit: usize) -> usize {
        if limit == 0 {
            0
        } else {
            (self.next_u64() % limit as u64) as usize
        }
    }
}

/// A move the annealer can apply; swaps are their own inverse.
#[derive(Clone, Copy, Debug)]
pub(super) enum Move {
    BoardSwap { a: usize, b: usize },
    PoolSwap { slot: usize, pool: usize },
}

impl Move {
    pub(super) fn apply(self, board: &mut Board, pool: &mut [Option<Cog>]) {
        match self {
            Move::BoardSwap { a, b } => board.swap(a, b),
            Move::PoolSwap { slot, pool: index } => swap_slot_pool(board, pool, slot, index),
        }
    }
}

/// The slots a goal reads: the eight neighbours of every player for
/// construction EXP, and the surrounding directional slots for build and Flaggy.
pub(super) fn bias_slots(board: &Board, goal: Goal) -> [bool; BOARD_SIZE] {
    let mut bias = [false; BOARD_SIZE];
    if goal == Goal::ConExp {
        for (slot, cog) in board.iter().enumerate().take(BOARD_SIZE) {
            if !cog.as_ref().is_some_and(|cog| cog.is_player) {
                continue;
            }
            let (col, row) = slot_to_pos(slot);
            for dr in -1..=1 {
                for dc in -1..=1 {
                    if dr == 0 && dc == 0 {
                        continue;
                    }
                    let (target_col, target_row) = (col as i32 + dc, row as i32 + dr);
                    if target_col >= 0
                        && target_col < BOARD_W as i32
                        && target_row >= 0
                        && target_row < BOARD_H as i32
                    {
                        bias[pos_to_slot(target_col as usize, target_row as usize)] = true;
                    }
                }
            }
        }
        return bias;
    }
    let totals = compute_totals(board);
    for slot in 0..BOARD_SIZE {
        if board[slot]
            .as_ref()
            .is_some_and(|cog| cog.direction.is_some() && cog.goal_surround(goal) > 0.0)
        {
            bias[slot] = true;
        }
    }
    let mut values: Vec<(usize, f64)> = (0..BOARD_SIZE)
        .map(|slot| {
            let surround = match goal {
                Goal::Build => totals.per_slot[slot].surround_build,
                Goal::Flaggy => totals.per_slot[slot].surround_flaggy,
                Goal::ConExp => 0.0,
            };
            (slot, surround)
        })
        .collect();
    values.sort_by(|a, b| b.1.total_cmp(&a.1));
    let top = (values.len() / 4).max(1);
    for (slot, value) in values.into_iter().take(top) {
        if value > 0.0 {
            bias[slot] = true;
        }
    }
    bias
}

pub(super) fn smart_move(
    board: &Board,
    pool: &[Option<Cog>],
    locked: &[bool; BOARD_SIZE],
    goal: Goal,
    bias: &[bool; BOARD_SIZE],
    rng: &mut Rng,
) -> Option<Move> {
    let roll = rng.unit();
    if roll < 0.30 && !pool.is_empty() {
        let slot = weak_slot(board, locked, goal, rng).or_else(|| random_slot(locked, rng))?;
        return Some(Move::PoolSwap {
            slot,
            pool: rng.below(pool.len()),
        });
    }
    if roll < 0.55 {
        let a = if rng.unit() < 0.5 {
            random_bias_slot(locked, bias, rng).or_else(|| random_slot(locked, rng))?
        } else {
            random_slot(locked, rng)?
        };
        let mut b = random_slot(locked, rng)?;
        let mut tries = 0;
        while (b == a || (board[a].is_none() && board[b].is_none())) && tries < 20 {
            b = random_slot(locked, rng)?;
            tries += 1;
        }
        return (a != b).then_some(Move::BoardSwap { a, b });
    }
    if roll < 0.75 && !pool.is_empty() {
        let slot = random_slot(locked, rng)?;
        return Some(Move::PoolSwap {
            slot,
            pool: rng.below(pool.len()),
        });
    }
    let a = random_slot(locked, rng)?;
    let b = random_slot(locked, rng)?;
    (a != b).then_some(Move::BoardSwap { a, b })
}

/// Pick a board slot in the bottom quartile of its goal contribution.
fn weak_slot(
    board: &Board,
    locked: &[bool; BOARD_SIZE],
    goal: Goal,
    rng: &mut Rng,
) -> Option<usize> {
    let mut slots: Vec<(usize, f64)> = (0..BOARD_SIZE)
        .filter(|slot| !locked[*slot])
        .filter_map(|slot| {
            board[slot].as_ref().map(|cog| {
                let value = match goal {
                    Goal::Build => cog.a + cog.e,
                    Goal::Flaggy => cog.c + cog.g,
                    Goal::ConExp => cog.d + cog.f + cog.b,
                };
                (slot, value)
            })
        })
        .collect();
    if slots.is_empty() {
        return None;
    }
    slots.sort_by(|a, b| a.1.total_cmp(&b.1));
    let cutoff = (slots.len() / 4).max(1);
    Some(slots[rng.below(cutoff)].0)
}

fn random_slot(locked: &[bool; BOARD_SIZE], rng: &mut Rng) -> Option<usize> {
    let mut slot = rng.below(BOARD_SIZE);
    let mut tries = 0;
    while locked[slot] && tries < 50 {
        slot = rng.below(BOARD_SIZE);
        tries += 1;
    }
    (!locked[slot]).then_some(slot)
}

fn random_bias_slot(
    locked: &[bool; BOARD_SIZE],
    bias: &[bool; BOARD_SIZE],
    rng: &mut Rng,
) -> Option<usize> {
    let slots: Vec<usize> = (0..BOARD_SIZE)
        .filter(|slot| bias[*slot] && !locked[*slot])
        .collect();
    (!slots.is_empty()).then(|| slots[rng.below(slots.len())])
}

/// Phase 3: exhaustive pairwise and board↔shelf swaps on the best board.
pub(super) fn greedy_finish(
    board: &mut Board,
    pool: &mut [Option<Cog>],
    locked: &[bool; BOARD_SIZE],
    goal: Goal,
) {
    let mut improved = true;
    let mut passes = 0;
    while improved && passes < 5 {
        improved = false;
        passes += 1;
        for a in 0..BOARD_SIZE {
            if locked[a] {
                continue;
            }
            let mut current = score(board, goal);
            for (b, locked_b) in locked.iter().enumerate().take(BOARD_SIZE).skip(a + 1) {
                if *locked_b {
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
                let mut best = None;
                for index in 0..pool.len() {
                    swap_slot_pool(board, pool, slot, index);
                    let value = score(board, goal);
                    swap_slot_pool(board, pool, slot, index);
                    if value > base && best.is_none_or(|(_, best_value)| value > best_value) {
                        best = Some((index, value));
                    }
                }
                if let Some((index, _)) = best {
                    swap_slot_pool(board, pool, slot, index);
                    improved = true;
                }
            }
        }
    }
}

/// Phase 4: put original cogs back where the score allows, then move wasted
/// directionals to non-directional slots. This keeps the swap list honest: a
/// cog that already sits where it should does not appear.
pub(super) fn swap_back(
    board: &mut Board,
    original: &Board,
    locked: &mut [bool; BOARD_SIZE],
    goal: Goal,
) {
    let mut best_score = score(board, goal);
    let mut changed = true;
    let mut passes = 0;
    while changed && passes < 4 {
        changed = false;
        passes += 1;
        let forward = passes % 2 == 1;
        let slots: Vec<usize> = if forward {
            (0..BOARD_SIZE).collect()
        } else {
            (0..BOARD_SIZE).rev().collect()
        };
        for slot in slots {
            if locked[slot] || board[slot] == original[slot] {
                continue;
            }
            let wanted = original[slot].clone();
            let mut source = None;
            let mut fallback = None;
            for other in 0..BOARD_SIZE {
                if other == slot || locked[other] || board[other] != wanted {
                    continue;
                }
                let useful_here = affects_player(board, other);
                let useful_at_destination =
                    wanted.as_ref().is_some_and(|cog| cog.direction.is_some())
                        && affects_player_at(board, other, slot);
                if useful_here && !useful_at_destination {
                    continue;
                }
                if useful_here {
                    fallback.get_or_insert(other);
                    continue;
                }
                source = Some(other);
                break;
            }
            let Some(source) = source.or(fallback) else {
                continue;
            };
            board.swap(slot, source);
            let value = score(board, goal);
            if value < best_score {
                board.swap(slot, source);
            } else {
                best_score = value;
                changed = true;
            }
        }
    }

    // Cosmetic: a directional cog that reaches no player looks out of place next
    // to a non-directional one, so trade places when the score allows it.
    let mut cosmetic = 0;
    for slot in 0..BOARD_SIZE {
        let Some(cog) = board[slot].clone() else {
            continue;
        };
        if cog.direction.is_none()
            || cog.direction == Some(Direction::Everything)
            || cog.yin
            || cog.is_player
            || affects_player(board, slot)
        {
            continue;
        }
        for other in 0..BOARD_SIZE {
            if other == slot || locked[other] {
                continue;
            }
            let Some(target) = board[other].clone() else {
                continue;
            };
            if target.direction.is_some() || target.yin || target.is_player || target == cog {
                continue;
            }
            board.swap(slot, other);
            let value = score(board, goal);
            if value < best_score {
                board.swap(slot, other);
            } else {
                best_score = value;
                cosmetic += 1;
                break;
            }
        }
    }
    if cosmetic == 0 {
        return;
    }

    // The cosmetic pass can displace cogs that were already home; restore them.
    for pass in 0..2 {
        let slots: Vec<usize> = if pass == 0 {
            (0..BOARD_SIZE).collect()
        } else {
            (0..BOARD_SIZE).rev().collect()
        };
        for slot in slots {
            if locked[slot] || board[slot] == original[slot] {
                continue;
            }
            let wanted = original[slot].clone();
            let source = (0..BOARD_SIZE).find(|other| {
                *other != slot
                    && !locked[*other]
                    && board[*other] == wanted
                    && !affects_player(board, *other)
            });
            let Some(source) = source else {
                continue;
            };
            board.swap(slot, source);
            let value = score(board, goal);
            if value < best_score {
                board.swap(slot, source);
            } else {
                best_score = value;
            }
        }
    }
}

/// Whether a cog moved from `from` to `to` would reach a player there.
fn affects_player_at(board: &Board, from: usize, to: usize) -> bool {
    let Some(cog) = board.get(from).and_then(Option::as_ref) else {
        return false;
    };
    let Some(direction) = cog.direction else {
        return false;
    };
    crate::cog::direction_reaches_player(board, to, direction)
}
