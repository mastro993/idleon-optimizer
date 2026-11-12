//! What the window does when the fishing section is edited or run.

use crate::state::AppState;
use gpui::Context;
use idleon_fishing::AccountInputs;

impl AppState {
    /// Reset the fishing toolkit and location to the selected character's save.
    pub(crate) fn fishing_reset(&mut self, cx: &mut Context<Self>) {
        self.fishing.character_defaults(&self.fishing_save);
        cx.notify();
    }

    /// Select a fishing character and take that character's saved toolkit/spot.
    pub(crate) fn set_fishing_character(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.character = index.min(self.fishing_save.characters.len().saturating_sub(1));
        self.fishing.character_defaults(&self.fishing_save);
        cx.notify();
    }

    /// Select a lure index, clamped to the upstream list.
    pub(crate) fn set_fishing_lure(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.lure = index.min(idleon_fishing::data().lures.len().saturating_sub(1));
        cx.notify();
    }

    /// Select a line index, clamped to the upstream list.
    pub(crate) fn set_fishing_line(&mut self, index: usize, cx: &mut Context<Self>) {
        self.fishing.line = index.min(idleon_fishing::data().lines.len().saturating_sub(1));
        cx.notify();
    }

    /// Apply a ranked lure/line pair from the all-combination table.
    pub(crate) fn use_fishing_toolkit(&mut self, lure: usize, line: usize, cx: &mut Context<Self>) {
        self.fishing.use_toolkit(lure, line);
        cx.notify();
    }

    /// Switch the lure/line ranking between Fishing EXP and fish quantity.
    pub(crate) fn set_fishing_objective(
        &mut self,
        objective: idleon_fishing::Objective,
        cx: &mut Context<Self>,
    ) {
        self.fishing.objective = objective;
        cx.notify();
    }

    /// Switch the selected pool by its location id.
    pub(crate) fn set_fishing_location(&mut self, id: String, cx: &mut Context<Self>) {
        if idleon_fishing::data().location(&id).is_some() {
            self.fishing.location = id;
            cx.notify();
        }
    }

    /// Set the AFK claim length in hours.
    pub(crate) fn set_fishing_hours(&mut self, hours: f64, cx: &mut Context<Self>) {
        self.fishing.hours = if hours.is_finite() {
            hours.clamp(0.04, 10_000.0)
        } else {
            1.0
        };
        cx.notify();
    }

    /// Set how many Trench depths the projection covers.
    pub(crate) fn set_fishing_trench_depths(&mut self, depths: usize, cx: &mut Context<Self>) {
        self.fishing.trench_depths = depths.clamp(1, 25);
        cx.notify();
    }

    /// Switch how the other Trench characters choose their lure and line.
    pub(crate) fn set_fishing_helper_mode(
        &mut self,
        mode: idleon_fishing::HelperMode,
        cx: &mut Context<Self>,
    ) {
        self.fishing.trench_helper_mode = mode;
        cx.notify();
    }

    /// Tick or untick another character on the Trench helper table.
    pub(crate) fn set_fishing_helper(&mut self, index: usize, on: bool, cx: &mut Context<Self>) {
        let characters = self.fishing_save.characters.len();
        self.fishing.set_helper(index, on, characters);
        cx.notify();
    }

    /// Edit one account value the fishing evaluator reads.
    pub(crate) fn set_fishing_account(
        &mut self,
        change: impl FnOnce(&mut AccountInputs),
        cx: &mut Context<Self>,
    ) {
        change(&mut self.fishing.account);
        self.fishing.clamp(&self.fishing_save);
        cx.notify();
    }
}
