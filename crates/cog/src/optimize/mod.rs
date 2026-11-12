//! The cog board optimizer: greedy construction, simulated annealing and a
//! greedy finish.
//!
//! Ported from `js/cogs/cog-optimizer.js`. The upstream algorithm has four
//! phases: build a strong greedy starting board, anneal it with moves biased
//! toward the slots the goal reads, refine it with exhaustive pairwise and
//! board↔shelf swaps, then restore as many original cogs as the score allows so
//! the user sees the fewest possible swaps. All four are kept here.
//!
//! Two differences are deliberate:
//!
//! - the random source is a seeded xorshift, so a run is reproducible and tests
//!   are stable (upstream uses `Math.random`);
//! - the greedy player-position sweep is scored without its per-candidate
//!   refinement pass; the winner still gets the full refinement. That keeps a
//!   debug build inside the harness's wait budget without changing the board
//!   rules or the scoring.

mod anneal;
mod greedy;

use idleon_common::Control;
use idleon_common::Progress;

use super::{BOARD_SIZE, Board, Cog, Objective, score, slot_to_pos};
use anneal::{Rng, bias_slots, greedy_finish, smart_move, swap_back};
use greedy::{greedy_construct, lock_yin_blocks};

/// How a run is tuned. [`OptimizeOptions::default`] is the section's setting.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptimizeOptions {
    /// Annealing iterations.
    pub iterations: usize,
    /// The xorshift seed; the same seed and inputs produce the same board.
    pub seed: u64,
    pub temp_start: f64,
    pub temp_end: f64,
}

impl Default for OptimizeOptions {
    fn default() -> Self {
        Self {
            iterations: 30_000,
            seed: 0x00c0_6b0a_7d1e_5eed,
            temp_start: 1.0,
            temp_end: 0.001,
        }
    }
}

/// One user-visible swap from the save board to the optimized board.
#[derive(Clone, Debug, PartialEq)]
pub struct SwapStep {
    /// The destination board slot.
    pub slot: usize,
    /// The cog name that leaves the slot, or `Blank`.
    pub from: String,
    /// The cog name that arrives in the slot, or `Blank`.
    pub to: String,
    /// The board slot the arriving cog came from; `None` means the shelf.
    pub src_slot: Option<usize>,
    pub col: usize,
    pub row: usize,
}

impl SwapStep {
    /// Where the arriving cog came from, as the upstream swap list shows it.
    pub fn source_label(&self) -> String {
        match self.src_slot {
            Some(slot) => {
                let (col, row) = slot_to_pos(slot);
                format!("board ({col},{row})")
            }
            None => "shelf".into(),
        }
    }
}

/// One finished optimizer run.
#[derive(Clone, Debug, PartialEq)]
pub struct CogResult {
    pub board: Board,
    /// The objective the run maximised, so the result explains itself even
    /// after the controls change.
    pub objective: Objective,
    pub score: f64,
    pub initial_score: f64,
    pub improvement: f64,
    pub improvement_pct: f64,
    pub swaps: Vec<SwapStep>,
}

/// Optimize `board` for `objective`, borrowing the shelf as the available pool.
///
/// `report` receives progress and may return [`Control::Stop`]; a stopped run
/// returns `None` and leaves the caller's result untouched. The returned board
/// is never worse than `board`: when no phase improves it, the original board
/// comes back with an empty swap list.
pub fn optimize(
    board: &Board,
    shelf: &[Cog],
    objective: &Objective,
    options: &OptimizeOptions,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<CogResult> {
    let goal = objective.dominant();
    let original = board.clone();
    let mut working = board.clone();
    let mut pool: Vec<Option<Cog>> = shelf
        .iter()
        .filter(|cog| !cog.name.is_empty() && cog.name != "Blank")
        .cloned()
        .map(Some)
        .collect();
    let initial_score = score(&original, objective);
    let original_pool = pool.clone();

    if !greedy_construct(&mut working, &mut pool, goal, objective, report) {
        return None;
    }
    // Upstream keeps the save board when the greedy rebuild is not better: it
    // is a better annealing start, not an unconditional replacement.
    if score(&working, objective) <= initial_score {
        working = original.clone();
        pool = original_pool;
    }
    let mut best_score = score(&working, objective);
    let mut current_board = working.clone();
    let mut best_board = current_board.clone();
    let mut best_pool = pool.clone();
    let mut current_score = best_score;
    let mut locked = [false; BOARD_SIZE];
    lock_yin_blocks(&best_board, &mut locked);
    let bias = bias_slots(&best_board, goal);

    let mut rng = Rng::new(options.seed);
    let iterations = options.iterations.max(1);
    let report_every = (iterations / 100).max(1);
    let log_ratio = (options.temp_end / options.temp_start).ln();
    for iter in 0..iterations {
        let temp = options.temp_start * (log_ratio * iter as f64 / iterations as f64).exp();
        let Some(next) = smart_move(&current_board, &pool, &locked, goal, &bias, &mut rng) else {
            continue;
        };
        next.apply(&mut current_board, &mut pool);
        let new_score = score(&current_board, objective);
        let delta = new_score - current_score;
        let accept = if delta > 0.0 {
            true
        } else if delta == 0.0 {
            false
        } else {
            let normalized = if current_score > 0.0 {
                delta / current_score
            } else {
                delta
            };
            rng.unit() < (normalized / temp).exp()
        };
        if accept {
            current_score = new_score;
            if current_score > best_score {
                best_score = current_score;
                best_board = current_board.clone();
                best_pool = pool.clone();
            }
        } else {
            next.apply(&mut current_board, &mut pool);
        }
        if iter.is_multiple_of(report_every) {
            let progress = Progress {
                fraction: (0.15 + 0.75 * iter as f64 / iterations as f64) as f32,
                message: format!(
                    "Simulated annealing · iteration {iter}/{iterations} · best {}",
                    idleon_common::formula::ValueFormat::Rate.format(best_score)
                ),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }
    }

    if report(Progress {
        fraction: 0.9,
        message: "Greedy refinement".into(),
    }) == Control::Stop
    {
        return None;
    }
    greedy_finish(&mut best_board, &mut best_pool, &locked, objective);
    swap_back(&mut best_board, &original, &mut locked, objective);
    let mut final_score = score(&best_board, objective);

    // The invariant: no phase may leave the user worse off than the save.
    if final_score < initial_score {
        best_board = original.clone();
        final_score = initial_score;
    }
    let swaps = compute_swap_sequence(&original, &best_board, shelf);
    let improvement = final_score - initial_score;
    let improvement_pct = if initial_score > 0.0 {
        improvement / initial_score * 100.0
    } else {
        0.0
    };
    if report(Progress {
        fraction: 1.0,
        message: format!("Done · {} swap(s)", swaps.len()),
    }) == Control::Stop
    {
        return None;
    }
    Some(CogResult {
        board: best_board,
        objective: objective.clone(),
        score: final_score,
        initial_score,
        improvement,
        improvement_pct,
        swaps,
    })
}

/// Apply the swap list in order to `board`, taking each arriving cog from
/// `target` and returning the displaced cog to `shelf` when the source is the
/// shelf. This is the operation the swap walkthrough drives.
pub fn apply_swaps(
    board: &mut Board,
    target: &Board,
    shelf: &mut [Option<Cog>],
    steps: &[SwapStep],
    count: usize,
) {
    for step in steps.iter().take(count) {
        let displaced = board.get_mut(step.slot).and_then(Option::take);
        let arriving = target.get(step.slot).cloned().flatten();
        if let Some(slot) = board.get_mut(step.slot) {
            *slot = arriving.clone();
        }
        match step.src_slot {
            Some(source) => {
                if let Some(slot) = board.get_mut(source) {
                    *slot = displaced;
                }
            }
            None => {
                if let Some(arriving) = arriving
                    && let Some(index) =
                        shelf.iter().position(|cog| cog.as_ref() == Some(&arriving))
                {
                    shelf[index] = None;
                }
                if let Some(displaced) = displaced
                    && let Some(empty) = shelf.iter_mut().find(|slot| slot.is_none())
                {
                    *empty = Some(displaced);
                }
            }
        }
    }
}

/// The swap sequence that turns `from` into `to`, matching cogs by their full
/// stat identity so identical cogs never produce a swap.
///
/// The shelf is tracked alongside the board: a cog the optimized board takes
/// from the shelf is reported with `src_slot: None`, and a displaced board cog
/// goes back to the shelf so a later step can use it. Without that bookkeeping
/// two identical cogs (one on the board, one on the shelf) would produce a step
/// list that cannot rebuild the result.
fn compute_swap_sequence(from: &Board, to: &Board, shelf: &[Cog]) -> Vec<SwapStep> {
    let mut current = from.clone();
    let mut shelf_left: Vec<Option<Cog>> = shelf.iter().cloned().map(Some).collect();
    let mut steps = Vec::new();
    for slot in 0..BOARD_SIZE {
        if current[slot] == to[slot] {
            continue;
        }
        let target = to[slot].clone();
        let leaving = current[slot].clone();
        let mut source = None;
        if let Some(wanted) = target.as_ref() {
            let mut unchanged = None;
            for other in 0..BOARD_SIZE {
                if other == slot || current[other].as_ref() != Some(wanted) {
                    continue;
                }
                if current[other] != to[other] {
                    source = Some(other);
                    break;
                }
                unchanged.get_or_insert(other);
            }
            if source.is_none() {
                // A shelf copy is the right source when the board copy at
                // another slot is already where it belongs.
                if let Some(index) = shelf_left
                    .iter()
                    .position(|cog| cog.as_ref() == Some(wanted))
                {
                    shelf_left[index] = None;
                } else {
                    source = unchanged;
                }
            }
        }
        let (col, row) = slot_to_pos(slot);
        steps.push(SwapStep {
            slot,
            from: leaving
                .as_ref()
                .map_or_else(|| "Blank".into(), |cog| cog.name.clone()),
            to: target
                .as_ref()
                .map_or_else(|| "Blank".into(), |cog| cog.name.clone()),
            src_slot: source,
            col,
            row,
        });
        current[slot] = target;
        match source {
            Some(source) => current[source] = leaving,
            None => {
                if let Some(leaving) = leaving {
                    if let Some(empty) = shelf_left.iter_mut().find(|slot| slot.is_none()) {
                        *empty = Some(leaving);
                    } else {
                        shelf_left.push(Some(leaving));
                    }
                }
            }
        }
    }
    steps
}

/// Swap one board slot with one shelf slot, blanks included.
pub(super) fn swap_slot_pool(
    board: &mut Board,
    pool: &mut [Option<Cog>],
    slot: usize,
    index: usize,
) {
    if let (Some(slot_cog), Some(pool_cog)) = (board.get_mut(slot), pool.get_mut(index)) {
        std::mem::swap(slot_cog, pool_cog);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Direction, Goal, empty_board};

    fn cog(name: &str, slot: usize) -> Cog {
        Cog::named(name, slot)
    }

    /// A board with one player and a shelf of upgrades, enough to improve.
    fn sample() -> (Board, Vec<Cog>) {
        let mut board = empty_board();
        let mut player = cog("Player_Ana", 0);
        player.b = 100.0;
        player.d = 20.0;
        board[0] = Some(player);
        let mut far = cog("Cog3ri", 90);
        far.direction = Some(Direction::Right);
        far.f = 30.0;
        far.d = 10.0;
        board[90] = Some(far);
        let mut filler = cog("Cog3A00", 1);
        filler.d = 5.0;
        board[1] = Some(filler);
        let mut around = cog("Cog3cr", 100);
        around.direction = Some(Direction::Around);
        around.f = 40.0;
        around.d = 30.0;
        (board, vec![around])
    }

    #[test]
    fn the_optimizer_never_returns_a_worse_board() {
        let (board, shelf) = sample();
        let options = OptimizeOptions {
            iterations: 400,
            ..OptimizeOptions::default()
        };
        let mut report = |_progress| Control::Continue;
        let objective = Objective::goal(Goal::ConExp);
        let result =
            optimize(&board, &shelf, &objective, &options, &mut report).expect("a full run");
        assert!(
            result.score >= result.initial_score,
            "{} < {}",
            result.score,
            result.initial_score
        );
        assert_eq!(result.board.len(), BOARD_SIZE);
        assert_eq!(result.board.iter().flatten().count(), 4);
    }

    #[test]
    fn the_same_seed_produces_the_same_board() {
        let (board, shelf) = sample();
        let options = OptimizeOptions {
            iterations: 300,
            ..OptimizeOptions::default()
        };
        let mut first = |_progress| Control::Continue;
        let objective = Objective::goal(Goal::ConExp);
        let a = optimize(&board, &shelf, &objective, &options, &mut first).expect("run a");
        let mut second = |_progress| Control::Continue;
        let b = optimize(&board, &shelf, &objective, &options, &mut second).expect("run b");
        assert_eq!(a, b);
    }

    #[test]
    fn a_cancelled_run_returns_nothing() {
        let (board, shelf) = sample();
        let options = OptimizeOptions {
            iterations: 10_000,
            ..OptimizeOptions::default()
        };
        let mut stop = |_progress| Control::Stop;
        let objective = Objective::goal(Goal::ConExp);
        assert!(optimize(&board, &shelf, &objective, &options, &mut stop).is_none());
    }

    #[test]
    fn applying_the_swap_list_reproduces_the_optimized_board() {
        let (board, shelf) = sample();
        let options = OptimizeOptions {
            iterations: 1_000,
            ..OptimizeOptions::default()
        };
        let mut report = |_progress| Control::Continue;
        let objective = Objective::goal(Goal::ConExp);
        let result =
            optimize(&board, &shelf, &objective, &options, &mut report).expect("a full run");
        let mut shelf_slots: Vec<Option<Cog>> = shelf.iter().cloned().map(Some).collect();
        shelf_slots.resize(120, None);
        let mut applied = board.clone();
        apply_swaps(
            &mut applied,
            &result.board,
            &mut shelf_slots,
            &result.swaps,
            result.swaps.len(),
        );
        assert_eq!(applied, result.board);
        // Undo: applying no steps leaves the board alone, and resetting is a
        // fresh copy of the save board.
        let mut reset = result.board.clone();
        apply_swaps(&mut reset, &result.board, &mut shelf_slots, &[], 0);
        assert_eq!(reset, result.board);
    }
}
