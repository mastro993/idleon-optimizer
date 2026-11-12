//! The two subscriptions every section repeats: redraw on a text edit, and
//! mirror a background run in the window's status line.

use super::AppState;
use ely_gpui_component::forms::{InputEvent, TextInput};
use gpui::{AppContext as _, Context, Entity, Subscription};
use idleon_common::Progress;
use idleon_ui::runner::{TaskEvent, TaskRunner};

/// Redraw the window on every edit of `input`, so a filter field narrows the
/// page as the user types.
pub(crate) fn redraw_on_edit(
    cx: &mut Context<AppState>,
    input: &Entity<TextInput>,
) -> Subscription {
    cx.subscribe(input, |_state, _input, event: &InputEvent, cx| {
        if *event == InputEvent::Changed {
            cx.notify();
        }
    })
}

/// What the status line says while a run reports progress.
#[derive(Clone, Copy)]
pub(super) enum ProgressLine {
    /// `"<label> 42%"` from the report's fraction.
    Percent(&'static str),
    /// `"<label> <message>"` from the report's own line.
    Message(&'static str),
}

impl ProgressLine {
    fn line(self, progress: &Progress) -> String {
        match self {
            Self::Percent(label) => format!("{label} {:.0}%", progress.fraction * 100.0),
            Self::Message(label) => format!("{label} {}", progress.message),
        }
    }
}

/// The status lines one run reports: its progress, and what a cancel left in
/// place, depending on whether an earlier result is kept.
pub(super) struct RunStatus {
    pub(crate) progress: ProgressLine,
    pub(crate) cancelled_kept: &'static str,
    pub(crate) cancelled_empty: &'static str,
}

/// A new runner mirrored in the status line. `finished` turns a stored result
/// into the closing line (and may update the state it belongs to); `None`
/// keeps the current line.
pub(super) fn report_run<R: 'static, I: 'static>(
    cx: &mut Context<AppState>,
    subscriptions: &mut Vec<Subscription>,
    status: RunStatus,
    finished: impl Fn(&mut AppState, &R) -> Option<String> + 'static,
) -> Entity<TaskRunner<R, I>> {
    let runner = cx.new(|_| TaskRunner::new());
    subscriptions.push(
        cx.subscribe(&runner, move |state, runner, event: &TaskEvent, cx| {
            let line = match event {
                TaskEvent::Progress(progress) => Some(status.progress.line(progress)),
                TaskEvent::Finished { cancelled: true } => Some(
                    if runner.read(cx).result().is_some() {
                        status.cancelled_kept
                    } else {
                        status.cancelled_empty
                    }
                    .to_string(),
                ),
                TaskEvent::Finished { cancelled: false } => runner
                    .read(cx)
                    .result()
                    .and_then(|result| finished(state, result)),
            };
            if let Some(line) = line {
                state.status = line;
            }
            cx.notify();
        }),
    );
    runner
}
