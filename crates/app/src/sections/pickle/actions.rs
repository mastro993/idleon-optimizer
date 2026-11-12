//! What the window does when the pickle section is edited or run.

use crate::state::AppState;
use gpui::Context;

impl AppState {
    /// The damage per second the pickle table solves for.
    pub(crate) fn set_pickle_dps(&mut self, dps: f64, cx: &mut Context<Self>) {
        self.pickle = super::State { dps, ..self.pickle }.clamped();
        cx.notify();
    }

    /// The kill-time budget the pickle table solves inside.
    pub(crate) fn set_pickle_ttk(&mut self, seconds: f64, cx: &mut Context<Self>) {
        self.pickle = super::State {
            max_ttk: seconds,
            ..self.pickle
        }
        .clamped();
        cx.notify();
    }
}
