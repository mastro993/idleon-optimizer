//! What the window does when the fountain section is edited or run.

use crate::sections;
use crate::sections::fountain::State;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_fountain::{Fountain, OptimizerPlan};
use idleon_ui::runner::TaskState;

impl AppState {
    /// The desired currency the fountain currency tab simulates.
    pub(crate) fn fountain_desired(&self) -> usize {
        self.fountain_state
            .desired
            .or_else(|| self.fountain.as_ref().map(Fountain::desired_currency))
            .unwrap_or(0)
    }

    pub(crate) fn set_fountain_tab(
        &mut self,
        tab: sections::fountain::Tab,
        cx: &mut Context<Self>,
    ) {
        self.fountain_state.tab = tab;
        cx.notify();
    }

    pub(crate) fn set_fountain_water(&mut self, water: usize, cx: &mut Context<Self>) {
        self.fountain_state.water = water.min(idleon_fountain::WATERS - 1);
        cx.notify();
    }

    /// Simulate a desired currency; `None` returns to the save's choice.
    pub(crate) fn set_fountain_desired(&mut self, desired: Option<usize>, cx: &mut Context<Self>) {
        self.fountain_state.desired =
            desired.filter(|currency| *currency < idleon_fountain::CURRENCIES);
        cx.notify();
    }

    /// Pick the upgrade the upgrade tab's cost curve plots.
    pub(crate) fn set_fountain_upgrade(&mut self, upgrade: Option<usize>, cx: &mut Context<Self>) {
        self.fountain_state.upgrade = upgrade;
        cx.notify();
    }

    /// Edit the planner's or optimizer's selections and redraw.
    pub(crate) fn edit_fountain_state(
        &mut self,
        cx: &mut Context<Self>,
        edit: impl FnOnce(&mut State),
    ) {
        edit(&mut self.fountain_state);
        cx.notify();
    }

    /// The plan run's progress, result and staleness for the current controls.
    pub(crate) fn fountain_plan_task(&self, cx: &App) -> TaskState<OptimizerPlan> {
        self.fountain_plan
            .read(cx)
            .state(&self.fountain_state.plan_request())
    }

    /// Optimize the fountain upgrades off the UI thread through the shared runner.
    pub(crate) fn run_fountain_plan(&mut self, cx: &mut Context<Self>) {
        let Some(fountain) = self.fountain.clone() else {
            self.set_status("Fountain plan: import a save first.", cx);
            return;
        };
        let request = self.fountain_state.plan_request();
        self.start_run(
            self.fountain_plan.clone(),
            "Planning fountain upgrades…",
            request,
            move |request, report| fountain.optimize_with(request, report),
            cx,
        );
    }

    /// Ask a running plan to stop at its next purchase.
    pub(crate) fn cancel_fountain_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.fountain_plan.clone(),
            "Cancelling the fountain plan…",
            cx,
        );
    }
}
