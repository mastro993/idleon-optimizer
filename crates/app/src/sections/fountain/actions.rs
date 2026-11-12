//! What the window does when the fountain section is edited or run.

use crate::sections;
use crate::state::AppState;
use gpui::Context;
use idleon_fountain::Fountain;

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
}
