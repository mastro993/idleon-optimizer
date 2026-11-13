//! What the window does when the opal section is edited or run.

use crate::sections::opal::Tab as OpalTab;
use crate::state::AppState;
use gpui::{App, ClipboardItem, Context};
use idleon_opal::{Opal, Plan as OpalPlan, PlanInput as OpalPlanInput, PlanTarget as OpalTarget};
use idleon_ui::runner::TaskState;

impl AppState {
    /// Switch the opal breakdown between the total and one villager.
    ///
    /// A locked villager (or no save) falls back to the total, so a stale
    /// selection never points at a villager this save does not have.
    pub(crate) fn set_opal_villager(&mut self, villager: Option<usize>, cx: &mut Context<Self>) {
        let selected = villager.filter(|villager| {
            self.opal
                .as_ref()
                .is_some_and(|opal| opal.unlocked(*villager))
        });
        if self.opal_selected != selected {
            self.opal_selected = selected;
            cx.notify();
        }
    }

    /// Switch the Opal Optimizer between the total and one villager.
    pub(crate) fn set_opal_target(&mut self, villager: Option<usize>, cx: &mut Context<Self>) {
        let target = villager.filter(|villager| {
            self.opal
                .as_ref()
                .is_some_and(|opal| opal.unlocked(*villager))
        });
        if self.opal_target != target {
            self.opal_target = target;
            cx.notify();
        }
    }

    /// Switch the Opal Optimizer's tab.
    pub(crate) fn set_opal_tab(&mut self, tab: OpalTab, cx: &mut Context<Self>) {
        if self.opal_tab != tab {
            self.opal_tab = tab;
            cx.notify();
        }
    }

    /// Set the opal total the optimizer splits; whole opals only.
    pub(crate) fn set_opal_total(&mut self, total: f64, cx: &mut Context<Self>) {
        let total = idleon_opal::whole(total);
        if self.opal_total != total {
            self.opal_total = total;
            cx.notify();
        }
    }

    /// Set one villager's manual opal count; whole opals only.
    pub(crate) fn set_opal_manual(&mut self, villager: usize, opals: f64, cx: &mut Context<Self>) {
        let Some(count) = self.opal_manual.get_mut(villager) else {
            return;
        };
        let opals = idleon_opal::whole(opals);
        if *count != opals {
            *count = opals;
            cx.notify();
        }
    }

    /// Put the manual simulator back on the save's own opal split.
    pub(crate) fn reset_opal_manual(&mut self, cx: &mut Context<Self>) {
        let saved = self
            .opal
            .as_ref()
            .map_or([0.0; idleon_opal::VILLAGERS], Opal::opals);
        if self.opal_manual != saved {
            self.opal_manual = saved;
            cx.notify();
        }
    }

    /// Put the optimizer back on the save's own opal total.
    pub(crate) fn use_saved_opal_total(&mut self, cx: &mut Context<Self>) {
        let total = self.opal.as_ref().map_or(0.0, Opal::total_opals);
        self.set_opal_total(total, cx);
    }

    /// The plan's inputs: the selected target, the total and the save's split.
    pub(crate) fn opal_plan_input(&self) -> OpalPlanInput {
        let current = self
            .opal
            .as_ref()
            .map_or([0.0; idleon_opal::VILLAGERS], Opal::opals);
        let target = match self.opal_target {
            Some(villager)
                if self
                    .opal
                    .as_ref()
                    .is_some_and(|opal| opal.unlocked(villager)) =>
            {
                OpalTarget::Villager(villager)
            }
            _ => OpalTarget::Total,
        };
        OpalPlanInput {
            target,
            total: self.opal_total,
            current,
        }
    }

    /// The plan run's progress, result and staleness.
    pub(crate) fn opal_task(&self, cx: &App) -> TaskState<OpalPlan> {
        self.opal_run.read(cx).state(&self.opal_plan_input())
    }

    /// Search the distribution off the UI thread through the shared runner.
    pub(crate) fn run_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(opal) = self.opal.clone() else {
            self.set_status("Opal plan: import a save with villagers first.", cx);
            return;
        };
        let input = self.opal_plan_input();
        if opal.unlocked_count() == 0 {
            self.set_status("Opal plan: no villager is unlocked in this save.", cx);
            return;
        }
        if input.total <= 0.0 {
            self.set_status("Opal plan: set a total above zero.", cx);
            return;
        }
        self.start_run(
            self.opal_run.clone(),
            "Optimizing the opal distribution…",
            input,
            move |input, report| opal.plan_with(input, report),
            cx,
        );
    }

    /// Ask a running opal plan to stop at its next report.
    pub(crate) fn cancel_opal_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.opal_run.clone(), "Cancelling the opal plan…", cx);
    }

    /// Copy the last finished plan as tab-separated text.
    pub(crate) fn copy_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.opal_run.read(cx).result().cloned() else {
            self.set_status("Run the optimizer before copying the plan.", cx);
            return;
        };
        let Some(opal) = self.opal.as_ref() else {
            self.set_status("Opal plan: import a save first.", cx);
            return;
        };
        let text = plan.text(opal);
        cx.write_to_clipboard(ClipboardItem::new_string(text.clone()));
        self.set_status(
            format!(
                "Copied the opal plan to the clipboard ({} bytes).",
                text.len()
            ),
            cx,
        );
    }
}
