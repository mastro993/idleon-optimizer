//! What the window does when the farming section is edited or run.

use crate::sections::farming::{
    CompareInputs as FarmingCompareInputs, RunInputs as FarmingRunInputs,
};
use crate::state::AppState;
use gpui::{App, Context, Window};
use idleon_farming::{
    Allocation as FarmingOutcome, Comparison as FarmingComparison, Mode, Objective,
};
use idleon_ui::runner::TaskState;

impl AppState {
    /// Select a plot; the farming section keeps it across dashboard visits.
    pub(crate) fn select_plot(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.farming.select(index);
        self.status = format!("Plot {} selected.", self.farming.selected_plot + 1);
        window.focus(&self.focus, cx);
        cx.notify();
    }

    /// Switch the farming tab; the section keeps it across dashboard visits.
    pub(crate) fn set_farming_tab(
        &mut self,
        tab: crate::sections::farming::Tab,
        cx: &mut Context<Self>,
    ) {
        self.farming.set_tab(tab);
        cx.notify();
    }

    /// Aim the later evolution features at a crop id, clamped to the families.
    pub(crate) fn set_target_crop(&mut self, value: f64, cx: &mut Context<Self>) {
        self.farming.set_target_crop(value);
        self.farming.adopt(&self.farm);
        cx.notify();
    }

    /// The inputs a Land Rank allocation started now would use.
    pub(crate) fn farming_inputs(&self) -> FarmingRunInputs {
        FarmingRunInputs {
            save_revision: self.save_revision,
            objective: self.farming.objective,
            mode: self.farming.mode,
            target_crop: self.farming.target_crop,
        }
    }

    /// The allocation run: running, progress, result and staleness.
    pub(crate) fn farming_task(&self, cx: &App) -> TaskState<FarmingOutcome> {
        self.farming_run.read(cx).state(&self.farming_inputs())
    }

    /// Plan the assignment and the allocation for another objective.
    pub(crate) fn set_farming_objective(&mut self, objective: Objective, cx: &mut Context<Self>) {
        self.farming.set_objective(objective);
        self.farming.adopt(&self.farm);
        self.set_status(format!("Farming objective: {}.", objective.label()), cx);
    }

    /// Keep the saved Land Rank levels, or reallocate every point.
    pub(crate) fn set_farming_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        self.farming.set_mode(mode);
        self.set_status(format!("Land Rank allocation: {}.", mode.label()), cx);
    }

    /// Spend the account's Land Rank points off the UI thread.
    pub(crate) fn run_farming(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.set_status("Import a save to allocate Land Rank.", cx);
            return;
        }
        let inputs = self.farming_inputs();
        let farm = self.farm.clone();
        let (objective, mode, target) = (
            self.farming.objective,
            self.farming.mode,
            Some(self.farming.target_crop),
        );
        self.start_run(
            self.farming_run.clone(),
            "Allocating Land Rank…",
            inputs,
            move |_inputs, report| farm.allocation(objective, mode, target, report),
            cx,
        );
    }

    /// Ask a running allocation to stop; the previous result stays.
    pub(crate) fn cancel_farming(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.farming_run.clone(),
            "Cancelling the Land Rank allocation…",
            cx,
        );
    }

    /// The inputs a preset comparison started now would use.
    pub(crate) fn farming_compare_inputs(&self) -> FarmingCompareInputs {
        FarmingCompareInputs {
            save_revision: self.save_revision,
            mode: self.farming.mode,
            target_crop: self.farming.target_crop,
        }
    }

    /// The comparison run: running, progress, result and staleness.
    pub(crate) fn farming_compare_task(&self, cx: &App) -> TaskState<FarmingComparison> {
        self.farming_compare_run
            .read(cx)
            .state(&self.farming_compare_inputs())
    }

    /// Plan every upstream goal from the current baseline off the UI thread.
    pub(crate) fn compare_farming(&mut self, cx: &mut Context<Self>) {
        if !self.save_loaded {
            self.set_status("Import a save to compare the farming preset goals.", cx);
            return;
        }
        let inputs = self.farming_compare_inputs();
        let farm = self.farm.clone();
        let (mode, target) = (self.farming.mode, Some(self.farming.target_crop));
        self.start_run(
            self.farming_compare_run.clone(),
            "Comparing farming preset goals…",
            inputs,
            move |_inputs, report| farm.comparison(mode, target, report),
            cx,
        );
    }

    /// Ask a running comparison to stop; the previous table stays.
    pub(crate) fn cancel_farming_compare(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.farming_compare_run.clone(),
            "Cancelling the preset comparison…",
            cx,
        );
    }

    /// Save the preset bar's name against the section's goal, baseline and crop.
    pub(crate) fn save_farming_preset(&mut self, cx: &mut Context<Self>) {
        let name = self.farming_name.read(cx).text().trim().to_string();
        match self.farming.save_preset(&name) {
            Ok(message) => {
                self.farming_name
                    .update(cx, |input, cx| input.set_text("", cx));
                self.status = message;
            }
            Err(message) => self.status = message,
        }
        cx.notify();
    }

    /// Apply a saved farming preset to the section's inputs.
    pub(crate) fn apply_farming_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(name) = self.farming.apply_preset(index) {
            self.farming.adopt(&self.farm);
            self.status = format!("Applied the farming preset “{name}”.");
        }
        cx.notify();
    }

    /// Forget one saved farming preset.
    pub(crate) fn delete_farming_preset(&mut self, index: usize, cx: &mut Context<Self>) {
        if let Some(preset) = self.farming.delete_preset(index) {
            self.status = format!("Deleted the farming preset “{}”.", preset.name);
        }
        cx.notify();
    }
}
