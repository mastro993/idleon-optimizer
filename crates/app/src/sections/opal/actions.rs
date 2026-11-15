//! What the window does when the opal plan is run, cancelled or copied; the
//! section's edits live on its [`State`](super::State).

use crate::state::AppState;
use gpui::{ClipboardItem, Context};

impl AppState {
    /// Search the distribution off the UI thread through the section's runner.
    pub(crate) fn run_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(opal) = self.opal.save.clone() else {
            self.set_status("Opal plan: import a save with villagers first.", cx);
            return;
        };
        let input = self.opal.plan_input();
        if opal.unlocked_count() == 0 {
            self.set_status("Opal plan: no villager is unlocked in this save.", cx);
            return;
        }
        if input.total <= 0.0 {
            self.set_status("Opal plan: set a total above zero.", cx);
            return;
        }
        self.start_run(
            self.opal.run.clone(),
            "Optimizing the opal distribution…",
            input,
            move |input, report| opal.plan_with(input, report),
            cx,
        );
    }

    /// Ask a running opal plan to stop at its next report.
    pub(crate) fn cancel_opal_plan(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.opal.run.clone(), "Cancelling the opal plan…", cx);
    }

    /// Copy the last finished plan as tab-separated text.
    pub(crate) fn copy_opal_plan(&mut self, cx: &mut Context<Self>) {
        let Some(plan) = self.opal.run.read(cx).result().cloned() else {
            self.set_status("Run the optimizer before copying the plan.", cx);
            return;
        };
        let Some(opal) = self.opal.save.as_ref() else {
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
