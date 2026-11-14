//! Insight ROI: what one observation's next insight levels are worth.
//!
//! Ported from `computeInsightROI` in the upstream `js/analysis.js` and
//! `buildConcentratedLayout` in `js/optimizers/monos.js`. Every row grinds one
//! observation by moving the monocles onto it, then asks what the level buys:
//!
//! - `grind_hours`: how long the concentrated layout needs for the level;
//! - `total_exp_lost`: the research EXP/hour the grind gives up, times those
//!   hours, measured against the best non-grind layout;
//! - `rate_gain`: what the new insight level permanently adds once the
//!   post-grind layout is re-optimized;
//! - `break_even_hours`: the grind plus the hours the gain needs to earn the
//!   lost EXP back.

use crate::data::game;
use crate::model::{Effort, Mag, MagKind, SimContext, SimOptions, Simulation};
use crate::optimize::{mags, shapes};
use crate::sim;
use idleon_common::{Control, Progress};

/// One way to spend insight on an observation: one level, or two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Scenario {
    /// Insight levels the grind buys.
    pub level_gain: u32,
    /// Hours the concentrated layout needs for them.
    pub grind_hours: f64,
    /// Research EXP/hour the grind layout earns.
    pub grind_rate: f64,
    /// EXP/hour the grind gives up against the baseline.
    pub exp_lost_per_hour: f64,
    /// `exp_lost_per_hour × grind_hours`.
    pub total_exp_lost: f64,
    /// EXP/hour once the level is in and the layout re-optimized.
    pub post_rate: f64,
    /// `post_rate - baseline`; the permanent gain.
    pub rate_gain: f64,
    /// Hours the gain needs to earn the lost EXP back, endless with no gain.
    pub recoup_hours: f64,
    /// `grind_hours + recoup_hours`.
    pub break_even_hours: f64,
    /// Whether the scenario finishes finite and ahead.
    pub worth: bool,
    /// Insight EXP/hour the observation earns while grinding.
    pub insight_rate: f64,
}

/// One observation's row: its level, the layouts the grind uses and both
/// scenarios upstream shows.
#[derive(Clone, Debug)]
pub struct Row {
    pub obs: usize,
    pub name: String,
    /// Insight level before the grind.
    pub level: u32,
    /// The one-level and two-level scenarios, in that order.
    pub scenarios: [Scenario; 2],
    /// The magnifiers the grind uses: the monocles concentrated on the target.
    pub grind_mags: Vec<Mag>,
    /// The shapes the grind uses; `None` keeps the save's own overlay.
    pub grind_overlay: Option<Vec<i32>>,
    /// Whether the grind swaps in insight-valued shapes.
    pub insight_shapes: bool,
    /// The magnifiers the post-grind layout uses.
    pub after_mags: Vec<Mag>,
    /// The post-grind shape overlay.
    pub after_overlay: Vec<i32>,
}

/// The tab's stored result.
#[derive(Clone, Debug, Default)]
pub struct Outcome {
    /// Optical monocles in the pool; no monocles means no insight grind.
    pub monocles: u32,
    /// EXP/hour of the baseline: optimized magnifiers, the save's shapes.
    pub base_rate: f64,
    /// The baseline's magnifiers.
    pub base_mags: Vec<Mag>,
    /// The baseline's shape overlay.
    pub base_overlay: Vec<i32>,
    /// One row per usable observation, best break-even first.
    pub rows: Vec<Row>,
}

impl Outcome {
    /// The rows in the order the tab's sort control asks for.
    pub fn sorted(&self, sort: Sort) -> Vec<&Row> {
        let mut rows: Vec<&Row> = self.rows.iter().collect();
        rows.sort_by(|a, b| sort.compare(a, b));
        rows
    }
}

/// The upstream sort modes for the table.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Sort {
    /// Hours the grind takes, shortest first.
    GrindTime,
    /// Permanent EXP/hour bought, largest first.
    RateGain,
    /// Rate gain per hour of break-even time, largest first.
    Efficiency,
    /// Hours until the level and its gain pay back, shortest first.
    #[default]
    BreakEven,
}

impl Sort {
    pub const ALL: [Sort; 4] = [
        Sort::GrindTime,
        Sort::RateGain,
        Sort::BreakEven,
        Sort::Efficiency,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Sort::GrindTime => "grindtime",
            Sort::RateGain => "rategain",
            Sort::BreakEven => "breakeven",
            Sort::Efficiency => "efficiency",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Sort::GrindTime => "Grind Time",
            Sort::RateGain => "Rate Gain",
            Sort::BreakEven => "Break-Even",
            Sort::Efficiency => "Efficiency",
        }
    }

    /// The upstream comparator, read against each row's one-level scenario.
    fn compare(self, left: &Row, right: &Row) -> std::cmp::Ordering {
        let left = &left.scenarios[0];
        let right = &right.scenarios[0];
        match self {
            Sort::GrindTime => left.grind_hours.total_cmp(&right.grind_hours),
            Sort::RateGain => right.rate_gain.total_cmp(&left.rate_gain),
            Sort::Efficiency => efficiency(right).total_cmp(&efficiency(left)),
            Sort::BreakEven => left.break_even_hours.total_cmp(&right.break_even_hours),
        }
    }
}

/// The permanent EXP/hour a scenario buys per hour of break-even time.
pub fn efficiency(scenario: &Scenario) -> f64 {
    if scenario.rate_gain <= 0.0 {
        return f64::NEG_INFINITY;
    }
    if scenario.break_even_hours > 0.0 && scenario.break_even_hours.is_finite() {
        scenario.rate_gain / scenario.break_even_hours
    } else {
        f64::INFINITY
    }
}

/// A post-grind layout: its magnifiers and shapes.
struct After {
    mags: Vec<Mag>,
    overlay: Vec<i32>,
}

/// Analyze every usable observation, reporting progress per observation.
///
/// Returns `None` when the run was asked to stop; nothing is stored then.
pub fn analyze(
    sim: &Simulation,
    ctx: &SimContext,
    effort: Effort,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<Outcome> {
    let monocles = sim
        .mags
        .iter()
        .filter(|mag| mag.kind == MagKind::Monocle)
        .count() as u32;
    if monocles == 0 {
        return Some(Outcome::default());
    }
    let base = mags::optimize(sim, ctx, effort, SimOptions::default());
    let base_overlay = sim.overlay();
    let base_rate = rate(sim, ctx, &base.mags, &base_overlay);
    // The research side of the blend reads the pool, not the optimized layout.
    let research_values = sim::bare_cell_values(sim, ctx);
    let usable = sim::available_slots(sim.research_level, &sim.observations);
    let total = usable.len().max(1);
    let mut rows = Vec::with_capacity(usable.len());
    for (done, obs) in usable.iter().copied().enumerate() {
        let name = game().observations[obs].name.replace('_', " ");
        if report(Progress {
            fraction: done as f32 / total as f32,
            message: format!("Insight ROI: {name} ({} of {}).", done + 1, usable.len()),
        }) == Control::Stop
        {
            return None;
        }
        rows.push(row(
            sim,
            ctx,
            &base.mags,
            base_rate,
            &research_values,
            obs,
            effort,
        ));
    }
    rows.sort_by(|left, right| {
        left.scenarios[0]
            .break_even_hours
            .total_cmp(&right.scenarios[0].break_even_hours)
    });
    Some(Outcome {
        monocles,
        base_rate,
        base_mags: base.mags,
        base_overlay,
        rows,
    })
}

/// One observation's grind, after layout and both scenarios.
fn row(
    sim: &Simulation,
    ctx: &SimContext,
    base_mags: &[Mag],
    base_rate: f64,
    research_values: &[f64],
    obs: usize,
    effort: Effort,
) -> Row {
    let level = sim.observations.insight(obs);
    let progress = sim
        .observations
        .insight_exp
        .get(obs)
        .copied()
        .unwrap_or(0.0);
    let current = sim.overlay();

    // The grind layout: the monocles moved onto this observation.
    let grind_mags = concentrated::build(sim, ctx, base_mags, obs, &current);
    // Shapes that pay insight instead of EXP, blended with research value so the
    // grind does not give up more EXP than it has to (upstream 70/30 split).
    let insight_values = insight_cell_values(sim, ctx, &grind_mags, obs);
    let insight_overlay = insight_shapes(sim, ctx, &grind_mags, &insight_values, research_values);
    let grind_mags_shaped = concentrated::build(sim, ctx, base_mags, obs, &insight_overlay);

    let after = after_layout(sim, ctx, obs, level + 1, effort);
    let plain = evaluate(
        sim,
        ctx,
        &grind_mags,
        obs,
        1,
        progress,
        None,
        &after,
        base_rate,
    );
    let shaped = evaluate(
        sim,
        ctx,
        &grind_mags_shaped,
        obs,
        1,
        progress,
        Some(&insight_overlay),
        &after,
        base_rate,
    );
    let insight_shapes = shaped.break_even_hours < plain.break_even_hours;
    let (grind_mags, grind_overlay, first) = if insight_shapes {
        (grind_mags_shaped, Some(insight_overlay.clone()), shaped)
    } else {
        (grind_mags, None, plain)
    };
    let after_two = after_layout(sim, ctx, obs, level + 2, effort);
    let second = evaluate(
        sim,
        ctx,
        &grind_mags,
        obs,
        2,
        progress,
        grind_overlay.as_deref(),
        &after_two,
        base_rate,
    );
    Row {
        obs,
        name: game().observations[obs].name.replace('_', " "),
        level,
        scenarios: [first, second],
        grind_mags,
        grind_overlay,
        insight_shapes,
        after_mags: after.mags,
        after_overlay: after.overlay,
    }
}

/// The rates for one grind scenario, ported from `evalLayout`.
#[allow(clippy::too_many_arguments)]
fn evaluate(
    sim: &Simulation,
    ctx: &SimContext,
    grind_mags: &[Mag],
    obs: usize,
    level_gain: u32,
    progress: f64,
    grind_overlay: Option<&[i32]>,
    after: &After,
    base_rate: f64,
) -> Scenario {
    let current = sim.overlay();
    let overlay = grind_overlay.unwrap_or(&current);
    let insight_rate = insight(sim, ctx, grind_mags, overlay, obs);
    let mut grind_hours = 0.0;
    let start = sim.observations.insight(obs);
    let mut banked = progress;
    for level in start..start + level_gain {
        if insight_rate <= 0.0 {
            grind_hours = f64::INFINITY;
            break;
        }
        grind_hours += (sim::insight_exp_required(obs, level) - banked).max(0.0) / insight_rate;
        banked = 0.0;
    }
    let grind_rate = rate(sim, ctx, grind_mags, overlay);
    let exp_lost_per_hour = (base_rate - grind_rate).max(0.0);
    // A layout that loses nothing loses nothing however long it grinds. Upstream
    // multiplies anyway, which yields NaN for an endless free grind.
    let total_exp_lost = if exp_lost_per_hour > 0.0 {
        exp_lost_per_hour * grind_hours
    } else {
        0.0
    };
    let mut levelled = sim.clone();
    levelled.observations.insight_level[obs] = sim.observations.insight(obs) + level_gain;
    let post_rate = rate(&levelled, ctx, &after.mags, &after.overlay);
    let rate_gain = post_rate - base_rate;
    let recoup_hours = if rate_gain > 0.0 {
        total_exp_lost / rate_gain
    } else {
        f64::INFINITY
    };
    let break_even_hours = grind_hours + recoup_hours;
    Scenario {
        level_gain,
        grind_hours,
        grind_rate,
        exp_lost_per_hour,
        total_exp_lost,
        post_rate,
        rate_gain,
        recoup_hours,
        break_even_hours,
        worth: break_even_hours.is_finite() && rate_gain > 0.0,
        insight_rate,
    }
}

/// The layout the observation is left with after the grind: magnifiers and
/// shapes re-optimized for its new insight level, as upstream `optimizePostGrind`.
fn after_layout(
    sim: &Simulation,
    ctx: &SimContext,
    obs: usize,
    level: u32,
    effort: Effort,
) -> After {
    let mut levelled = sim.clone();
    levelled.observations.insight_level[obs] = level;
    let mags = mags::optimize(&levelled, ctx, effort, SimOptions::default());
    levelled.mags = mags.mags.clone();
    let shapes = shapes::optimize(&levelled, ctx, effort, &mut |_| Control::Continue);
    After {
        mags: mags.mags,
        overlay: shapes.overlay,
    }
}

/// EXP/hour as `simTotalExpWith` reads it: the layout, not the saved state.
fn rate(sim: &Simulation, ctx: &SimContext, mags: &[Mag], overlay: &[i32]) -> f64 {
    let trial = layout(sim, mags, overlay);
    sim::exp_rate(&trial, ctx).total
}

/// Insight EXP/hour of one observation with the given layout.
fn insight(sim: &Simulation, ctx: &SimContext, mags: &[Mag], overlay: &[i32], obs: usize) -> f64 {
    let trial = layout(sim, mags, overlay);
    let kal_map = sim::kaleidoscope_map(mags, game().observations.len());
    let kal_base = sim::kalei_base(&sim.grid, overlay, ctx);
    sim::insight_rate(obs, &trial, overlay, ctx, &kal_map, kal_base)
}

/// The simulation a layout reads through: magnifiers and shapes swapped in.
fn layout(sim: &Simulation, mags: &[Mag], overlay: &[i32]) -> Simulation {
    Simulation {
        mags: mags.to_vec(),
        saved_overlay: Some(overlay.to_vec()),
        ..sim.clone()
    }
}

/// The layouts that move every monocle onto one observation, as upstream
/// `buildConcentratedLayout` does.
///
/// Public so the fixture test can compare the layout the oracle built.
pub fn concentrated_layout(
    sim: &Simulation,
    ctx: &SimContext,
    base: &[Mag],
    target: usize,
    overlay: &[i32],
) -> Vec<Mag> {
    concentrated::build(sim, ctx, base, target, overlay)
}

/// EXP the layout earns from observations alone, before every multiplier.
/// Upstream `evalMagScoreWith`, which ranks evicted magnifiers by EXP.
fn observation_score(sim: &Simulation, ctx: &SimContext, mags: &[Mag], overlay: &[i32]) -> f64 {
    let trial = layout(sim, mags, overlay);
    sim::exp_rate(&trial, ctx).obs_base
}

/// Each live cell's contribution to one observation's insight rate, at the 25%
/// baseline shape bonus (upstream `computeInsightCellValues`).
fn insight_cell_values(sim: &Simulation, ctx: &SimContext, mags: &[Mag], obs: usize) -> Vec<f64> {
    let cells = game().cells();
    let mut bare = vec![-1; cells];
    let base = insight(sim, ctx, mags, &bare, obs);
    let mut values = vec![0.0; cells];
    for cell in 0..cells {
        if sim.grid.level(cell) == 0 || game().node_at(cell).is_none() {
            continue;
        }
        bare[cell] = 0;
        values[cell] = insight(sim, ctx, mags, &bare, obs) - base;
        bare[cell] = -1;
    }
    values
}

/// The grind shapes: mostly insight value, a little research value so the
/// grind keeps as much EXP as it can.
fn insight_shapes(
    sim: &Simulation,
    ctx: &SimContext,
    mags: &[Mag],
    insight_values: &[f64],
    research_values: &[f64],
) -> Vec<i32> {
    let top = |values: &[f64]| {
        values
            .iter()
            .copied()
            .filter(|value| *value > 0.0)
            .fold(0.001, f64::max)
    };
    let insight_top = top(insight_values);
    let research_top = top(research_values);
    let blended: Vec<f64> = (0..game().cells())
        .map(|cell| {
            0.7 * insight_values[cell] / insight_top + 0.3 * research_values[cell] / research_top
        })
        .collect();
    let mut trial = sim.clone();
    trial.mags = mags.to_vec();
    shapes::optimize_for_values(&trial, ctx, &blended, &mut |_| Control::Continue).overlay
}

/// The layouts that move every monocle onto one observation.
mod concentrated {
    use super::*;

    /// Port of `buildConcentratedLayout`: monocles onto the target up to its
    /// cap, evicted magnifiers onto the next best slot, then the kaleidoscopes
    /// one move at a time while that costs less EXP per insight.
    pub(super) fn build(
        sim: &Simulation,
        ctx: &SimContext,
        base: &[Mag],
        target: usize,
        overlay: &[i32],
    ) -> Vec<Mag> {
        let cap = sim::mag_cap(sim.research_level) as usize;
        let mut mags = base.to_vec();
        let monocles = mags
            .iter()
            .filter(|mag| mag.kind == MagKind::Monocle)
            .count();
        if monocles == 0
            || cap == 0
            || !sim::observation_usable(target, sim.research_level, &sim.observations)
        {
            return mags;
        }
        let on_target = |mag: &Mag| mag.slot == Some(target);
        let non_mono = mags
            .iter()
            .filter(|mag| mag.kind != MagKind::Monocle && on_target(mag))
            .count();
        let room = cap.saturating_sub(non_mono);
        let want = monocles.min(cap);
        let mut evicted = Vec::new();
        if want > room {
            let need = want - room;
            for (index, mag) in mags.iter_mut().enumerate() {
                if evicted.len() >= need {
                    break;
                }
                if mag.kind != MagKind::Monocle && on_target(mag) {
                    mag.slot = None;
                    evicted.push(index);
                }
            }
        }
        let actual = want.min(room + evicted.len());
        let mut placed = 0;
        let mut overflow = Vec::new();
        for (index, mag) in mags.iter_mut().enumerate() {
            if mag.kind != MagKind::Monocle {
                continue;
            }
            if placed < actual {
                mag.slot = Some(target);
                placed += 1;
            } else {
                mag.slot = None;
                overflow.push(index);
            }
        }
        let normal = rate(sim, ctx, base, overlay);
        spill_monocles(sim, ctx, &mut mags, &overflow, target, cap, overlay);
        spill_evicted(sim, ctx, &mut mags, &evicted, cap, overlay);
        move_kaleidoscopes(sim, ctx, &mags, target, overlay, normal)
    }

    /// Overflow monocles go where a monocle earns the most insight.
    fn spill_monocles(
        sim: &Simulation,
        ctx: &SimContext,
        mags: &mut [Mag],
        overflow: &[usize],
        target: usize,
        cap: usize,
        overlay: &[i32],
    ) {
        if overflow.is_empty() {
            return;
        }
        let to_find = to_find(sim);
        let mut counts = slot_counts(mags, to_find);
        for index in overflow {
            let mut best: Option<(f64, usize)> = None;
            for (slot, taken) in counts.iter().enumerate() {
                if slot == target
                    || *taken >= cap
                    || !sim::observation_usable(slot, sim.research_level, &sim.observations)
                {
                    continue;
                }
                mags[*index].slot = Some(slot);
                let value = insight(sim, ctx, mags, overlay, slot);
                if best.is_none_or(|(current, _)| value > current) {
                    best = Some((value, slot));
                }
            }
            match best {
                Some((_, slot)) => {
                    mags[*index].slot = Some(slot);
                    counts[slot] += 1;
                }
                None => mags[*index].slot = None,
            }
        }
    }

    /// Evicted magnifiers go to the slot that keeps the most observation EXP.
    fn spill_evicted(
        sim: &Simulation,
        ctx: &SimContext,
        mags: &mut [Mag],
        evicted: &[usize],
        cap: usize,
        overlay: &[i32],
    ) {
        if evicted.is_empty() {
            return;
        }
        let to_find = to_find(sim);
        let mut counts = slot_counts(mags, to_find);
        for index in evicted {
            let mut best: Option<(f64, usize)> = None;
            for (slot, taken) in counts.iter().enumerate() {
                if *taken >= cap
                    || !sim::observation_usable(slot, sim.research_level, &sim.observations)
                {
                    continue;
                }
                mags[*index].slot = Some(slot);
                let value = observation_score(sim, ctx, mags, overlay);
                if best.is_none_or(|(current, _)| value > current) {
                    best = Some((value, slot));
                }
            }
            match best {
                Some((_, slot)) => {
                    mags[*index].slot = Some(slot);
                    counts[slot] += 1;
                }
                None => mags[*index].slot = None,
            }
        }
    }

    /// Move each kaleidoscope where the grind loses less EXP per insight.
    fn move_kaleidoscopes(
        sim: &Simulation,
        ctx: &SimContext,
        mags: &[Mag],
        target: usize,
        overlay: &[i32],
        normal: f64,
    ) -> Vec<Mag> {
        let indices: Vec<usize> = mags
            .iter()
            .enumerate()
            .filter(|(_, mag)| mag.kind == MagKind::Kaleidoscope)
            .map(|(index, _)| index)
            .collect();
        if indices.is_empty() {
            return mags.to_vec();
        }
        let cap = sim::mag_cap(sim.research_level) as usize;
        let usable = sim::available_slots(sim.research_level, &sim.observations);
        let cost = |trial: &[Mag]| {
            let insight_rate = insight(sim, ctx, trial, overlay, target);
            if insight_rate <= 0.0 {
                f64::NEG_INFINITY
            } else {
                -(normal - rate(sim, ctx, trial, overlay)).max(0.0) / insight_rate
            }
        };
        let mut best = mags.to_vec();
        let mut best_score = cost(&best);
        for index in indices {
            let original = best[index].slot;
            for slot in &usable {
                if Some(*slot) == original {
                    continue;
                }
                if best.iter().filter(|mag| mag.slot == Some(*slot)).count() >= cap {
                    continue;
                }
                let mut trial = best.clone();
                trial[index].slot = Some(*slot);
                let score = cost(&trial);
                if score > best_score {
                    best_score = score;
                    best = trial;
                }
            }
        }
        best
    }

    fn to_find(sim: &Simulation) -> usize {
        sim::observations_to_find(sim.research_level, &sim.observations.found)
            .min(game().observations.len())
    }

    fn slot_counts(mags: &[Mag], to_find: usize) -> Vec<usize> {
        let mut counts = vec![0; to_find];
        for mag in mags {
            if let Some(slot) = mag.slot
                && slot < to_find
            {
                counts[slot] += 1;
            }
        }
        counts
    }
}
