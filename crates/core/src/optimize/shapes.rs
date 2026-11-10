//! Shape placement: greedy over target priorities, then local search on real EXP.

use crate::data::game;
use crate::model::{Placement, Progress, Simulation, SimContext};
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

/// Greedy placement, then a local-search pass when the effort allows it.
pub fn optimize(
    sim: &Simulation,
    ctx: &SimContext,
    effort: crate::model::Effort,
    report: &mut dyn FnMut(Progress),
) -> ShapesOutcome {
    let game = game();
    let before = sim::exp_rate(sim, ctx).total;
    let owned = sim::shapes_owned(sim.research_level, ctx);
    let mut outcome = ShapesOutcome {
        before,
        ..ShapesOutcome::default()
    };
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
        outcome
            .steps
            .push("No grid square is worth shaping yet; level a square first.".into());
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

    let rotations = if sim::can_rotate(sim.research_level) {
        shape::ROT_STEPS
    } else {
        1
    };
    let mut covered = vec![false; game.cells()];
    for (index, shape_id) in order.iter().enumerate() {
        report(Progress {
            fraction: 0.1 + 0.6 * index as f32 / order.len() as f32,
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
            outcome.placements.push(placement);
        }
    }

    outcome.overlay = shape::overlay(&outcome.placements);
    let mut trial = sim.clone();
    trial.placements = outcome.placements.clone();
    outcome.rate = sim::exp_rate(&trial, ctx).total;

    if effort != crate::model::Effort::Quick {
        report(Progress {
            fraction: 0.75,
            message: "Refining shape placement…".into(),
        });
        let (placements, rate) = local_search(sim, ctx, outcome.placements.clone(), effort);
        outcome.placements = placements;
        outcome.overlay = shape::overlay(&outcome.placements);
        outcome.rate = rate;
    }

    outcome.steps.push(format!(
        "Placed {} shapes: {} EXP/hr ({:+.1}%).",
        outcome.placements.len(),
        sim::format_rate(outcome.rate),
        if before > 0.0 {
            (outcome.rate / before - 1.0) * 100.0
        } else {
            0.0
        }
    ));
    outcome
}

fn place_shape(
    shape_id: usize,
    anchors: &[usize],
    weights: &Weights,
    covered: &[bool],
    rotations: usize,
    bonus_pct: f64,
) -> Option<Placement> {
    let game = game();
    let cols = game.grid.cols as i32;
    let scale = bonus_pct / 25.0;

    let mut best: Option<(f64, f64, usize, Placement)> = None;
    for step in 0..rotations {
        let rot = step as f64 * shape::ROT_STEP;
        for pattern in shape::lut().patterns(shape_id, rot) {
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
    }
    let _ = cols;
    best.map(|(_, _, _, placement)| placement)
}

/// Hill-climb each shape's anchor while the real EXP rate improves.
fn local_search(
    sim: &Simulation,
    ctx: &SimContext,
    placements: Vec<Placement>,
    effort: crate::model::Effort,
) -> (Vec<Placement>, f64) {
    let game = game();
    if placements.is_empty() {
        return (placements, 0.0);
    }
    let passes = if effort == crate::model::Effort::Thorough {
        3
    } else {
        1
    };
    let offsets: Vec<(f64, f64)> = if effort == crate::model::Effort::Thorough {
        vec![
            (0.0, 0.0),
            (5.0, 0.0),
            (-5.0, 0.0),
            (0.0, 5.0),
            (0.0, -5.0),
            (15.0, 15.0),
            (-15.0, 15.0),
            (15.0, -15.0),
            (-15.0, -15.0),
        ]
    } else {
        vec![(0.0, 0.0), (10.0, 10.0), (-10.0, -10.0), (10.0, -10.0), (-10.0, 10.0)]
    };
    let rotation_deltas: Vec<f64> = if effort == crate::model::Effort::Thorough {
        vec![0.0, 5.0, -5.0, 10.0, -10.0, 15.0, -15.0, 30.0, -30.0, 45.0, -45.0]
    } else {
        vec![0.0, 5.0, -5.0]
    };

    let score = |placements: &[Placement]| {
        let mut trial = sim.clone();
        trial.placements = placements.to_vec();
        sim::exp_rate(&trial, ctx).total
    };

    let mut current = placements;
    let mut current_score = score(&current);
    for _ in 0..passes {
        let mut improved = false;
        for index in 0..current.len() {
            let shape_id = current[index].shape;
            let _ = &game.shapes[shape_id];
            let baseline = current[index];
            let candidates: Vec<Placement> = offsets
                .iter()
                .flat_map(|(dx, dy)| {
                    rotation_deltas.iter().map(move |dro| Placement {
                        x: baseline.x + dx,
                        y: baseline.y + dy,
                        rot: baseline.rot + dro,
                        ..baseline
                    })
                })
                .filter(shape::in_bounds)
                .collect();
            let results: Vec<(f64, Placement)> = candidates
                .into_par_iter()
                .map(|candidate| {
                    let mut trial = current.clone();
                    trial[index] = candidate;
                    (score(&trial), candidate)
                })
                .collect();
            if let Some((best_score, best_placement)) =
                results.into_iter().max_by(|a, b| a.0.total_cmp(&b.0))
                && best_score > current_score * (1.0 + 1e-12) {
                    current[index] = best_placement;
                    current_score = best_score;
                    improved = true;
                }
        }
        if !improved {
            break;
        }
    }
    (current, current_score)
}
