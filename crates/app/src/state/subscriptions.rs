//! The two subscriptions every section repeats: redraw on a text edit, and
//! mirror a background run in the window's status line and the sidebar's
//! activity marks.

use super::AppState;
use gpui::{AppContext as _, Context, Entity, EventEmitter, Subscription};
use gpui_kit::component::input::{InputEvent, InputState};
use idleon_common::Progress;
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};

/// A text field whose edits [`redraw_on_edit`] watches: the kit's
/// `InputState`, and Ely's `TextInput` until #361 removes Ely. A section
/// moving its field to the kit changes the field's type, not this call.
pub(crate) trait TextField: EventEmitter<Self::Event> {
    type Event: 'static;

    /// Whether `event` reports a change of the field's text.
    fn is_edit(event: &Self::Event) -> bool;
}

impl TextField for InputState {
    type Event = InputEvent;

    fn is_edit(event: &InputEvent) -> bool {
        matches!(event, InputEvent::Change)
    }
}

impl TextField for ely_gpui_component::forms::TextInput {
    type Event = ely_gpui_component::forms::InputEvent;

    fn is_edit(event: &Self::Event) -> bool {
        *event == ely_gpui_component::forms::InputEvent::Changed
    }
}

/// Redraw the window on every edit of `input`, so a filter field narrows the
/// page as the user types.
pub(crate) fn redraw_on_edit<F: TextField>(
    cx: &mut Context<AppState>,
    input: &Entity<F>,
) -> Subscription {
    cx.subscribe(input, |_state, _input, event: &F::Event, cx| {
        if F::is_edit(event) {
            cx.notify();
        }
    })
}

/// What the status line says while a run reports progress.
#[derive(Clone, Copy)]
pub(crate) enum ProgressLine {
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

/// A run of `section` reported: it started, moved or ended, and the status line
/// and the activity table may have changed. It is emitted instead of a notify
/// so the run redraws only the views that show it: the sidebar's mark, the
/// status line and the owning section's page (ADR 0004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RunReported {
    pub(crate) section: &'static str,
}

impl EventEmitter<RunReported> for AppState {}

/// The status line a run whose job panicked leaves.
const FAILED: &str = "Run failed; nothing was stored.";

/// The section a run belongs to, and the status lines it reports: its
/// progress, and what a cancel left in place, depending on whether an earlier
/// result is kept.
pub(crate) struct RunStatus {
    /// The registry id of the section that owns the runner.
    pub(crate) section: &'static str,
    pub(crate) progress: ProgressLine,
    pub(crate) cancelled_kept: &'static str,
    pub(crate) cancelled_empty: &'static str,
}

/// A new runner mirrored in the status line and recorded in the window's
/// activity under its section, each event announced as [`RunReported`].
/// `finished` turns a stored result into the closing line (and may update the
/// state it belongs to, which is then saved with the session); `None` keeps
/// the current line.
pub(crate) fn report_run<R: 'static, I: 'static>(
    cx: &mut Context<AppState>,
    subscriptions: &mut Vec<Subscription>,
    status: RunStatus,
    finished: impl Fn(&mut AppState, &R) -> Option<String> + 'static,
) -> Entity<TaskRunner<R, I>> {
    debug_assert!(
        crate::sections::find(status.section).is_some(),
        "run section {} is not in the registry",
        status.section
    );
    let runner = cx.new(|_| TaskRunner::new());
    subscriptions.push(
        cx.subscribe(&runner, move |state, runner, event: &TaskEvent, cx| {
            let (section, id) = (status.section, runner.entity_id());
            let line = match event {
                TaskEvent::Started => {
                    state.activity.report(section, id, 0.0);
                    None
                }
                TaskEvent::Progress(progress) => {
                    state.activity.report(section, id, progress.fraction);
                    Some(status.progress.line(progress))
                }
                TaskEvent::Finished(ending) => {
                    let away = state.current_section().map(|open| open.id) != Some(section);
                    state.activity.finish(section, id, *ending, away);
                    state.schedule_session_save(cx);
                    ending_line(state, &runner, &status, *ending, &finished, cx)
                }
            };
            if let Some(line) = line {
                state.status = line;
            }
            cx.emit(RunReported { section });
        }),
    );
    runner
}

/// The status line a run's ending leaves, or `None` to keep the current one.
fn ending_line<R: 'static, I: 'static>(
    state: &mut AppState,
    runner: &Entity<TaskRunner<R, I>>,
    status: &RunStatus,
    ending: Ending,
    finished: &impl Fn(&mut AppState, &R) -> Option<String>,
    cx: &mut Context<AppState>,
) -> Option<String> {
    match ending {
        Ending::Cancelled => Some(
            if runner.read(cx).result().is_some() {
                status.cancelled_kept
            } else {
                status.cancelled_empty
            }
            .to_string(),
        ),
        Ending::Failed => Some(FAILED.to_string()),
        Ending::Stored => runner
            .read(cx)
            .result()
            .and_then(|result| finished(state, result)),
    }
}
