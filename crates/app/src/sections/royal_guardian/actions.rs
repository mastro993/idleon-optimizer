//! What the window does when the royal guardian section is edited or run.

use crate::sections::royal_guardian::Tab as RoyalTab;
use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// The pane the Royal Guardian strip shows.
    pub(crate) fn set_royal_tab(&mut self, tab: RoyalTab, cx: &mut Context<Self>) {
        self.royal_state.tab = tab;
        cx.notify();
    }

    /// The world the Royal Guardian tables list.
    pub(crate) fn set_royal_world(&mut self, world: usize, cx: &mut Context<Self>) {
        self.royal_state.world = world.min(idleon_royal_guardian::WORLDS - 1);
        self.royal_state.outpost = None;
        self.royal_state.resource = None;
        cx.notify();
    }

    /// The outpost whose next range target the panel explains.
    pub(crate) fn set_royal_outpost(&mut self, map: usize, cx: &mut Context<Self>) {
        self.royal_state.outpost = Some(map);
        cx.notify();
    }

    /// The resource node the resources pane explains.
    pub(crate) fn set_royal_resource(&mut self, resource: usize, cx: &mut Context<Self>) {
        self.royal_state.resource = Some(resource);
        cx.notify();
    }

    /// The currency slot the currencies pane explains.
    pub(crate) fn set_royal_currency(&mut self, slot: i64, cx: &mut Context<Self>) {
        self.royal_state.currency = Some(slot);
        cx.notify();
    }
}
