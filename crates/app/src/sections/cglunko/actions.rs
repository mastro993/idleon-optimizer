//! What the window does when the cglunko section is edited or run.

use crate::sections::cglunko::Tab as CglunkoTab;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_cglunko::{Page as CglunkoPage, Plan as CglunkoPlan};
use idleon_ui::runner::TaskState;

impl AppState {
    pub(crate) fn set_cglunko_tab(&mut self, tab: CglunkoTab, cx: &mut Context<Self>) {
        self.cglunko_tab = tab;
        cx.notify();
    }

    /// Pick the page the upgrade list and the plan work on. A stored plan is
    /// marked stale by the runner, not dropped, so the previous numbers stay
    /// readable until the next run.
    pub(crate) fn set_cglunko_page(&mut self, page: CglunkoPage, cx: &mut Context<Self>) {
        self.cglunko_page = page;
        cx.notify();
    }

    /// The plan run's progress, result and staleness for the selected page.
    pub(crate) fn cglunko_task(&self, cx: &App) -> TaskState<CglunkoPlan> {
        self.cglunko_run.read(cx).state(&self.cglunko_page)
    }

    /// Plan the selected page's purchases off the UI thread through the shared
    /// runner.
    pub(crate) fn run_cglunko_plan(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.set_status("Cglunko plan: import a save first.", cx);
            return;
        }
        let page = self.cglunko_page;
        let cglunko = self.cglunko.clone();
        self.start_run(
            self.cglunko_run.clone(),
            format!("Planning Cglunko {} upgrades…", page.label()),
            page,
            move |page, report| cglunko.plan_with(page, report),
            cx,
        );
    }

    /// Ask a running plan to stop at its next purchase.
    pub(crate) fn cancel_cglunko_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.cglunko_run.clone(), "Cancelling the Cglunko plan…", cx);
    }
}
