//! Grid point spending: a greedy climb with an optional refine pass.

use crate::data::game;
use crate::model::{Effort, GridState, Simulation, SimContext};
use crate::sim;

/// A grid layout plus what it spends.
#[derive(Clone, Debug, Default)]
pub struct GridOutcome {
    pub grid: GridState,
    pub spent: u32,
    pub steps: Vec<String>,
}

/// Spend the free grid points on the squares that raise EXP the most.
pub fn respec(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    budget: Option<u32>,
) -> GridOutcome {
    let game = game();
    let mut grid = sim.grid.clone();
    let points = sim::grid_points(sim, ctx);
    let mut free = budget.map_or(points.free, |budget| budget.saturating_sub(grid.spent()));
    let mut steps = Vec::new();

    while free > 0 {
        let mut best: Option<(f64, usize)> = None;
        for node in &game.nodes {
            let level = grid.level(node.id);
            if level >= node.max_lv || !unlocked(node.id, &grid) {
                continue;
            }
            let mut trial = sim.clone();
            trial.grid = grid.clone();
            trial.grid.set(node.id, level + 1);
            let rate = sim::exp_rate(&trial, ctx).total;
            let key = (rate, node.id);
            let better = match &best {
                None => true,
                Some((current, id)) => rate > *current || (rate == *current && node.id < *id),
            };
            if better {
                best = Some(key);
            }
        }
        let Some((_, cell)) = best else { break };
        let before = sim::exp_rate(
            &Simulation {
                grid: grid.clone(),
                ..sim.clone()
            },
            ctx,
        )
        .total;
        grid.set(cell, grid.level(cell) + 1);
        free -= 1;
        let after = sim::exp_rate(
            &Simulation {
                grid: grid.clone(),
                ..sim.clone()
            },
            ctx,
        )
        .total;
        if after <= before {
            grid.set(cell, grid.level(cell) - 1);
            break;
        }
        steps.push(format!(
            "Leveled {} to {}.",
            game.node_at(cell).map(|node| node.name.as_str()).unwrap_or("a square"),
            grid.level(cell)
        ));
    }

    if effort == Effort::Thorough && !steps.is_empty() {
        refine(sim, ctx, &mut grid, &mut steps);
    }

    GridOutcome {
        spent: grid.spent(),
        grid,
        steps,
    }
}

/// Move one point at a time when that beats the current layout.
fn refine(sim: &Simulation, ctx: &SimContext, grid: &mut GridState, steps: &mut Vec<String>) {
    let mut improved = true;
    let mut passes = 0;
    while improved && passes < 4 {
        improved = false;
        passes += 1;
        let mut current = sim.clone();
        current.grid = grid.clone();
        let base = sim::exp_rate(&current, ctx).total;
        for node in &game().nodes {
            if grid.level(node.id) == 0 {
                continue;
            }
            for other in &game().nodes {
                if other.id == node.id || grid.level(other.id) >= other.max_lv {
                    continue;
                }
                if !unlocked(other.id, grid) {
                    continue;
                }
                let mut trial = grid.clone();
                trial.set(node.id, trial.level(node.id) - 1);
                trial.set(other.id, trial.level(other.id) + 1);
                let mut candidate = sim.clone();
                candidate.grid = trial.clone();
                let rate = sim::exp_rate(&candidate, ctx).total;
                if rate > base * (1.0 + 1e-12) {
                    *grid = trial;
                    steps.push(format!(
                        "Moved a point from {} to {}.",
                        game().node_at(node.id).map(|n| n.name.as_str()).unwrap_or("a square"),
                        game().node_at(other.id).map(|n| n.name.as_str()).unwrap_or("a square")
                    ));
                    improved = true;
                    break;
                }
            }
            if improved {
                break;
            }
        }
    }
}

/// A square is unlocked when leveled, seeded, or beside a leveled one.
pub fn unlocked(cell: usize, grid: &GridState) -> bool {
    let game = game();
    if game.node_at(cell).is_none() {
        return false;
    }
    if grid.level(cell) >= 1 {
        return true;
    }
    let cols = game.grid.cols;
    let col = cell % cols;
    if (col == 9 || col == 10) && (100..=140).contains(&cell) {
        return true;
    }
    let neighbours = [
        cell.checked_sub(cols),
        Some(cell + cols).filter(|next| *next < game.cells()),
        (col > 0).then_some(cell - 1),
        (col + 1 < cols).then_some(cell + 1),
    ];
    neighbours
        .into_iter()
        .flatten()
        .any(|neighbour| grid.level(neighbour) >= 1)
}
