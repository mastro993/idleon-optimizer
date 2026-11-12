//! Starting, cancelling and dropping a section's background run, with the
//! status line each one leaves.

use super::AppState;
use gpui::{App, Context, Entity};
use idleon_common::{Control, Progress};
use idleon_ui::runner::TaskRunner;

impl AppState {
    /// Show `line` in the status bar and redraw.
    pub(crate) fn set_status(&mut self, line: impl Into<String>, cx: &mut Context<Self>) {
        self.status = line.into();
        cx.notify();
    }

    /// Start `job` off the UI thread and say `line` once it starts; a run
    /// already in flight is left alone.
    pub(crate) fn start_run<R, I>(
        &mut self,
        runner: Entity<TaskRunner<R, I>>,
        line: impl Into<String>,
        inputs: I,
        job: impl FnOnce(I, &mut dyn FnMut(Progress) -> Control) -> Option<R> + Send + 'static,
        cx: &mut Context<Self>,
    ) where
        R: Send + 'static,
        I: Clone + PartialEq + Send + 'static,
    {
        if runner.update(cx, |runner, cx| runner.start(cx, inputs, job)) {
            self.set_status(line, cx);
        }
    }

    /// Ask a run to stop at its next report; when one was in flight, say
    /// `line`. The previous result stays.
    pub(crate) fn cancel_run<R: 'static, I: 'static>(
        &mut self,
        runner: Entity<TaskRunner<R, I>>,
        line: &str,
        cx: &mut Context<Self>,
    ) {
        if runner.update(cx, |runner, _| runner.cancel()) {
            self.set_status(line, cx);
        }
    }
}

/// Stop a run and drop its stored result: the inputs it answered are gone.
pub(crate) fn discard_run<R: 'static, I: 'static>(runner: &Entity<TaskRunner<R, I>>, cx: &mut App) {
    runner.update(cx, |runner, _| {
        runner.cancel();
        runner.clear();
    });
}
