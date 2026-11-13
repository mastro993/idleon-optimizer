//! The target-aware run: a port of the event loop in upstream `unifiedSim`
//! (`js/sim-engine.js`).
//!
//! The EXP rate is constant between events, so the loop jumps from one event to
//! the next: a research level-up, an insight level-up, a tournament registration
//! or the target's horizon. On a research level-up the run spends the grid
//! points the new level earned and re-fits the magnifiers and shapes to the new
//! layout; an insight level-up re-fits them when the grid reads insight for EXP
//! (`insightAffectsExp`), because that is when which observation holds the
//! glasses can change.

use crate::data::game;
use crate::model::{Effort, SimContext, SimOptions, Simulation, Target};
use crate::optimize::{grid, mags, shapes};
use crate::sim::{
    self, Event, Projection, Sample, TOURNAMENT_BONUS_HOURS, TOURNAMENT_INTERVAL_HOURS,
    advance_level, first_tournament_hours, insight_exp_req, insight_rate, kalei_base,
    kaleidoscope_map,
};
use idleon_common::Control;
use idleon_common::Progress;

/// How the run treats the layout it starts with.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refit {
    /// Keep the layout fixed: the plain projection.
    Fixed,
    /// Spend each level's grid points and re-fit magnifiers and shapes.
    Level,
}

/// A finished run: what it reached and the state it reached it in.
#[derive(Clone, Debug)]
pub struct TargetRun {
    pub projection: Projection,
    /// The run's state at its last event, with every re-fit applied.
    pub state: Simulation,
    /// Grid points the re-fits spent on the way.
    pub grid_points: u32,
}

/// Simulate `sim` to `target`, reporting progress and honouring a cancel.
///
/// A stopped report returns `None`, so the caller keeps the result it had.
/// Without [`Refit::Level`] this is the plain projection: EXP, levels, insight
/// and observation unlocks only.
pub fn simulate(
    sim: &Simulation,
    ctx: &SimContext,
    target: Target,
    options: SimOptions,
    refit: Refit,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<TargetRun> {
    let start_level = sim.research_level;
    let mut state = sim.clone();
    let mut hours = 0.0;
    let mut total_exp = 0.0;
    let mut insight_gained = 0;
    let mut grid_points = 0;
    let mut samples = vec![Sample {
        hours: 0.0,
        level: start_level,
        insight: 0,
        rate: sim::exp_rate(&state, ctx).total,
        event: Event::Start,
        bonus: 0.0,
    }];

    let max_hours = match target {
        Target::Level(_) => f64::INFINITY,
        Target::Hours(hours) => hours,
    };
    let target_level = match target {
        Target::Level(level) => Some(level),
        Target::Hours(_) => None,
    };
    let mut next_tournament = if options.include_tournament {
        first_tournament_hours(ctx)
    } else {
        f64::INFINITY
    };

    for jump in 0..MAX_JUMPS {
        if hours >= max_hours {
            break;
        }
        if target_level.is_some_and(|level| state.research_level >= level) {
            break;
        }
        let rate = sim::exp_rate(&state, ctx).total;
        if rate <= 0.0 {
            break;
        }
        if jump > 0 && jump % REPORT_EVERY == 0 {
            let progress = Progress {
                fraction: progress_fraction(target, start_level, &state, hours),
                message: format!(
                    "Simulating level {} at {} EXP/hr…",
                    state.research_level,
                    sim::format_rate(rate)
                ),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }

        // Time to the next research level and the next insight level.
        let exp_req = sim::research_exp_req(state.research_level, ctx.server_var_res_xp);
        let to_level = ((exp_req - state.research_exp) / rate).max(0.0);

        let overlay = state.overlay();
        let kal_map = kaleidoscope_map(&state.mags, game().observations.len());
        let kal_base = kalei_base(&state.grid, &overlay, ctx);
        let mut to_insight = f64::INFINITY;
        for obs in 0..game().observations.len() {
            let rate = insight_rate(obs, &state, &overlay, ctx, &kal_map, kal_base);
            if rate <= 0.0 {
                continue;
            }
            let req = insight_exp_req(obs, state.observations.insight_level[obs]);
            let left = ((req - state.observations.insight_exp[obs]) / rate).max(0.0);
            if left < to_insight {
                to_insight = left;
            }
        }

        let to_tournament = (next_tournament - hours).max(0.0);
        let step = to_level
            .min(to_insight)
            .min(max_hours - hours)
            .min(to_tournament);
        let step = if step.is_finite() {
            step.max(1e-9)
        } else {
            0.0
        };
        if step <= 0.0 && to_level > 0.0 && to_insight.is_infinite() {
            break;
        }

        hours += step;
        let gained = rate * step;
        state.research_exp += gained;
        total_exp += gained;
        let fired_tournament = options.include_tournament && hours >= next_tournament - 1e-9;
        let mut bonus = 0.0;

        let level_before = state.research_level;
        advance_level(
            &mut state.research_level,
            &mut state.research_exp,
            ctx.server_var_res_xp,
        );
        let leveled = state.research_level > level_before;
        if leveled && options.assume_observations {
            sim::unlock_observations(&mut state);
        }

        // One collection event covers every observation with a monocle, as
        // upstream `advanceInsightLevels` reads the whole board.
        let insight_before: u32 = state.observations.insight_level.iter().sum();
        insight_gained += sim::advance_insight_hours(&mut state, ctx, step);
        let insight_leveled = state.observations.insight_level.iter().sum::<u32>() > insight_before;

        if refit == Refit::Level {
            if leveled {
                grid_points += refit_layout(&mut state, ctx, options);
            } else if insight_leveled && insight_affects_exp(&state, ctx) {
                // No points were earned, so only the assignment can move: an
                // insight level changes which observation the glasses belong on.
                refit_magnifiers(&mut state, ctx, options);
            }
        }

        if fired_tournament {
            // The registration injects raw research time at the current rate and
            // one insight collection for the monocle board.
            bonus = rate * TOURNAMENT_BONUS_HOURS;
            state.research_exp += bonus;
            total_exp += bonus;
            insight_gained += sim::advance_insight_hours(&mut state, ctx, TOURNAMENT_BONUS_HOURS);
            let level_before = state.research_level;
            advance_level(
                &mut state.research_level,
                &mut state.research_exp,
                ctx.server_var_res_xp,
            );
            let leveled = state.research_level > level_before;
            if leveled && options.assume_observations {
                sim::unlock_observations(&mut state);
            }
            if refit == Refit::Level && leveled {
                grid_points += refit_layout(&mut state, ctx, options);
            }
            next_tournament += TOURNAMENT_INTERVAL_HOURS;
        }

        samples.push(Sample {
            hours,
            level: state.research_level,
            insight: insight_gained,
            rate: sim::exp_rate(&state, ctx).total,
            event: if fired_tournament {
                Event::Tournament
            } else if leveled && insight_leveled {
                Event::LevelAndInsight
            } else if leveled {
                Event::Level
            } else if insight_leveled {
                Event::Insight
            } else {
                Event::End
            },
            bonus,
        });
    }

    Some(TargetRun {
        projection: Projection {
            hours,
            final_level: state.research_level,
            total_exp,
            insight_gained,
            samples,
        },
        state,
        grid_points,
    })
}

/// Whether `candidate` is the better plan for the target.
///
/// A level target ranks by the time it takes to reach the level; an hour target
/// by the EXP earned in the window, with the level breaking a tie. Upstream's
/// worker pool sorts its finished simulations the same way.
pub fn better(candidate: &Projection, current: &Projection, target: Target) -> bool {
    match target {
        Target::Level(level) => {
            let candidate_reached = candidate.final_level >= level;
            let current_reached = current.final_level >= level;
            if candidate_reached != current_reached {
                return candidate_reached;
            }
            if candidate.hours != current.hours {
                return candidate.hours < current.hours;
            }
            if candidate.final_level != current.final_level {
                return candidate.final_level > current.final_level;
            }
            candidate.total_exp > current.total_exp
        }
        Target::Hours(_) => {
            if candidate.total_exp != current.total_exp {
                return candidate.total_exp > current.total_exp;
            }
            candidate.final_level > current.final_level
        }
    }
}

/// Event jumps a run may take before it gives up, as upstream `MAX_JUMPS`.
const MAX_JUMPS: usize = 50_000;
/// Jumps between progress reports, as upstream yields every 40.
const REPORT_EVERY: usize = 40;

/// Spend what the level earned and fit the magnifiers and shapes to it.
///
/// The run uses the instant marginal climb and greedy passes rather than the
/// full search: this repeats on every level-up, and the starting layout is
/// already the search's answer. Returns the points the climb spent.
fn refit_layout(state: &mut Simulation, ctx: &SimContext, options: SimOptions) -> u32 {
    let before = state.grid.spent();
    let spent = grid::respec(state, ctx, Effort::Quick, None, &mut |_| Control::Continue);
    state.grid = spent.grid;

    let placed = shapes::optimize(state, ctx, Effort::Quick, &mut |_| Control::Continue);
    state.placements = placed.placements;
    state.saved_overlay = None;

    refit_magnifiers(state, ctx, options);
    state.grid.spent().saturating_sub(before)
}

/// Re-fit the magnifier assignment to the current observations and grid.
fn refit_magnifiers(state: &mut Simulation, ctx: &SimContext, options: SimOptions) {
    let fitted = mags::optimize(state, ctx, Effort::Quick, options);
    state.mags = fitted.mags;
}

/// `insightAffectsExp`: whether the grid reads insight levels for EXP, so an
/// insight level-up is worth re-fitting the layout for.
fn insight_affects_exp(sim: &Simulation, ctx: &SimContext) -> bool {
    let overlay = sim.overlay();
    sim::grid_bonus(&sim.grid, &overlay, 93, ctx) > 0.0
        || sim::grid_bonus(&sim.grid, &overlay, 94, ctx) > 0.0
}

/// How far into the target the run is, for the progress bar.
fn progress_fraction(target: Target, start_level: u32, state: &Simulation, hours: f64) -> f32 {
    let fraction = match target {
        Target::Level(level) => {
            let span = level.saturating_sub(start_level).max(1) as f32;
            state.research_level.saturating_sub(start_level) as f32 / span
        }
        Target::Hours(limit) => (hours / limit.max(1e-9)) as f32,
    };
    fraction.clamp(0.0, 0.99)
}
