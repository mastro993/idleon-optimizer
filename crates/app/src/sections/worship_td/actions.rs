//! What the window does when the worship td section is edited or run.

use crate::sections::worship_td::{OptimizeRunInputs, PlanRunInputs};
use crate::state::{AppState, SearchOutcome, discard_run};
use gpui::{App, Context};
use idleon_common::{Control, Progress};
use idleon_ui::runner::TaskState;
use idleon_worship::{self as worship, Run, RunResult as WorshipRunResult};

impl AppState {
    pub(crate) fn worship_plan_task(&self, cx: &App) -> TaskState<WorshipRunResult> {
        self.worship_plan
            .read(cx)
            .state(&self.worship_plan_inputs())
    }

    /// The inputs a plan run started now would use.
    pub(crate) fn worship_plan_inputs(&self) -> PlanRunInputs {
        PlanRunInputs {
            trial: self.worship.trial,
            source: self.worship.plan_source,
            seed: self.worship.seed,
            inputs: self.worship_inputs.clone(),
            plan: self.worship.plan.clone().unwrap_or_default(),
        }
    }

    /// Simulate the plan to the end off the UI thread.
    pub(crate) fn simulate_worship_plan(&mut self, cx: &mut Context<Self>) {
        self.worship_ensure_plan();
        let inputs = self.worship_plan_inputs();
        self.start_run(
            self.worship_plan.clone(),
            "Simulating the Worship TD plan…",
            inputs,
            move |inputs, report| {
                let mut run = Run::with_plan(inputs.inputs, inputs.trial, inputs.seed, inputs.plan);
                let max_ticks = run.max_ticks().max(1);
                while !run.done() {
                    run.step(500);
                    let progress = Progress {
                        fraction: (run.tick() as f64 / max_ticks as f64).min(0.99) as f32,
                        message: format!("Simulating plan · wave {}", run.wave()),
                    };
                    if report(progress) == Control::Stop {
                        return None;
                    }
                }
                Some(run.result())
            },
            cx,
        );
    }

    /// Ask a running plan simulation to stop at its next report.
    pub(crate) fn cancel_worship_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.worship_plan.clone(), "Cancelling the plan run…", cx);
    }

    /// The strategy search: running, progress, result and staleness.
    pub(crate) fn worship_optimize_task(&self, cx: &App) -> TaskState<SearchOutcome> {
        self.worship_optimize
            .read(cx)
            .state(&self.worship_optimize_inputs())
    }

    /// The inputs a search started now would use.
    pub(crate) fn worship_optimize_inputs(&self) -> OptimizeRunInputs {
        OptimizeRunInputs {
            trial: self.worship.trial,
            config: self.worship.optimize,
            inputs: self.worship_inputs.clone(),
        }
    }

    /// Search strategies off the UI thread through the shared task runner.
    pub(crate) fn optimize_worship(&mut self, cx: &mut Context<Self>) {
        self.worship_ensure_plan();
        let inputs = self.worship_optimize_inputs();
        // The plan on screen is another baseline to beat, not part of the key:
        // adopting a recommendation must not mark it stale.
        let options = inputs
            .config
            .options(inputs.trial, self.worship.plan.clone());
        self.start_run(
            self.worship_optimize.clone(),
            "Searching the Worship TD strategies…",
            inputs,
            move |inputs, report| match worship::optimize(&inputs.inputs, &options, report) {
                worship::Outcome::Found(search) => Some(Ok(*search)),
                worship::Outcome::Cancelled => None,
                worship::Outcome::Refused(reason) => Some(Err(reason.to_string())),
            },
            cx,
        );
    }

    /// Ask a running search to stop at its next report.
    pub(crate) fn cancel_worship_optimize(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.worship_optimize.clone(),
            "Cancelling the strategy search…",
            cx,
        );
    }

    /// Adopt the recommendation into the Plan pane and open it.
    pub(crate) fn worship_use_optimized_plan(&mut self, cx: &mut Context<Self>) {
        let Some(Ok(result)) = self.worship_optimize.read(cx).result().cloned() else {
            return;
        };
        let trial = self
            .worship
            .trial
            .min(worship::data().trials.len().saturating_sub(1));
        if result.trial != trial {
            // A recommendation for another trial cannot be placed here.
            self.set_status("That recommendation was searched on another trial.", cx);
            return;
        }
        let Some(trial_data) = worship::trial(trial) else {
            return;
        };
        self.worship.plan_stages = worship::plan_stages(trial_data, &result.plan);
        self.worship.plan_issues =
            worship::plan_issues(&self.worship_inputs, trial_data, &result.plan);
        self.worship.plan = Some(result.plan);
        self.worship.plan_from_optimizer = true;
        discard_run(&self.worship_plan, cx);
        self.status = format!(
            "Using the recommended plan: {} clears {:.0} waves.",
            result.label, result.score.primary
        );
        self.worship_set_tab(crate::sections::worship_td::Tab::Plan, cx);
    }
}
