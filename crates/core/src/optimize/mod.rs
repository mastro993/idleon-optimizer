//! Optimizers: shape placement, magnifier assignment, grid spending, and the
//! run that chains them.

pub mod grid;
pub mod mags;
pub mod shapes;

use crate::model::{
    Effort, OptimizeRequest, OptimizeResult, Progress, SimContext, Simulation,
};
use crate::sim;
use std::time::Instant;

/// Run the optimizers for a request and report the before and after states.
pub fn run(
    simulation: &Simulation,
    ctx: &SimContext,
    request: &OptimizeRequest,
    report: &mut dyn FnMut(Progress),
) -> OptimizeResult {
    let started = Instant::now();
    let before = simulation.clone();
    let before_rate = sim::exp_rate(&before, ctx).total;
    let mut after = before.clone();
    let mut steps = Vec::new();

    report(Progress {
        fraction: 0.02,
        message: "Placing magnifiers…".into(),
    });
    let mags_outcome = mags::optimize(&after, ctx);
    after.mags = mags_outcome.mags;
    steps.extend(mags_outcome.steps);

    let shapes_outcome = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
        report(Progress {
            fraction: 0.1 + progress.fraction * 0.6,
            message: progress.message,
        })
    });
    after.placements = shapes_outcome.placements;
    steps.extend(shapes_outcome.steps);

    if request.effort == Effort::Thorough || request.respec_grid {
        report(Progress {
            fraction: 0.8,
            message: "Spending grid points…".into(),
        });
        let grid_outcome = grid::respec(&after, ctx, request.effort, request.grid_points);
        after.grid = grid_outcome.grid;
        steps.extend(grid_outcome.steps);

        let reshaped = shapes::optimize(&after, ctx, request.effort, &mut |progress| {
            report(Progress {
                fraction: 0.85 + progress.fraction * 0.1,
                message: progress.message,
            })
        });
        after.placements = reshaped.placements;
        steps.extend(reshaped.steps);
    }

    report(Progress {
        fraction: 0.97,
        message: "Projecting the run…".into(),
    });
    let after_rate = sim::exp_rate(&after, ctx).total;
    let projection = sim::project(&after, ctx, request.target);
    report(Progress {
        fraction: 1.0,
        message: "Done.".into(),
    });

    OptimizeResult {
        before,
        after,
        before_rate,
        after_rate,
        projection,
        steps,
        elapsed_ms: started.elapsed().as_millis(),
    }
}
