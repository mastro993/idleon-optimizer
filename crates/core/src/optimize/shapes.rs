//! Shape placement: greedy over target priorities, then a wider local search.
//!
//! The greedy pass scores cells by the preset's priorities and the EXP a shape
//! would add there. The local search then works on the real EXP rate alone: it
//! walks each shape's anchor over the 5 px placement lattice, tries rotation
//! steps once research level 90 unlocks them, and swaps which shape sits in
//! which slot when that scores better.

use crate::data::game;
use crate::model::{Effort, Placement, Progress, Simulation, SimContext};
use crate::shape;
use crate::sim;
use rayon::prelude::*;

/// A shape layout plus the score it earns.
#[derive(Clone, Debug, Default)]
pub struct ShapesOutcome {
    pub placements: Vec<Placement>,
    pub overlay: Vec<i32>,
    pub rate: f64,
    pub before: f64,
    pub steps: Vec<String>,
}

/// Per-cell priorities: the preset's targets first, EXP value as the tiebreak.
struct Weights {
    above: Vec<f64>,
    value: Vec<f64>,
}

impl Weights {
    fn build(sim: &Simulation, ctx: &SimContext) -> Self {
        let game = game();
        let mut above = vec![0.0; game.cells()];
        let targets: Vec<usize> = sim
            .preset()
            .targets
            .iter()
            .copied()
            .filter(|cell| sim.grid.level(*cell) > 0)
            .collect();
        let count = targets.len();
        for (rank, cell) in targets.iter().enumerate() {
            // A higher-ranked target outweighs every lower one together.
            above[*cell] = 2_f64.powi((count - 1 - rank) as i32);
        }
        Self {
            above,
            value: sim::cell_values(sim, ctx),
        }
    }

    fn anchors(&self) -> Vec<usize> {
        let mut anchors: Vec<usize> = (0..self.above.len())
            .filter(|cell| self.above[*cell] > 0.0 || self.value[*cell] > 0.0)
            .collect();
        anchors.sort_by(|a, b| {
            let left = self.above[*a] * 1e6 + self.value[*a];
            let right = self.above[*b] * 1e6 + self.value[*b];
            right.total_cmp(&left)
        });
        anchors
    }
}

/// Greedy placement, then the local search the effort allows.
pub fn optimize(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    report: &mut dyn FnMut(Progress),
) -> ShapesOutcome {
    let game = game();
    let before = sim::exp_rate(sim, ctx).total;
    // Whatever the search does, it may not do worse than the layout it started
    // from: that layout is the incumbent every candidate is measured against.
    let mut outcome = ShapesOutcome {
        placements: sim.placements.clone(),
        rate: before,
        before,
        ..ShapesOutcome::default()
    };
    let owned = sim::shapes_owned(sim.research_level, ctx);
    if owned == 0 {
        outcome.steps.push("No shapes unlocked at this level yet.".into());
        return outcome;
    }

    report(Progress {
        fraction: 0.05,
        message: "Scoring grid squares…".into(),
    });
    let weights = Weights::build(sim, ctx);
    let anchors = weights.anchors();
    if anchors.is_empty() {
        outcome.steps.push(
            "Every square a shape still helps is covered; kept the current layout.".into(),
        );
        return outcome;
    }

    // Largest bonus first: the first rank gets the shape with the most gain.
    let mut order: Vec<usize> = (0..owned).collect();
    order.sort_by(|a, b| {
        game.shapes[*b]
            .bonus_pct
            .total_cmp(&game.shapes[*a].bonus_pct)
            .then_with(|| {
                let area = |shape: &crate::data::ShapeDef| shape.dims[0] * shape.dims[1];
                area(&game.shapes[*b]).total_cmp(&area(&game.shapes[*a]))
            })
    });

    // Rotation is a level-90 unlock in game, so below it only the upright
    // polygon is legal.
    let rotations = if sim::can_rotate(sim.research_level) {
        shape::ROT_STEPS
    } else {
        1
    };
    let mut covered = vec![false; game.cells()];
    let mut greedy = Vec::new();
    for (index, shape_id) in order.iter().enumerate() {
        report(Progress {
            fraction: 0.1 + 0.5 * index as f32 / order.len() as f32,
            message: format!("Placing {}.", game.shapes[*shape_id].name),
        });
        if let Some(placement) = place_shape(
            *shape_id,
            &anchors,
            &weights,
            &covered,
            rotations,
            game.shapes[*shape_id].bonus_pct,
        ) {
            for cell in shape::cells_at(&placement) {
                covered[cell] = true;
            }
            greedy.push(placement);
        }
    }

    let greedy_rate = score(sim, ctx, &greedy);
    if greedy_rate > outcome.rate {
        outcome.rate = greedy_rate;
        outcome.placements = greedy;
    }

    let mut changes = Changes::default();
    if effort != Effort::Quick {
        let (placements, rate, found) =
            local_search(sim, ctx, outcome.placements.clone(), effort, report);
        outcome.placements = placements;
        outcome.rate = rate;
        changes = found;
    }
    outcome.overlay = shape::overlay(&outcome.placements);

    let mut summary = format!(
        "Placed {} shapes: {} EXP/hr to {}.",
        outcome.placements.len(),
        sim::format_rate(before),
        sim::format_rate(outcome.rate)
    );
    if let Some(detail) = changes.summary() {
        summary.push(' ');
        summary.push_str(&detail);
    }
    outcome.steps.push(summary);
    outcome
}

/// What the local search changed.
#[derive(Default)]
struct Changes {
    slides: u32,
    rotations: u32,
    swaps: u32,
}

impl Changes {
    fn summary(&self) -> Option<String> {
        let mut parts = Vec::new();
        if self.slides > 0 {
            parts.push(format!("{} anchor moves", self.slides));
        }
        if self.rotations > 0 {
            parts.push(format!("{} rotations", self.rotations));
        }
        if self.swaps > 0 {
            parts.push(format!("{} swaps", self.swaps));
        }
        if parts.is_empty() {
            None
        } else {
            Some(format!("Search: {}.", parts.join(", ")))
        }
    }
}

fn score(sim: &Simulation, ctx: &SimContext, placements: &[Placement]) -> f64 {
    let mut trial = sim.clone();
    trial.placements = placements.to_vec();
    sim::exp_rate(&trial, ctx).total
}

fn place_shape(
    shape_id: usize,
    anchors: &[usize],
    weights: &Weights,
    covered: &[bool],
    rotations: usize,
    bonus_pct: f64,
) -> Option<Placement> {
    // Rotations are independent, so they are swept in parallel and folded back
    // in rotation order: the same placement wins as in a sequential sweep.
    let scored: Vec<Option<(f64, f64, usize, Placement)>> = (0..rotations)
        .into_par_iter()
        .map(|step| best_for_rotation(shape_id, anchors, weights, covered, step, bonus_pct))
        .collect();
    let mut best: Option<(f64, f64, usize, Placement)> = None;
    for candidate in scored.into_iter().flatten() {
        let better = match &best {
            None => true,
            Some(current) => {
                candidate.0 > current.0
                    || (candidate.0 == current.0 && candidate.1 > current.1)
                    || (candidate.0 == current.0
                        && candidate.1 == current.1
                        && candidate.2 > current.2)
            }
        };
        if better {
            best = Some(candidate);
        }
    }
    best.map(|(_, _, _, placement)| placement)
}

/// The best anchor for one rotation of a shape: priorities first, then EXP, then
/// the number of uncovered cells it reaches.
fn best_for_rotation(
    shape_id: usize,
    anchors: &[usize],
    weights: &Weights,
    covered: &[bool],
    step: usize,
    bonus_pct: f64,
) -> Option<(f64, f64, usize, Placement)> {
    let game = game();
    let rot = step as f64 * shape::ROT_STEP;
    let scale = bonus_pct / 25.0;
    let mut best: Option<(f64, f64, usize, Placement)> = None;
    for pattern in shape::lut().patterns(shape_id, rot) {
        // One pattern has one coverage shape, so a base column and row seen
        // twice within it covers the same cells; across patterns it does not.
        let mut tried: Vec<i32> = Vec::new();
        for anchor in anchors.iter().take(48) {
            if covered[*anchor] {
                continue;
            }
            let anchor_col = (*anchor % game.grid.cols) as i32;
            let anchor_row = (*anchor / game.grid.cols) as i32;
            for (dcol, drow) in &pattern.offsets {
                let base_col = anchor_col - dcol;
                let base_row = anchor_row - drow;
                let key = (base_col + 32) * 128 + (base_row + 32);
                if tried.contains(&key) {
                    continue;
                }
                tried.push(key);
                let cells = shape::cells_for(pattern, base_col, base_row);
                let fresh: Vec<usize> =
                    cells.into_iter().filter(|cell| !covered[*cell]).collect();
                if fresh.is_empty() {
                    continue;
                }
                let above: f64 = fresh.iter().map(|cell| weights.above[*cell]).sum();
                let value: f64 = fresh
                    .iter()
                    .map(|cell| weights.value[*cell] * scale)
                    .sum();
                let placement = Placement {
                    shape: shape_id,
                    x: (base_col * shape::CELL as i32) as f64
                        + (pattern.phases[0].0 * shape::PHASE_STEP) as f64,
                    y: (base_row * shape::CELL as i32) as f64
                        + (pattern.phases[0].1 * shape::PHASE_STEP) as f64,
                    rot,
                };
                let candidate = (above, value, fresh.len(), placement);
                let better = match &best {
                    None => true,
                    Some(current) => {
                        candidate.0 > current.0
                            || (candidate.0 == current.0 && candidate.1 > current.1)
                            || (candidate.0 == current.0
                                && candidate.1 == current.1
                                && candidate.2 > current.2)
                    }
                };
                if better {
                    best = Some(candidate);
                }
            }
        }
    }
    best
}

/// The lattice steps each effort walks, how many passes it makes and how many
/// whole-layout scorings it may spend.
fn profile(effort: Effort) -> (&'static [f64], usize, usize) {
    match effort {
        Effort::Quick => (&[], 0, 0),
        Effort::Balanced => (&[10.0, 5.0], 2, 20_000),
        Effort::Thorough => (&[15.0, 10.0, 5.0], 3, 60_000),
    }
}

/// Rotation offsets each effort tries, in degrees around the current angle.
fn rotation_offsets(effort: Effort) -> &'static [f64] {
    match effort {
        Effort::Quick => &[],
        Effort::Balanced => &[5.0, -5.0, 15.0, -15.0],
        Effort::Thorough => &[
            5.0, -5.0, 10.0, -10.0, 15.0, -15.0, 30.0, -30.0, 45.0, -45.0, 90.0, -90.0,
        ],
    }
}

/// Walk anchors over the pixel lattice, try rotation steps, then swap shapes.
fn local_search(
    sim: &Simulation,
    ctx: &SimContext,
    placements: Vec<Placement>,
    effort: Effort,
    report: &mut dyn FnMut(Progress),
) -> (Vec<Placement>, f64, Changes) {
    let (lattice, passes, limit) = profile(effort);
    let mut search = Search {
        sim,
        ctx,
        evaluations: 0,
        limit,
    };
    let mut current = placements;
    let mut current_score = search.score(&current);
    let mut changes = Changes::default();
    if current.is_empty() || lattice.is_empty() {
        return (current, current_score, changes);
    }

    for pass in 0..passes {
        report(Progress {
            fraction: 0.6 + 0.35 * pass as f32 / passes as f32,
            message: format!("Refining placements (pass {} of {passes})…", pass + 1),
        });
        let mut changed = false;

        // Anchors, fine step last: a shape slides onto the lattice that scores
        // best beside where it already sits.
        for index in 0..current.len() {
            for step in lattice.iter().copied() {
                for _ in 0..2 {
                    let baseline = current[index];
                    let neighbours = [
                        (-step, 0.0),
                        (step, 0.0),
                        (0.0, -step),
                        (0.0, step),
                        (-step, -step),
                        (step, -step),
                        (-step, step),
                        (step, step),
                    ];
                    let mut best: Option<(f64, Placement)> = None;
                    for (dx, dy) in neighbours {
                        let candidate = Placement {
                            x: baseline.x + dx,
                            y: baseline.y + dy,
                            ..baseline
                        };
                        if !shape::in_bounds(&candidate) {
                            continue;
                        }
                        current[index] = candidate;
                        let value = search.score(&current);
                        current[index] = baseline;
                        if value > current_score * (1.0 + 1e-12)
                            && best.is_none_or(|(best_rate, _)| value > best_rate)
                        {
                            best = Some((value, candidate));
                        }
                        if search.exhausted() {
                            break;
                        }
                    }
                    let Some((value, placement)) = best else { break };
                    current[index] = placement;
                    current_score = value;
                    changes.slides += 1;
                    changed = true;
                }
            }
        }

        // Rotation steps, only where the level unlocks them.
        if sim::can_rotate(sim.research_level) {
            let mut batch: Vec<(usize, Placement)> = Vec::new();
            for (index, placement) in current.iter().enumerate() {
                for delta in rotation_offsets(effort) {
                    let candidate = Placement {
                        rot: placement.rot + delta,
                        ..*placement
                    };
                    if shape::in_bounds(&candidate) {
                        batch.push((index, candidate));
                    }
                }
            }
            let scored: Vec<(f64, usize, Placement)> = batch
                .into_par_iter()
                .map(|(index, candidate)| {
                    let mut trial = current.clone();
                    trial[index] = candidate;
                    (score(sim, ctx, &trial), index, candidate)
                })
                .collect();
            search.evaluations += scored.len();
            let mut best: Option<(f64, usize, Placement)> = None;
            for (value, index, candidate) in scored {
                if value > current_score * (1.0 + 1e-12)
                    && best.is_none_or(|(best_rate, ..)| value > best_rate)
                {
                    best = Some((value, index, candidate));
                }
            }
            if let Some((value, index, candidate)) = best {
                current[index] = candidate;
                current_score = value;
                changes.rotations += 1;
                changed = true;
            }
        }

        // Which shape sits in which slot: the polygons have different bonuses,
        // so trading two of them can beat sliding either one.
        let mut traded: Option<(f64, usize, usize)> = None;
        for left in 0..current.len() {
            for right in left + 1..current.len() {
                if current[left].shape == current[right].shape {
                    continue;
                }
                let (left_shape, right_shape) = (current[left].shape, current[right].shape);
                current[left].shape = right_shape;
                current[right].shape = left_shape;
                let value = search.score(&current);
                current[left].shape = left_shape;
                current[right].shape = right_shape;
                if value > current_score * (1.0 + 1e-12)
                    && traded.is_none_or(|(best_rate, ..)| value > best_rate)
                {
                    traded = Some((value, left, right));
                }
            }
        }
        if let Some((value, left, right)) = traded {
            let (left_shape, right_shape) = (current[left].shape, current[right].shape);
            current[left].shape = right_shape;
            current[right].shape = left_shape;
            current_score = value;
            changes.swaps += 1;
            changed = true;
        }

        if !changed || search.exhausted() {
            break;
        }
    }
    (current, current_score, changes)
}

/// Whole-layout scoring with the evaluation budget the search may spend.
struct Search<'a> {
    sim: &'a Simulation,
    ctx: &'a SimContext,
    evaluations: usize,
    limit: usize,
}

impl Search<'_> {
    fn score(&mut self, placements: &[Placement]) -> f64 {
        self.evaluations += 1;
        score(self.sim, self.ctx, placements)
    }

    fn exhausted(&self) -> bool {
        self.evaluations >= self.limit
    }
}
