//! Grid point spending: a marginal climb for Quick runs, a beam search above it.
//!
//! Every candidate is a whole layout scored with [`sim::exp_rate`], so a point is
//! priced by what it does to the finished build rather than by the raw bonus of
//! the square it lands on. A point in a square that no shape covers, or one that
//! feeds a multiplier already covered, is worth less than its bonus suggests.

use crate::data::game;
use crate::model::{Effort, GridState, Mag, MagKind, Progress, SimContext, Simulation};
use crate::sim;

/// A grid layout plus what it spends.
#[derive(Clone, Debug, Default)]
pub struct GridOutcome {
    pub grid: GridState,
    pub spent: u32,
    pub steps: Vec<String>,
}

/// Most magnifiers the simulation will ever own, the pool cache's upper bound.
const MAX_OWNED: usize = 80;

/// Beam width, points looked ahead, and evaluations allowed, per effort.
///
/// `Quick` keeps the instant marginal climb and never beams; the evaluation cap
/// is what keeps a Thorough run on a 200-point budget near a second in a debug
/// build instead of growing with the level.
fn search_profile(effort: Effort) -> (usize, usize, usize) {
    match effort {
        Effort::Quick => (1, 1, 0),
        Effort::Balanced => (3, 2, 60_000),
        Effort::Thorough => (8, 3, 150_000),
    }
}

/// Spend the free grid points on the squares that raise EXP the most.
pub fn respec(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    budget: Option<u32>,
    report: &mut dyn FnMut(Progress),
) -> GridOutcome {
    let (width, depth, limit) = search_profile(effort);
    let points = sim::grid_points(sim, ctx);
    let free = budget.map_or(points.free, |budget| {
        budget.saturating_sub(sim.grid.spent())
    });
    if free == 0 {
        return GridOutcome {
            grid: sim.grid.clone(),
            spent: sim.grid.spent(),
            steps: vec!["No free grid points to spend.".into()],
        };
    }

    let before = sim::exp_rate(sim, ctx).total;
    let start = sim.grid.clone();
    let mut search = Spender::new(sim, ctx, limit);

    // The marginal climb is the incumbent every wider search has to beat.
    report(Progress {
        fraction: 0.05,
        message: format!("Climbing the {free} free points one at a time…"),
    });
    let mut ledger = Ledger::new();
    let mut best = search.climb(&start, free, &mut ledger);

    if limit > 0 {
        report(Progress {
            fraction: 0.4,
            message: format!("Planning {depth} points ahead over a beam of {width}…"),
        });
        let mut beamed_ledger = Ledger::new();
        let beamed = search.beam(&start, free, width, depth, &mut beamed_ledger);
        if search.rate(&beamed) > search.rate(&best) {
            best = beamed;
            ledger = beamed_ledger;
        }

        let passes = if effort == Effort::Thorough { 3 } else { 1 };
        for pass in 0..passes {
            report(Progress {
                fraction: 0.75 + 0.2 * pass as f32 / passes as f32,
                message: "Moving points between squares…".into(),
            });
            if search.exhausted() || !search.refine(&mut best, &mut ledger) {
                break;
            }
        }
    }

    let after = search.rate_on_entry_pool(&best);
    let granted = search.grants(&best);
    let spent = best.spent().saturating_sub(start.spent());
    let mut steps = Vec::new();
    if let Some(summary) = ledger.summary(spent, before, after) {
        steps.push(summary);
    }
    if granted > 0 {
        steps.push(format!(
            "The levels also grant {granted} magnifiers: {} EXP/hr once the pool is fitted.",
            sim::format_rate(search.rate(&best))
        ));
    }
    if ledger.moves > 0 {
        steps.push(format!(
            "Moved {} points onto squares the climb had left too rich.",
            ledger.moves
        ));
    }
    let unspent = free.saturating_sub(spent);
    if unspent > 0 {
        steps.push(format!(
            "Left {unspent} points unspent: no square raised the rate any further."
        ));
    }

    GridOutcome {
        spent: best.spent(),
        grid: best,
        steps,
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

/// Squares that may take another point: unlocked, on a node, under their cap.
fn legal_cells(grid: &GridState) -> Vec<usize> {
    game()
        .nodes
        .iter()
        .filter(|node| grid.level(node.id) < node.max_lv && unlocked(node.id, grid))
        .map(|node| node.id)
        .collect()
}

/// Whole-layout scoring plus the evaluation budget the search may spend.
struct Spender<'a> {
    ctx: &'a SimContext,
    scratch: Simulation,
    evaluations: usize,
    limit: usize,
    /// Magnifier pool the run came in with, before any grant is counted.
    entry_pool: Vec<Mag>,
    /// Observations that can hold a magnifier at this research level.
    usable: Vec<usize>,
    cap: usize,
    owned: usize,
    /// Magnifier pools by owned count, filled once and reused: the greedy fill
    /// is identical for a given count, and it runs on most scorings.
    pools: Vec<Option<Vec<Mag>>>,
}

impl<'a> Spender<'a> {
    fn new(sim: &Simulation, ctx: &'a SimContext, limit: usize) -> Self {
        let usable: Vec<usize> = (0..game().observations.len())
            .filter(|obs| sim::observation_usable(*obs, sim.research_level, &sim.observations))
            .collect();
        Self {
            ctx,
            scratch: sim.clone(),
            evaluations: 0,
            limit,
            entry_pool: sim.mags.clone(),
            usable,
            cap: sim::mag_cap(sim.research_level) as usize,
            owned: sim::magnifiers_owned(sim, ctx) as usize,
            pools: vec![None; MAX_OWNED + 1],
        }
    }

    /// The magnifier pool the grid in `scratch` owns: the one it came in with,
    /// topped up with glasses for the magnifiers its square levels grant. Two
    /// squares grant a magnifier each, so a layout that levels them owns more
    /// glasses than the run came in with, which is what the run re-fits after.
    fn pool_for_layout(&mut self) -> Vec<Mag> {
        let owned = (sim::magnifiers_owned(&self.scratch, self.ctx) as usize).min(MAX_OWNED);
        if let Some(pool) = &self.pools[owned] {
            return pool.clone();
        }
        let mut pool = self.entry_pool.clone();
        if owned > self.owned {
            fill_pool(&mut pool, &self.usable, self.cap, owned);
        }
        self.pools[owned] = Some(pool.clone());
        pool
    }

    /// EXP/hour of a whole layout: the one metric every search here ranks on.
    fn rate(&mut self, grid: &GridState) -> f64 {
        self.scratch.grid = grid.clone();
        self.scratch.mags = self.pool_for_layout();
        self.evaluations += 1;
        sim::exp_rate(&self.scratch, self.ctx).total
    }

    /// EXP/hour of a layout with the magnifiers the run already owns, before the
    /// pool the spending unlocks is fitted.
    fn rate_on_entry_pool(&mut self, grid: &GridState) -> f64 {
        self.scratch.grid = grid.clone();
        self.scratch.mags = self.entry_pool.clone();
        self.evaluations += 1;
        sim::exp_rate(&self.scratch, self.ctx).total
    }

    /// Magnifiers the layout's squares grant, beyond the pool it came in with.
    fn grants(&mut self, grid: &GridState) -> usize {
        self.scratch.grid = grid.clone();
        (sim::magnifiers_owned(&self.scratch, self.ctx) as usize).saturating_sub(self.owned)
    }

    fn exhausted(&self) -> bool {
        self.evaluations >= self.limit
    }

    /// One point at a time, on the square that raises the whole layout most.
    fn climb(&mut self, start: &GridState, free: u32, ledger: &mut Ledger) -> GridState {
        let mut grid = start.clone();
        let mut rate = self.rate(&grid);
        for _ in 0..free {
            let mut best: Option<(f64, usize)> = None;
            for cell in legal_cells(&grid) {
                let mut trial = grid.clone();
                trial.set(cell, trial.level(cell) + 1);
                let value = self.rate(&trial);
                let better = match best {
                    None => true,
                    Some((current, id)) => value > current || (value == current && cell < id),
                };
                if better {
                    best = Some((value, cell));
                }
            }
            let Some((value, cell)) = best else { break };
            if value <= rate {
                break;
            }
            grid.set(cell, grid.level(cell) + 1);
            rate = value;
            ledger.spend(cell);
        }
        grid
    }

    /// Beam search: plan `depth` points ahead over a beam of `width` layouts,
    /// commit the best plan, then re-plan with what is left.
    fn beam(
        &mut self,
        start: &GridState,
        free: u32,
        width: usize,
        depth: usize,
        ledger: &mut Ledger,
    ) -> GridState {
        let mut grid = start.clone();
        let mut remaining = free;
        while remaining > 0 && !self.exhausted() {
            let Some(plan) = self.plan(&grid, remaining, width, depth) else {
                break;
            };
            let gained = plan.spent().saturating_sub(grid.spent());
            if gained == 0 {
                break;
            }
            ledger.record(&grid, &plan);
            grid = plan;
            remaining -= gained.min(remaining);
        }
        grid
    }

    /// The best layout at most `depth` points away, from a beam of `width`.
    fn plan(
        &mut self,
        start: &GridState,
        points: u32,
        width: usize,
        depth: usize,
    ) -> Option<GridState> {
        let start_rate = self.rate(start);
        let mut beam = vec![(start_rate, start.clone())];
        let mut levels = 0;
        for _ in 0..depth.min(points as usize) {
            let mut children: Vec<(f64, GridState)> = Vec::new();
            for (_, grid) in &beam {
                for cell in legal_cells(grid) {
                    let mut trial = grid.clone();
                    trial.set(cell, trial.level(cell) + 1);
                    let rate = self.rate(&trial);
                    children.push((rate, trial));
                }
                if self.exhausted() {
                    break;
                }
            }
            if children.is_empty() {
                break;
            }
            // Highest rate first; the sort is stable, so equal layouts keep the
            // square order of `legal_cells` and the run stays byte-identical.
            children.sort_by(|a, b| b.0.total_cmp(&a.0));
            children.dedup_by(|a, b| a.1.levels() == b.1.levels());
            children.truncate(width);
            beam = children;
            levels += 1;
            if self.exhausted() {
                break;
            }
        }
        if levels == 0 {
            return None;
        }
        let (rate, grid) = beam.into_iter().next()?;
        if rate <= start_rate {
            return None;
        }
        Some(grid)
    }

    /// Move one point from a leveled square to another when that raises the
    /// layout: the repair the beam cannot make, because it never takes points back.
    fn refine(&mut self, grid: &mut GridState, ledger: &mut Ledger) -> bool {
        let base = self.rate(grid);
        let mut best: Option<(f64, usize, usize)> = None;
        let donors: Vec<usize> = grid.live_cells().collect();
        for donor in donors {
            let mut dropped = grid.clone();
            dropped.set(donor, dropped.level(donor) - 1);
            let mut trial = dropped.clone();
            for receipt in legal_cells(&dropped) {
                trial.set(receipt, dropped.level(receipt) + 1);
                let rate = self.rate(&trial);
                trial.set(receipt, dropped.level(receipt));
                if best.is_some_and(|(current, ..)| rate <= current) {
                    continue;
                }
                best = Some((rate, donor, receipt));
                if self.exhausted() {
                    break;
                }
            }
            if self.exhausted() {
                break;
            }
        }
        let Some((rate, donor, receipt)) = best else {
            return false;
        };
        if rate <= base {
            return false;
        }
        grid.set(donor, grid.level(donor) - 1);
        grid.set(receipt, grid.level(receipt) + 1);
        ledger.moved(donor, receipt);
        true
    }
}

/// What the run changed, so the steps read as an account of the spending.
struct Ledger {
    /// Net points added to each square.
    points: Vec<u32>,
    /// Points the refine pass took off one square and put on another.
    moves: u32,
}

impl Ledger {
    fn new() -> Self {
        Self {
            points: vec![0; game().cells()],
            moves: 0,
        }
    }

    fn spend(&mut self, cell: usize) {
        self.points[cell] += 1;
    }

    /// Fold a full layout into the ledger, keeping only the points it adds.
    fn record(&mut self, from: &GridState, to: &GridState) {
        for cell in 0..game().cells() {
            let before = from.level(cell);
            let after = to.level(cell);
            if after > before {
                self.points[cell] += after - before;
            }
        }
    }

    fn moved(&mut self, _donor: usize, receipt: usize) {
        self.points[receipt] += 1;
        self.moves += 1;
    }

    fn summary(&self, spent: u32, before: f64, after: f64) -> Option<String> {
        let squares = self.points.iter().filter(|points| **points > 0).count();
        if squares == 0 || spent == 0 {
            return None;
        }
        let mut ranked: Vec<(usize, u32)> = self
            .points
            .iter()
            .enumerate()
            .filter(|(_, points)| **points > 0)
            .map(|(cell, points)| (cell, *points))
            .collect();
        ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
        let names: Vec<String> = ranked
            .iter()
            .take(3)
            .map(|(cell, points)| format!("{} +{points}", square_name(*cell)))
            .collect();
        Some(format!(
            "Spent {spent} points across {squares} squares ({}): {} EXP/hr to {}.",
            names.join(", "),
            sim::format_rate(before),
            sim::format_rate(after)
        ))
    }
}

fn square_name(cell: usize) -> &'static str {
    game()
        .node_at(cell)
        .map(|node| node.name.as_str())
        .unwrap_or("a square")
}

/// Top a magnifier pool up to `owned` with glasses on the richest free
/// observations. The grid servers fill the pool the same greedy way once the
/// spending is applied, so the search prices layouts the way the run ends.
fn fill_pool(mags: &mut Vec<Mag>, usable: &[usize], cap: usize, owned: usize) {
    while mags.len() < owned {
        let mut best: Option<(f64, usize)> = None;
        for slot in usable {
            let held = mags.iter().filter(|mag| mag.slot == Some(*slot)).count();
            if held >= cap {
                continue;
            }
            let value = sim::obs_base_exp(*slot);
            if best.is_none_or(|(current, _)| value > current) {
                best = Some((value, *slot));
            }
        }
        let Some((_, slot)) = best else { return };
        mags.push(Mag::new(MagKind::Regular, Some(slot)));
    }
}
