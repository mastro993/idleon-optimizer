//! Optimizers: magnifier assignment, shape placement, grid spending, and the
//! run that chains them.
//!
//! The run builds one optimized layout, then simulates the saved layout and the
//! optimized one all the way to the target. A level target keeps whichever
//! reaches the level first and an hour target whichever earns the most EXP in
//! the window, as upstream's worker pool ranks its finished simulations; the
//! immediate EXP/hour only prices the pieces the layout is built from.

pub mod grid;
pub mod mags;
pub mod monos;
pub mod shapes;

use crate::engine::{self, Refit, TargetRun};
use crate::model::{
    Effort, OptimizeRequest, OptimizeResult, SimContext, Simulation, Strategy, Target,
};
use crate::sim;
use idleon_common::Control;
use idleon_common::Progress;
use std::time::Instant;

/// Run the optimizers for a request and report the before and after states.
///
/// Magnifiers go first because they set the rate everything else is measured
/// against; shapes then stake out the squares worth spending on; a Thorough run
/// spends grid points and re-places both, because the spending moves the value
/// of the squares out from under the shapes and the magnifier grants.
///
/// The reporter can return [`Control::Stop`] to cancel; the run then returns
/// `None` so the caller keeps the result it already had.
pub fn run(
    simulation: &Simulation,
    ctx: &SimContext,
    request: &OptimizeRequest,
    report: &mut dyn FnMut(Progress) -> Control,
) -> Option<OptimizeResult> {
    let started = Instant::now();
    let before = simulation.clone();
    let before_rate = sim::exp_rate(&before, ctx).total;
    let mut after = before.clone();
    let mut steps = Vec::new();
    // A level target already met says nothing about the layout; project a day.
    let planned = match request.target {
        Target::Level(level) if before.research_level >= level => {
            steps.push(format!(
                "Research level {level} is already reached; projecting 24 hours from here."
            ));
            Target::Hours(24.0)
        }
        target => target,
    };

    if report(Progress {
        fraction: 0.02,
        message: "Assigning magnifiers…".into(),
    }) == Control::Stop
    {
        return None;
    }
    let mags_outcome = mags::optimize(&after, ctx, request.effort, request.options);
    after.mags = mags_outcome.mags;
    steps.extend(mags_outcome.steps);

    let mut stopped = false;
    let shapes_outcome = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
        let control = report(Progress {
            fraction: 0.05 + progress.fraction * 0.5,
            message: progress.message,
        });
        stopped |= control == Control::Stop;
        control
    });
    if stopped {
        return None;
    }
    after.placements = shapes_outcome.placements;
    after.saved_overlay = None;
    steps.extend(shapes_outcome.steps);

    let thorough = request.effort == Effort::Thorough;
    if thorough || request.respec_grid {
        let grid_outcome = grid::spend_for_target(
            &after,
            ctx,
            request.effort,
            request.grid_points,
            planned,
            request.options,
            &mut |progress| {
                let control = report(Progress {
                    fraction: 0.57 + progress.fraction * 0.2,
                    message: progress.message,
                });
                stopped |= control == Control::Stop;
                control
            },
        )?;
        after.grid = grid_outcome.grid;
        steps.extend(grid_outcome.steps);

        let reshaped = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
            let control = report(Progress {
                fraction: 0.79 + progress.fraction * 0.08,
                message: progress.message,
            });
            stopped |= control == Control::Stop;
            control
        });
        if stopped {
            return None;
        }
        after.placements = reshaped.placements;
        after.saved_overlay = None;
        steps.extend(reshaped.steps);

        if report(Progress {
            fraction: 0.88,
            message: "Re-fitting magnifiers to the new levels…".into(),
        }) == Control::Stop
        {
            return None;
        }
        let remags = mags::optimize(&after, ctx, request.effort, request.options);
        after.mags = remags.mags;
        steps.extend(remags.steps);
    }

    if report(Progress {
        fraction: 0.9,
        message: "Simulating to the target…".into(),
    }) == Control::Stop
    {
        return None;
    }

    // The saved layout is the incumbent; the optimized one has to reach the
    // target sooner (or earn more in the window) to be the run's answer.
    let baseline = engine::simulate(
        &before,
        ctx,
        planned,
        request.options,
        Refit::Level,
        &mut |progress| {
            report(Progress {
                fraction: 0.9 + progress.fraction * 0.045,
                message: progress.message,
            })
        },
    )?;
    let optimized = engine::simulate(
        &after,
        ctx,
        planned,
        request.options,
        Refit::Level,
        &mut |progress| {
            report(Progress {
                fraction: 0.945 + progress.fraction * 0.045,
                message: progress.message,
            })
        },
    )?;
    // The saved layout has to beat the optimized one outright; a tie keeps the
    // plan the optimizer built.
    let optimized_wins = !engine::better(&baseline.projection, &optimized.projection, planned);
    let saved = Strategy {
        label: "Saved layout",
        projection: baseline.projection.clone(),
    };
    let built = Strategy {
        label: "Optimized layout",
        projection: optimized.projection.clone(),
    };
    let (after, chosen, other, (best, worst)) = if optimized_wins {
        (after, &optimized, &baseline, (built, saved))
    } else {
        (before.clone(), &baseline, &optimized, (saved, built))
    };
    steps.push(format!(
        "The {} layout {}; the {} layout {}.",
        if optimized_wins { "optimized" } else { "saved" },
        outcome_label(chosen, planned),
        if optimized_wins { "saved" } else { "optimized" },
        outcome_label(other, planned),
    ));
    if chosen.grid_points > 0 {
        steps.push(format!(
            "The run spends the {} grid points the levels earn on the way and re-fits \
             magnifiers and shapes at every level-up.",
            chosen.grid_points
        ));
    }

    let after_rate = sim::exp_rate(&after, ctx).total;
    let gain = if before_rate > 0.0 {
        (after_rate / before_rate - 1.0) * 100.0
    } else {
        0.0
    };
    steps.push(format!(
        "Finished at {} EXP/hr ({gain:+.1}%); the projection ends at level {} after {}.",
        sim::format_rate(after_rate),
        best.projection.final_level,
        hours_label(best.projection.hours)
    ));
    if report(Progress {
        fraction: 1.0,
        message: "Done.".into(),
    }) == Control::Stop
    {
        return None;
    }

    Some(OptimizeResult {
        before,
        after,
        before_rate,
        after_rate,
        best,
        worst,
        target: planned,
        steps,
        elapsed_ms: started.elapsed().as_millis(),
    })
}

/// How a run fared against the target, for the strategy step.
fn outcome_label(run: &TargetRun, target: Target) -> String {
    let projection = &run.projection;
    match target {
        Target::Level(level) if projection.final_level < level => format!(
            "stalls at level {} after {}",
            projection.final_level,
            hours_label(projection.hours)
        ),
        Target::Level(_) => format!("reaches the level in {}", hours_label(projection.hours)),
        Target::Hours(hours) => format!(
            "earns {} EXP in {}",
            sim::format_rate(projection.total_exp),
            hours_label(hours)
        ),
    }
}

/// Hours as the projection reports them, in a unit a person can read.
fn hours_label(hours: f64) -> String {
    if !hours.is_finite() {
        return "no reachable horizon".into();
    }
    if hours >= 1e6 {
        format!("{:.1}M hours", hours / 1e6)
    } else if hours >= 1.0 {
        format!("{hours:.1} hours")
    } else {
        format!("{:.0} minutes", hours * 60.0)
    }
}
