//! Optimizers: magnifier assignment, shape placement, grid spending, and the
//! run that chains them.

pub mod grid;
pub mod mags;
pub mod shapes;

use crate::model::{
    Control, Effort, OptimizeRequest, OptimizeResult, Progress, SimContext, Simulation,
};
use crate::sim;
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

    if report(Progress {
        fraction: 0.02,
        message: "Assigning magnifiers…".into(),
    }) == Control::Stop
    {
        return None;
    }
    let mags_outcome = mags::optimize(&after, ctx, request.effort);
    after.mags = mags_outcome.mags;
    steps.extend(mags_outcome.steps);

    let mut stopped = false;
    let shapes_outcome = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
        let control = report(Progress {
            fraction: 0.05 + progress.fraction * 0.55,
            message: progress.message,
        });
        stopped |= control == Control::Stop;
        control
    });
    if stopped {
        return None;
    }
    after.placements = shapes_outcome.placements;
    steps.extend(shapes_outcome.steps);

    let thorough = request.effort == Effort::Thorough;
    if thorough || request.respec_grid {
        let grid_outcome = grid::respec(
            &after,
            ctx,
            request.effort,
            request.grid_points,
            &mut |progress| {
                let control = report(Progress {
                    fraction: 0.62 + progress.fraction * 0.2,
                    message: progress.message,
                });
                stopped |= control == Control::Stop;
                control
            },
        );
        if stopped {
            return None;
        }
        after.grid = grid_outcome.grid;
        steps.extend(grid_outcome.steps);

        let reshaped = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
            let control = report(Progress {
                fraction: 0.84 + progress.fraction * 0.1,
                message: progress.message,
            });
            stopped |= control == Control::Stop;
            control
        });
        if stopped {
            return None;
        }
        after.placements = reshaped.placements;
        steps.extend(reshaped.steps);

        if report(Progress {
            fraction: 0.95,
            message: "Re-fitting magnifiers to the new levels…".into(),
        }) == Control::Stop
        {
            return None;
        }
        let remags = mags::optimize(&after, ctx, request.effort);
        after.mags = remags.mags;
        steps.extend(remags.steps);
    }

    if report(Progress {
        fraction: 0.97,
        message: "Projecting the run…".into(),
    }) == Control::Stop
    {
        return None;
    }
    let after_rate = sim::exp_rate(&after, ctx).total;
    // A level target already met says nothing about the layout; project a day.
    let projection = match request.target {
        crate::model::Target::Level(level) if after.research_level >= level => {
            steps.push(format!(
                "Research level {level} is already reached; projecting 24 hours from here."
            ));
            sim::project(&after, ctx, crate::model::Target::Hours(24.0))
        }
        target => sim::project(&after, ctx, target),
    };
    let gain = if before_rate > 0.0 {
        (after_rate / before_rate - 1.0) * 100.0
    } else {
        0.0
    };
    steps.push(format!(
        "Finished at {} EXP/hr ({gain:+.1}%); the projection ends at level {} after {}.",
        sim::format_rate(after_rate),
        projection.final_level,
        hours_label(projection.hours)
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
        projection,
        steps,
        elapsed_ms: started.elapsed().as_millis(),
    })
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
