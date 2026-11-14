//! Magnifier assignment: a greedy pass, then a repair pass that moves and swaps.
//!
//! The greedy pass prices each kind by what it can still reach: glasses by the
//! EXP of the observation under them, kaleidoscopes by the magnifiers next to
//! them, monocles by insight gained per hour against the EXP still missing. It
//! runs twice, glasses first and kaleidoscopes first, and keeps the better whole
//! assignment: whichever kind goes first takes the room, and only the finished
//! score shows which order was worth it. The repair pass then scores whole
//! assignments with [`sim::exp_rate`] and moves or swaps one magnifier at a time
//! while that helps.

use crate::data::game;
use crate::model::{Effort, Mag, MagKind, SimContext, SimOptions, Simulation};
use crate::sim;

/// A magnifier pool plus what the optimizer did to it.
#[derive(Clone, Debug, Default)]
pub struct MagsOutcome {
    pub mags: Vec<Mag>,
    pub rate: f64,
    pub steps: Vec<String>,
}

/// Assign the owned magnifiers to observations.
pub fn optimize(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    options: SimOptions,
) -> MagsOutcome {
    let game = game();
    let owned = sim::magnifiers_owned(sim, ctx) as usize;
    let cap = sim::mag_cap(sim.research_level) as usize;
    let usable: Vec<usize> = (0..game.observations.len())
        .filter(|obs| sim::observation_usable(*obs, sim.research_level, &sim.observations))
        .collect();
    let mut outcome = MagsOutcome::default();
    if usable.is_empty() || owned == 0 || cap == 0 {
        outcome
            .steps
            .push("No observation can hold a magnifier at this level yet.".into());
        return outcome;
    }

    let counts = count_kinds(&sim.mags);
    let kaleidoscopes = counts.kaleidoscopes.min(owned);
    let remaining = owned - kaleidoscopes;
    let monocles = counts.monocles.min(remaining);
    let regulars = remaining - monocles;
    let room = Room::new(&usable, cap);
    let orders = [
        vec![
            (MagKind::Regular, regulars),
            (MagKind::Kaleidoscope, kaleidoscopes),
        ],
        vec![
            (MagKind::Kaleidoscope, kaleidoscopes),
            (MagKind::Regular, regulars),
        ],
    ];
    let mut best: Option<(f64, Vec<Mag>)> = None;
    for plan in &orders {
        let mags = assign(sim, ctx, plan, monocles, &room, options);
        let rate = assignment_rate(sim, ctx, &mags);
        if best.as_ref().is_none_or(|(current, _)| rate > *current) {
            best = Some((rate, mags));
        }
    }
    // Both orders always run, so a plan is always in hand.
    let mut mags = best.map(|(_, mags)| mags).unwrap_or_default();

    let before = sim::exp_rate(sim, ctx).total;
    let repair = repair(sim, ctx, &mut mags, &usable, cap, effort);

    outcome.rate = assignment_rate(sim, ctx, &mags);
    outcome.mags = mags;
    if !outcome.mags.is_empty() {
        outcome.steps.push(format!(
            "Placed {} magnifiers across {} observations ({}), {} EXP/hr to {}.",
            outcome.mags.len(),
            outcome
                .mags
                .iter()
                .filter_map(|mag| mag.slot)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            kind_summary(&outcome.mags),
            sim::format_rate(before),
            sim::format_rate(outcome.rate)
        ));
    }
    if let Some(summary) = repair.summary() {
        outcome.steps.push(summary);
    }
    outcome
}

/// Free slots on each observation, shared by both greedy orders.
struct Room {
    slots: Vec<usize>,
}

impl Room {
    fn new(usable: &[usize], cap: usize) -> Self {
        let mut slots = vec![0; game().observations.len()];
        for obs in usable {
            slots[*obs] = cap;
        }
        Self { slots }
    }

    fn take(&mut self, obs: usize) -> bool {
        match self.slots.get_mut(obs) {
            Some(slots) if *slots > 0 => {
                *slots -= 1;
                true
            }
            _ => false,
        }
    }

    fn has_room(&self, obs: usize) -> bool {
        self.slots.get(obs).copied().unwrap_or(0) > 0
    }
}

/// Greedy placement of one kind order: each kind fills the observations that
/// pay it the most, with the kinds already placed taken into account.
fn assign(
    sim: &Simulation,
    ctx: &SimContext,
    plan: &[(MagKind, usize)],
    monocles: usize,
    room: &Room,
    options: SimOptions,
) -> Vec<Mag> {
    let slots = game().observations.len();
    let overlay = sim.overlay();
    let kal_base = sim::kalei_base(&sim.grid, &overlay, ctx);
    let insight = sim::grid_bonus(&sim.grid, &overlay, 93, ctx);
    let mut room = Room {
        slots: room.slots.clone(),
    };
    let mut placed: Vec<Mag> = Vec::new();
    // The extended lookahead values a monocle by the EXP rate its insight
    // level buys. Monocles never change the rate, so one pass after the
    // glasses and kaleidoscopes are down is enough for the whole assignment.
    let mut gains: Option<Vec<f64>> = None;

    for (kind, count) in plan.iter().chain([(MagKind::Monocle, monocles)].iter()) {
        if *kind == MagKind::Monocle && monocles > 0 && options.extended_insight && gains.is_none()
        {
            gains = Some(insight_gains(sim, ctx, &placed));
        }
        for _ in 0..*count {
            let kal_map = sim::kaleidoscope_map(&placed, slots);
            let mut best: Option<(f64, usize)> = None;
            for obs in 0..slots {
                if !room.has_room(obs) {
                    continue;
                }
                let value = match kind {
                    MagKind::Regular => glass_value(sim, obs, &kal_map, kal_base, insight),
                    // With glasses already down, a kaleidoscope is worth the
                    // glasses beside it. Standing alone, it is worth the glasses
                    // the observations beside it could still take.
                    MagKind::Kaleidoscope => {
                        if placed.iter().any(|mag| mag.kind == MagKind::Regular) {
                            kaleidoscope_weight(sim, obs, &placed, kal_base, insight)
                        } else {
                            prospect_weight(sim, obs, &room, &kal_map, kal_base, insight)
                        }
                    }
                    MagKind::Monocle => {
                        let roi = insight_roi(sim, ctx, &overlay, obs, &kal_map, kal_base);
                        let gain = gains
                            .as_ref()
                            .and_then(|values| values.get(obs).copied())
                            .unwrap_or(0.0);
                        if gain > 0.0 { roi * gain } else { roi }
                    }
                };
                if best.is_none_or(|(current, _)| value > current) {
                    best = Some((value, obs));
                }
            }
            let Some((_, obs)) = best else { break };
            if !room.take(obs) {
                break;
            }
            placed.push(Mag::new(*kind, Some(obs)));
        }
    }
    placed
}

/// EXP/hour each observation's next insight level would permanently add, at
/// the magnifiers placed so far. The extended lookahead ranks a monocle by
/// `gain × insight/hour`, the upstream `chooseMonoTargets` score, instead of
/// the next level-up alone.
fn insight_gains(sim: &Simulation, ctx: &SimContext, mags: &[Mag]) -> Vec<f64> {
    let mut trial = sim.clone();
    trial.mags = mags.to_vec();
    let base = sim::exp_rate(&trial, ctx).total;
    (0..game().observations.len())
        .map(|obs| {
            let level = trial.observations.insight_level[obs];
            trial.observations.insight_level[obs] = level + 1;
            let rate = sim::exp_rate(&trial, ctx).total;
            trial.observations.insight_level[obs] = level;
            (rate - base).max(0.0)
        })
        .collect()
}

/// EXP/hour of one assignment against the state it was built for.
fn assignment_rate(sim: &Simulation, ctx: &SimContext, mags: &[Mag]) -> f64 {
    let mut trial = sim.clone();
    trial.mags = mags.to_vec();
    sim::exp_rate(&trial, ctx).total
}

/// What the repair pass changed.
#[derive(Default)]
struct Repair {
    moves: u32,
    swaps: u32,
    insight_moves: u32,
}

impl Repair {
    fn summary(&self) -> Option<String> {
        if self.moves + self.swaps + self.insight_moves == 0 {
            return None;
        }
        let mut parts = Vec::new();
        if self.moves > 0 {
            parts.push(format!("{} moves", self.moves));
        }
        if self.swaps > 0 {
            parts.push(format!("{} swaps", self.swaps));
        }
        if self.insight_moves > 0 {
            parts.push(format!("{} monocle moves", self.insight_moves));
        }
        Some(format!(
            "Repaired the assignment with {}.",
            parts.join(", ")
        ))
    }
}

/// Move or swap a magnifier whenever the whole assignment scores better.
fn repair(
    sim: &Simulation,
    ctx: &SimContext,
    mags: &mut [Mag],
    usable: &[usize],
    cap: usize,
    effort: Effort,
) -> Repair {
    let (passes, limit) = match effort {
        Effort::Quick => (0, 0),
        Effort::Balanced => (2, 12_000),
        Effort::Thorough => (4, 40_000),
    };
    let mut repair = Repair::default();
    if passes == 0 {
        return repair;
    }
    let mut evaluator = Evaluator::new(sim, ctx, limit);
    for _ in 0..passes {
        let moved = evaluator.move_or_swap(mags, usable, cap, &mut repair);
        let insight = evaluator.move_monocle(mags, usable, cap, &mut repair);
        if !moved && !insight {
            break;
        }
        if evaluator.exhausted() {
            break;
        }
    }
    repair
}

/// Whole-assignment scoring with the evaluation budget the repair may spend.
struct Evaluator<'a> {
    ctx: &'a SimContext,
    scratch: Simulation,
    /// Shape coverage never changes here, so the overlay is built once.
    overlay: Vec<i32>,
    evaluations: usize,
    limit: usize,
}

impl<'a> Evaluator<'a> {
    fn new(sim: &Simulation, ctx: &'a SimContext, limit: usize) -> Self {
        Self {
            ctx,
            scratch: sim.clone(),
            overlay: sim.overlay(),
            evaluations: 0,
            limit,
        }
    }

    fn rate(&mut self, mags: &[Mag]) -> f64 {
        self.scratch.mags = mags.to_vec();
        self.evaluations += 1;
        sim::exp_rate(&self.scratch, self.ctx).total
    }

    /// Insight per hour against the EXP still missing, over the whole board.
    fn insight_roi(&mut self, mags: &[Mag]) -> f64 {
        self.scratch.mags = mags.to_vec();
        self.evaluations += 1;
        let slots = game().observations.len();
        let kal_map = sim::kaleidoscope_map(&self.scratch.mags, slots);
        let kal_base = sim::kalei_base(&self.scratch.grid, &self.overlay, self.ctx);
        (0..slots)
            .map(|obs| {
                insight_roi(
                    &self.scratch,
                    self.ctx,
                    &self.overlay,
                    obs,
                    &kal_map,
                    kal_base,
                )
            })
            .sum()
    }

    fn exhausted(&self) -> bool {
        self.evaluations >= self.limit
    }

    /// The best single move or swap of a non-monocle magnifier, applied in place.
    fn move_or_swap(
        &mut self,
        mags: &mut [Mag],
        usable: &[usize],
        cap: usize,
        repair: &mut Repair,
    ) -> bool {
        let base = self.rate(mags);
        let mut best_move: Option<(f64, usize, usize)> = None;
        let mut best_swap: Option<(f64, usize, usize)> = None;

        for index in 0..mags.len() {
            if mags[index].kind == MagKind::Monocle {
                continue;
            }
            for slot in usable {
                if mags[index].slot == Some(*slot) {
                    continue;
                }
                let mut trial = mags.to_vec();
                if !place(&mut trial, index, Some(*slot), cap) {
                    continue;
                }
                let rate = self.rate(&trial);
                if best_move.is_none_or(|(current, ..)| rate > current) {
                    best_move = Some((rate, index, *slot));
                }
                if self.exhausted() {
                    return false;
                }
            }
        }

        for left in 0..mags.len() {
            for right in left + 1..mags.len() {
                let mut trial = mags.to_vec();
                let (a, b) = (trial[left].slot, trial[right].slot);
                trial[left].slot = b;
                trial[right].slot = a;
                if !legal(&trial, usable, cap) {
                    continue;
                }
                let rate = self.rate(&trial);
                if best_swap.is_none_or(|(current, ..)| rate > current) {
                    best_swap = Some((rate, left, right));
                }
                if self.exhausted() {
                    return false;
                }
            }
        }

        let move_gain = best_move
            .map(|(rate, ..)| rate)
            .unwrap_or(f64::NEG_INFINITY);
        let swap_gain = best_swap
            .map(|(rate, ..)| rate)
            .unwrap_or(f64::NEG_INFINITY);
        let threshold = base * (1.0 + 1e-12);
        if move_gain <= threshold && swap_gain <= threshold {
            return false;
        }
        if move_gain >= swap_gain {
            let Some((_, index, slot)) = best_move else {
                return false;
            };
            place(mags, index, Some(slot), cap);
            repair.moves += 1;
        } else {
            let Some((_, left, right)) = best_swap else {
                return false;
            };
            let (a, b) = (mags[left].slot, mags[right].slot);
            mags[left].slot = b;
            mags[right].slot = a;
            repair.swaps += 1;
        }
        true
    }

    /// Kaleidoscopes and glasses are fixed here: monocles never touch the EXP
    /// rate, so only the insight objective has to be ranked.
    fn move_monocle(
        &mut self,
        mags: &mut [Mag],
        usable: &[usize],
        cap: usize,
        repair: &mut Repair,
    ) -> bool {
        let base = self.insight_roi(mags);
        let mut best: Option<(f64, usize, usize)> = None;
        for index in 0..mags.len() {
            if mags[index].kind != MagKind::Monocle {
                continue;
            }
            for slot in usable {
                if mags[index].slot == Some(*slot) {
                    continue;
                }
                let mut trial = mags.to_vec();
                if !place(&mut trial, index, Some(*slot), cap) {
                    continue;
                }
                let roi = self.insight_roi(&trial);
                if best.is_none_or(|(current, ..)| roi > current) {
                    best = Some((roi, index, *slot));
                }
                if self.exhausted() {
                    return false;
                }
            }
        }
        let Some((roi, index, slot)) = best else {
            return false;
        };
        if roi <= base * (1.0 + 1e-12) {
            return false;
        }
        place(mags, index, Some(slot), cap);
        repair.insight_moves += 1;
        true
    }
}

/// Point one magnifier at a slot, refusing when the observation is full.
fn place(mags: &mut [Mag], index: usize, slot: Option<usize>, cap: usize) -> bool {
    let held = mags
        .iter()
        .enumerate()
        .filter(|(other, mag)| *other != index && mag.slot == slot)
        .count();
    if slot.is_some() && held >= cap {
        return false;
    }
    mags[index].slot = slot;
    true
}

/// Every observation within its cap and one that can hold a magnifier.
fn legal(mags: &[Mag], usable: &[usize], cap: usize) -> bool {
    mags.iter().all(|mag| match mag.slot {
        None => true,
        Some(slot) => {
            usable.contains(&slot)
                && mags.iter().filter(|other| other.slot == Some(slot)).count() <= cap
        }
    })
}

fn kind_summary(mags: &[Mag]) -> String {
    let counts = count_kinds(mags);
    let mut parts = Vec::new();
    if counts.kaleidoscopes > 0 {
        parts.push(plural(
            counts.kaleidoscopes,
            "kaleidoscope",
            "kaleidoscopes",
        ));
    }
    if counts.monocles > 0 {
        parts.push(plural(counts.monocles, "monocle", "monocles"));
    }
    if counts.regular > 0 {
        parts.push(plural(counts.regular, "glass", "glasses"));
    }
    if parts.is_empty() {
        "none".into()
    } else {
        parts.join(", ")
    }
}

fn plural(count: usize, one: &str, many: &str) -> String {
    if count == 1 {
        format!("1 {one}")
    } else {
        format!("{count} {many}")
    }
}

fn count_kinds(mags: &[Mag]) -> KindCounts {
    let mut counts = KindCounts::default();
    for mag in mags {
        match mag.kind {
            MagKind::Regular => counts.regular += 1,
            MagKind::Monocle => counts.monocles += 1,
            MagKind::Kaleidoscope => counts.kaleidoscopes += 1,
        }
    }
    counts
}

#[derive(Default)]
struct KindCounts {
    regular: usize,
    monocles: usize,
    kaleidoscopes: usize,
}

/// EXP a glass on `obs` adds, with the kaleidoscopes beside it already counted.
fn glass_value(sim: &Simulation, obs: usize, kal_map: &[u32], kal_base: f64, insight: f64) -> f64 {
    let level = sim.observations.insight(obs) as f64;
    sim::obs_base_exp(obs)
        * (1.0 + insight * level / 100.0)
        * (1.0 + kal_map.get(obs).copied().unwrap_or(0) as f64 * kal_base)
}

/// EXP the glasses beside a kaleidoscope slot could still be worth, for the
/// pass that places kaleidoscopes before any glass is down.
fn prospect_weight(
    sim: &Simulation,
    obs: usize,
    room: &Room,
    kal_map: &[u32],
    kal_base: f64,
    insight: f64,
) -> f64 {
    neighbour_slots(obs)
        .into_iter()
        .filter(|slot| room.has_room(*slot))
        .map(|slot| glass_value(sim, slot, kal_map, kal_base, insight))
        .sum()
}

/// EXP the glasses next to a kaleidoscope slot would multiply: only the
/// observations that actually hold one count.
fn kaleidoscope_weight(
    sim: &Simulation,
    obs: usize,
    glasses: &[Mag],
    kal_base: f64,
    insight: f64,
) -> f64 {
    neighbour_slots(obs)
        .into_iter()
        .map(|slot| {
            let held = glasses.iter().filter(|mag| mag.slot == Some(slot)).count() as f64;
            held * glass_value(sim, slot, &[], kal_base, insight)
        })
        .sum()
}

fn neighbour_slots(obs: usize) -> Vec<usize> {
    let mut slots = Vec::with_capacity(4);
    let col = obs % 8;
    if col != 7 {
        slots.push(obs + 1);
    }
    if col != 0 {
        slots.push(obs.saturating_sub(1));
    }
    if obs > 7 {
        slots.push(obs - 8);
    }
    if obs < 72 {
        slots.push(obs + 8);
    }
    slots
}

/// Insight gain per hour against the EXP still missing for the next level.
fn insight_roi(
    sim: &Simulation,
    ctx: &SimContext,
    overlay: &[i32],
    obs: usize,
    kal_map: &[u32],
    kal_base: f64,
) -> f64 {
    let level = sim.observations.insight_level[obs];
    let missing =
        (sim::insight_exp_required(obs, level) - sim.observations.insight_exp[obs]).max(1.0);
    let bonus =
        sim::grid_bonus(&sim.grid, overlay, 91, ctx) + sim::grid_bonus(&sim.grid, overlay, 92, ctx);
    let kaleido = 1.0 + kal_map.get(obs).copied().unwrap_or(0) as f64 * kal_base;
    3.0 * (1.0 + bonus / 100.0) * kaleido / missing
}
