//! The coral plan's run: the beam search through the shared task runner.
//!
//! The search is deterministic but budget-sized, so it runs off the UI thread
//! like every other optimizer and reports each beam step.

use crate::state::AppState;
use crate::task::TaskState;
use gpui::{App, Context};
use idleon_core::coral::{Inputs, Plan, plan_with};
use idleon_core::model::Control;

impl AppState {
    /// The inputs a run started now would use, or why it cannot run.
    pub fn coral_plan_inputs(&self) -> Result<Inputs, String> {
        let inputs = self.coral.inputs.clone();
        let validation = inputs.validation();
        if validation.ok() {
            Ok(inputs)
        } else {
            Err(validation.errors.join(" "))
        }
    }

    /// The run in flight, its progress and its result against the live inputs.
    pub fn coral_task(&self, cx: &App) -> TaskState<Plan> {
        self.coral_run.read(cx).state(&self.coral.inputs)
    }

    /// Run the beam search off the UI thread; a stored plan is left as it was.
    pub fn run_coral(&mut self, cx: &mut Context<Self>) {
        let inputs = match self.coral_plan_inputs() {
            Ok(inputs) => inputs,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        let started = self.coral_run.update(cx, |runner, cx| {
            runner.start(cx, inputs, |inputs, report| {
                plan_with(&inputs, |progress| report(progress) == Control::Continue)
            })
        });
        if started {
            self.status = "Planning Coral Kid upgrades…".into();
            cx.notify();
        }
    }

    /// Ask the plan to stop at its next beam step.
    pub fn cancel_coral(&mut self, cx: &mut Context<Self>) {
        if self.coral_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the coral plan…".into();
            cx.notify();
        }
    }
}
