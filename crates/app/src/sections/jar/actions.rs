//! What the window does when the jar section is edited or run.

use crate::sections::jar::Tab as JarTab;
use crate::state::AppState;
use gpui::Context;

impl AppState {
    pub(crate) fn set_jar_tab(&mut self, tab: JarTab, cx: &mut Context<Self>) {
        self.jar_tab = tab;
        cx.notify();
    }

    /// Switch the tier the rupie value tab projects, clamped to the six tiers.
    pub(crate) fn set_jar_tier(&mut self, tier: u32, cx: &mut Context<Self>) {
        self.jar_tier = tier.min(idleon_jar::INCOME_TIERS - 1);
        cx.notify();
    }
}
