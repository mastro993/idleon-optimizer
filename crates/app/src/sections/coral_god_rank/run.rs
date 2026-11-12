//! The coral plan's run: the beam search through the shared task runner.
//!
//! The search is deterministic but budget-sized, so it runs off the UI thread
//! like every other optimizer and reports each beam step.

use crate::state::AppState;
use gpui::{App, Context};
use idleon_common::Control;
use idleon_coral::{Inputs, Plan, plan_with};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The inputs a run started now would use, or why it cannot run.
    pub(crate) fn coral_plan_inputs(&self) -> Result<Inputs, String> {
        let inputs = self.coral.inputs.clone();
        let validation = inputs.validation();
        if validation.ok() {
            Ok(inputs)
        } else {
            Err(validation.errors.join(" "))
        }
    }

    /// The run in flight, its progress and its result against the live inputs.
    pub(crate) fn coral_task(&self, cx: &App) -> TaskState<Plan> {
        self.coral_run.read(cx).state(&self.coral.inputs)
    }

    /// Run the beam search off the UI thread; a stored plan is left as it was.
    pub(crate) fn run_coral(&mut self, cx: &mut Context<Self>) {
        let inputs = match self.coral_plan_inputs() {
            Ok(inputs) => inputs,
            Err(error) => {
                self.set_status(error, cx);
                return;
            }
        };
        self.start_run(
            self.coral_run.clone(),
            "Planning Coral Kid upgrades…",
            inputs,
            |inputs, report| plan_with(&inputs, |progress| report(progress) == Control::Continue),
            cx,
        );
    }

    /// Ask the plan to stop at its next beam step.
    pub(crate) fn cancel_coral(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.coral_run.clone(), "Cancelling the coral plan…", cx);
    }
}
