//! What the window does when the cog section is edited or run.

use crate::state::{AppState, discard_run};
use gpui::{App, Context};
use idleon_cog::{
    CogResult, Goal as CogGoal, OptimizeOptions, PerfectBoard, optimize as optimize_cog_board,
    perfect_board as perfect_cog_board,
};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The cog optimizer's run: running, progress, result and staleness.
    pub(crate) fn cog_task(&self, cx: &App) -> TaskState<CogResult> {
        self.cog_run.read(cx).state(&self.cog.inputs())
    }

    /// The perfect board's run: running, progress, result and staleness.
    pub(crate) fn cog_perfect_task(&self, cx: &App) -> TaskState<PerfectBoard> {
        self.cog_perfect_run
            .read(cx)
            .state(&self.cog.perfect_inputs())
    }

    /// Change the objective; a stored result becomes stale until it reruns.
    pub(crate) fn set_cog_goal(&mut self, goal: CogGoal, cx: &mut Context<Self>) {
        if self.cog.goal != goal {
            self.cog.goal = goal;
            self.set_status(format!("Cog objective: {}.", goal.label()), cx);
        }
    }

    /// Turn the custom weights on or off; a stored result becomes stale.
    pub(crate) fn set_cog_weighted(&mut self, weighted: bool, cx: &mut Context<Self>) {
        if self.cog.weighted != weighted {
            self.cog.weighted = weighted;
            self.set_status(
                if weighted {
                    "Cog objective: custom weights.".into()
                } else {
                    format!("Cog objective: {}.", self.cog.goal.label())
                },
                cx,
            );
        }
    }

    /// Replace one custom weight; the controls clamp it to 0..=100.
    pub(crate) fn set_cog_weight(&mut self, goal: CogGoal, value: f64, cx: &mut Context<Self>) {
        let mut weights = self.cog.weights;
        weights.set(goal, value);
        if self.cog.weights != weights {
            self.cog.weights = weights;
            self.set_status(
                match self.cog.weight_problem() {
                    Some(problem) => format!("Cog weights: {problem}"),
                    None => format!("Cog {} weight: {:.0}.", goal.label(), value),
                },
                cx,
            );
        }
    }

    /// Optimize the board off the UI thread through the shared task runner.
    pub(crate) fn optimize_cog(&mut self, cx: &mut Context<Self>) {
        if let Some(problem) = self.cog.weight_problem() {
            self.set_status(format!("Cog weights: {problem}"), cx);
            return;
        }
        self.cog.dismiss_walkthrough();
        self.cog.show_perfect = false;
        let inputs = self.cog.inputs();
        if inputs.board.iter().all(Option::is_none) {
            self.set_status("Cog board: no cogs to place.", cx);
            return;
        }
        self.cog_perfect_run.update(cx, |runner, _| {
            runner.cancel();
        });
        self.start_run(
            self.cog_run.clone(),
            "Optimizing the cog board…",
            inputs,
            move |inputs, report| {
                optimize_cog_board(
                    &inputs.board,
                    &inputs.shelf,
                    &inputs.objective,
                    &OptimizeOptions::default(),
                    report,
                )
            },
            cx,
        );
    }

    /// Apply the next swap in the open walkthrough.
    pub(crate) fn next_cog_swap(&mut self, cx: &mut Context<Self>) {
        if !self.cog.next_swap() {
            return;
        }
        let (applied, total, complete) =
            self.cog
                .walkthrough
                .as_ref()
                .map_or((0, 0, false), |walkthrough| {
                    (
                        walkthrough.applied(),
                        walkthrough.total(),
                        walkthrough.is_complete(),
                    )
                });
        self.set_status(
            if complete {
                format!("Cog walkthrough: all steps shown ({total}); press Done to keep the board.")
            } else {
                format!("Cog walkthrough: step {} of {total}.", applied + 1)
            },
            cx,
        );
    }

    /// Restore the board before the last applied swap.
    pub(crate) fn previous_cog_swap(&mut self, cx: &mut Context<Self>) {
        if !self.cog.previous_swap() {
            return;
        }
        let (applied, total) = self.cog.walkthrough.as_ref().map_or((0, 0), |walkthrough| {
            (walkthrough.applied(), walkthrough.total())
        });
        self.set_status(
            if applied == 0 {
                format!("Cog walkthrough: back at the run's start board; step 1 of {total}.")
            } else {
                format!("Cog walkthrough: step {} of {total}.", applied + 1)
            },
            cx,
        );
    }

    /// Keep the optimized board the walkthrough ends on.
    pub(crate) fn finish_cog_swaps(&mut self, cx: &mut Context<Self>) {
        // Done changes the board itself, so the stored result's inputs must move
        // with it; otherwise the run would read as stale straight after Done. A
        // genuinely stale result (a goal edit, say) stays stale.
        let stale = self.cog_task(cx).stale;
        if !self.cog.finish_walkthrough() {
            return;
        }
        if !stale {
            let inputs = self.cog.inputs();
            self.cog_run
                .update(cx, |runner, _| runner.commit_inputs(inputs));
        }
        self.set_status(
            "Cog walkthrough: final board kept; Reset restores the save board.",
            cx,
        );
    }

    /// Ask a running cog optimization to stop at its next report.
    pub(crate) fn cancel_cog(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.cog_run.clone(), "Cancelling the cog optimization…", cx);
    }

    /// Build the perfect board off the UI thread. It needs a player cog on the
    /// save board, which the section's button checks before calling this.
    pub(crate) fn perfect_cog(&mut self, cx: &mut Context<Self>) {
        if idleon_cog::player_slot(&self.cog.save.board).is_none() {
            self.set_status("Perfect board needs a player cog on the save board.", cx);
            return;
        }
        self.cog.dismiss_walkthrough();
        self.cog.show_perfect = true;
        self.cog_run.update(cx, |runner, _| {
            runner.cancel();
        });
        let inputs = self.cog.perfect_inputs();
        self.start_run(
            self.cog_perfect_run.clone(),
            "Building the perfect cog board…",
            inputs,
            move |inputs, report| {
                perfect_cog_board(
                    &inputs.board,
                    &inputs.shelf,
                    inputs.construction_level,
                    inputs.goal,
                    report,
                )
            },
            cx,
        );
    }

    /// Ask a running perfect-board build to stop at its next report.
    pub(crate) fn cancel_cog_perfect(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.cog_perfect_run.clone(),
            "Cancelling the perfect board…",
            cx,
        );
    }

    /// Restore the save board and drop both stored results.
    pub(crate) fn reset_cog(&mut self, cx: &mut Context<Self>) {
        self.cog.reset();
        discard_run(&self.cog_run, cx);
        discard_run(&self.cog_perfect_run, cx);
        self.set_status("Cog board reset to the save state.", cx);
    }
}
