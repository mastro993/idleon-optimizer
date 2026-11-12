//! What the window does when the clamworks section is edited or run.

use crate::sections::clamworks::Tab as ClamworksTab;
use crate::state::AppState;
use gpui::{App, Context};
use idleon_clamworks::{Clamworks, Plan as ClamworksPlan, PlanInput as ClamworksPlanInput};
use idleon_ui::runner::TaskState;

impl AppState {
    pub(crate) fn set_clamworks_tab(&mut self, tab: ClamworksTab, cx: &mut Context<Self>) {
        self.clamworks_tab = tab;
        cx.notify();
    }

    pub(crate) fn set_clamworks_multikill(&mut self, multikill: f64, cx: &mut Context<Self>) {
        self.clamworks_multikill = multikill.max(0.0);
        cx.notify();
    }

    /// The Clamworks levels the section shows: the applied plan's, or the save's.
    pub(crate) fn clamworks_view(&self) -> Clamworks {
        let Some(levels) = self.clamworks_projection else {
            return self.clamworks.clone();
        };
        Clamworks {
            upgrades: levels,
            ..self.clamworks.clone()
        }
    }

    /// The inputs a Clamworks plan run started now would use.
    pub(crate) fn clamworks_plan_input(&self) -> ClamworksPlanInput {
        ClamworksPlanInput {
            budget: self.clamworks_budget,
            multikill_total: self.clamworks_multikill,
        }
    }

    /// The plan run's progress, result and staleness.
    pub(crate) fn clamworks_task(&self, cx: &App) -> TaskState<ClamworksPlan> {
        self.clamworks_run
            .read(cx)
            .state(&self.clamworks_plan_input())
    }

    /// Set the pearls the plan may spend; the saved balance until it is changed.
    pub(crate) fn set_clamworks_budget(&mut self, budget: f64, cx: &mut Context<Self>) {
        self.clamworks_budget = if budget.is_finite() {
            budget.max(0.0)
        } else {
            0.0
        };
        cx.notify();
    }

    /// Search the upgrade plans off the UI thread through the shared runner.
    pub(crate) fn run_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        let input = self.clamworks_plan_input();
        let clamworks = self.clamworks_view();
        if input.budget <= 0.0 {
            self.set_status("Clamworks plan: set a pearl budget above zero.", cx);
            return;
        }
        self.start_run(
            self.clamworks_run.clone(),
            "Planning Clamworks upgrades…",
            input,
            move |input, report| clamworks.plan_with(input, report),
            cx,
        );
    }

    /// Ask a running plan to stop at its next purchase.
    pub(crate) fn cancel_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.clamworks_run.clone(),
            "Cancelling the Clamworks plan…",
            cx,
        );
    }

    /// Preview the plan's levels across the section, as applying a layout does.
    pub(crate) fn apply_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self
            .clamworks_run
            .update(cx, |runner, _| runner.take_result())
        else {
            return;
        };
        self.clamworks_projection = Some(plan.end.upgrades);
        self.set_status(
            format!(
                "Clamworks plan applied: {} purchases, {:.2}× pearls per pearl.",
                plan.steps.len(),
                plan.pearl_value_after()
            ),
            cx,
        );
    }

    /// Drop the stored plan without previewing it.
    pub(crate) fn dismiss_clamworks_plan(&mut self, cx: &mut Context<Self>) {
        self.clamworks_run.update(cx, |runner, _| runner.clear());
        self.set_status("Clamworks plan dismissed.", cx);
    }

    /// Go back to the saved upgrade levels after previewing a plan.
    pub(crate) fn show_saved_clamworks_levels(&mut self, cx: &mut Context<Self>) {
        if self.clamworks_projection.take().is_some() {
            self.set_status("Clamworks: showing the saved upgrade levels.", cx);
        }
    }

    /// Pick the character the AFK claim and its comparison run for.
    pub(crate) fn set_clamworks_character(&mut self, character: usize, cx: &mut Context<Self>) {
        let count = self.save.as_ref().map_or(0, |save| save.character_count());
        self.clamworks_claim.set_character(character, count);
        cx.notify();
    }

    pub(crate) fn set_clamworks_phase(
        &mut self,
        phase: idleon_clamworks::afk::Phase,
        cx: &mut Context<Self>,
    ) {
        self.clamworks_claim.set_phase(phase);
        cx.notify();
    }

    /// The selected character's kills per hour, the combat simulator's stand-in.
    pub(crate) fn set_clamworks_kills_per_hour(&mut self, kills: f64, cx: &mut Context<Self>) {
        let character = self.clamworks_claim.character;
        self.clamworks_claim
            .set_stand_in(character, |stand_in| stand_in.kills_per_hour = kills);
        cx.notify();
    }

    /// The selected character's drop-rate multiplier, the same stand-in.
    pub(crate) fn set_clamworks_drop_rate(&mut self, drop_rate: f64, cx: &mut Context<Self>) {
        let character = self.clamworks_claim.character;
        self.clamworks_claim
            .set_stand_in(character, |stand_in| stand_in.drop_rate = drop_rate);
        cx.notify();
    }

    /// Read the typed hours and let the claim accept or refuse them.
    pub(crate) fn calculate_clamworks_claim(&mut self, cx: &mut Context<Self>) {
        let text = self.clamworks_claim.hours_text(cx);
        self.clamworks_claim.commit_hours(&text);
        if let Some(error) = self.clamworks_claim.error {
            self.status = format!("Clamworks claim: {}", error.message());
        } else {
            let multikill = self.clamworks_multikill;
            self.status = match self
                .clamworks_view()
                .afk_claim(self.clamworks_claim.inputs(), multikill)
            {
                Ok(claim) => format!(
                    "Clamworks claim: {} pearls, {} drop rolls.",
                    idleon_ui::format::number(claim.expected_pearls),
                    claim.rolls
                ),
                Err(error) => format!("Clamworks claim: {}", error.message()),
            };
        }
        cx.notify();
    }
}
