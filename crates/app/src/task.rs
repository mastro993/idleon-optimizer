//! One background task runner for every simulation and optimizer.
//!
//! A section owns a [`TaskRunner`] entity instead of hand-rolling a thread, a
//! progress pump and a cancel flag. The runner:
//!
//! - moves the job to the background executor, so the UI thread never runs it;
//! - copies progress reports into the entity on a foreground pump;
//! - stops the job at its next report when the user cancels, keeping the result
//!   the section already showed;
//! - remembers the inputs a result came from, so a later edit marks it stale
//!   instead of silently dropping it.

use gpui::{Context, EventEmitter, Task};
use idleon_core::model::{Control, Progress};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// How often the foreground pump copies a background report into the entity.
const PUMP_INTERVAL: Duration = Duration::from_millis(40);

/// What a runner tells its owner: a new progress line, or the end of the run.
#[derive(Clone, Debug)]
pub enum TaskEvent {
    /// The run reported a new line.
    Progress(Progress),
    /// The run finished. `cancelled` is true when it stored nothing.
    Finished { cancelled: bool },
}

/// What a section renders about one run: its flags, its progress and its result.
#[derive(Clone, Debug)]
pub struct TaskState<R> {
    pub running: bool,
    pub cancelled: bool,
    pub progress: Option<Progress>,
    pub result: Option<R>,
    /// Whether `result` no longer matches the inputs the section now shows.
    pub stale: bool,
}

/// The latest line a background job reported, and whether it has finished.
struct Report {
    progress: Progress,
    done: bool,
}

/// A stored result and the inputs it was produced from.
struct Stored<R, I> {
    result: R,
    inputs: I,
}

/// A job that runs off the UI thread, reports progress and can be cancelled.
pub struct TaskRunner<R, I> {
    running: bool,
    cancelled: bool,
    progress: Option<Progress>,
    stored: Option<Stored<R, I>>,
    /// Set to stop the job at its next progress report.
    cancel: Option<Arc<AtomicBool>>,
    /// Held so the job keeps running while the runner is alive.
    task: Option<Task<()>>,
    /// Held so progress keeps reaching the entity while the job runs.
    pump: Option<Task<()>>,
}

impl<R: 'static, I: 'static> TaskRunner<R, I> {
    /// An idle runner with no result.
    pub fn new() -> Self {
        Self {
            running: false,
            cancelled: false,
            progress: None,
            stored: None,
            cancel: None,
            task: None,
            pump: None,
        }
    }

    /// The latest progress line while a run is in flight.
    pub fn progress(&self) -> Option<&Progress> {
        self.progress.as_ref()
    }

    /// Whether a run is in flight.
    pub fn running(&self) -> bool {
        self.running
    }

    /// Whether the last run was cancelled; cleared by the next run or result action.
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }

    /// The stored result, if the last finished run produced one.
    pub fn result(&self) -> Option<&R> {
        self.stored.as_ref().map(|stored| &stored.result)
    }

    /// Whether the stored result was produced from inputs other than `inputs`.
    pub fn is_stale(&self, inputs: &I) -> bool
    where
        I: PartialEq,
    {
        self.stored
            .as_ref()
            .is_some_and(|stored| &stored.inputs != inputs)
    }

    /// What a section renders: flags, progress and the result cloned against `inputs`.
    pub fn state(&self, inputs: &I) -> TaskState<R>
    where
        R: Clone,
        I: PartialEq,
    {
        TaskState {
            running: self.running,
            cancelled: self.cancelled(),
            progress: self.progress().cloned(),
            result: self.result().cloned(),
            stale: self.is_stale(inputs),
        }
    }

    /// Ask the job to stop at its next report; the previous result stays.
    ///
    /// Returns whether a run was in flight, so the owner can update its status.
    pub fn cancel(&mut self) -> bool {
        if !self.running {
            return false;
        }
        if let Some(cancel) = &self.cancel {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(progress) = &mut self.progress {
            progress.message = "Cancelling…".into();
        }
        true
    }

    /// Take the stored result, clearing it and the cancelled mark.
    pub fn take_result(&mut self) -> Option<R> {
        self.cancelled = false;
        self.stored.take().map(|stored| stored.result)
    }

    /// Drop the stored result and the cancelled mark.
    pub fn clear(&mut self) {
        self.stored = None;
        self.cancelled = false;
    }

    /// Store a finished run; `true` when it was cancelled.
    ///
    /// A cancelled run stores nothing, so the result the section already shows
    /// stays exactly as it was.
    fn settle(&mut self, outcome: Option<R>, inputs: I) -> bool {
        self.running = false;
        self.progress = None;
        self.cancel = None;
        match outcome {
            Some(result) => {
                self.cancelled = false;
                self.stored = Some(Stored { result, inputs });
                false
            }
            None => {
                self.cancelled = true;
                true
            }
        }
    }

    fn report(&mut self, progress: Progress, cx: &mut Context<Self>) {
        if !self.running {
            return;
        }
        let changed = self.progress.as_ref().is_none_or(|current| {
            current.fraction != progress.fraction || current.message != progress.message
        });
        if !changed {
            return;
        }
        self.progress = Some(progress.clone());
        cx.emit(TaskEvent::Progress(progress));
    }

    fn finish(&mut self, outcome: Option<R>, inputs: I, cx: &mut Context<Self>) {
        let cancelled = self.settle(outcome, inputs);
        cx.emit(TaskEvent::Finished { cancelled });
    }
}

impl<R, I> TaskRunner<R, I>
where
    R: Send + 'static,
    I: Clone + PartialEq + Send + 'static,
{
    /// Start `job` off the UI thread. A run already in flight is left alone.
    ///
    /// The job reports through the closure it is handed: return [`Control::Stop`]
    /// to stop the run. It stores its result with [`Some`], or returns [`None`]
    /// when it stopped.
    pub fn start(
        &mut self,
        cx: &mut Context<Self>,
        inputs: I,
        job: impl FnOnce(I, &mut dyn FnMut(Progress) -> Control) -> Option<R> + Send + 'static,
    ) -> bool {
        if self.running {
            return false;
        }
        self.running = true;
        self.cancelled = false;
        let starting = Progress {
            fraction: 0.0,
            message: "Starting…".into(),
        };
        self.progress = Some(starting.clone());
        let cancel = Arc::new(AtomicBool::new(false));
        self.cancel = Some(cancel.clone());
        let report = Arc::new(Mutex::new(Report {
            progress: starting,
            done: false,
        }));

        // The job reports from a background thread, so a foreground pump copies
        // its latest line into the entity while the work is in flight.
        let pump_report = report.clone();
        self.pump = Some(cx.spawn(async move |this, cx| {
            loop {
                let (progress, done) = {
                    let report = pump_report.lock().expect("task progress");
                    (report.progress.clone(), report.done)
                };
                this.update(cx, |runner, cx| runner.report(progress, cx))
                    .ok();
                if done {
                    break;
                }
                cx.background_executor().timer(PUMP_INTERVAL).await;
            }
        }));

        let run_inputs = inputs.clone();
        let job_report = report.clone();
        self.task = Some(cx.spawn(async move |this, cx| {
            let outcome = cx
                .background_executor()
                .spawn(async move {
                    let mut report = |progress: Progress| {
                        if cancel.load(Ordering::Relaxed) {
                            return Control::Stop;
                        }
                        job_report.lock().expect("task progress").progress = progress;
                        Control::Continue
                    };
                    let outcome = job(inputs, &mut report);
                    job_report.lock().expect("task progress").done = true;
                    outcome
                })
                .await;
            this.update(cx, |runner, cx| runner.finish(outcome, run_inputs, cx))
                .ok();
        }));
        true
    }
}

impl<R: 'static, I: 'static> EventEmitter<TaskEvent> for TaskRunner<R, I> {}

#[cfg(test)]
mod tests {
    use super::*;

    fn stored(result: u32, inputs: u32) -> TaskRunner<u32, u32> {
        let mut runner = TaskRunner::new();
        runner.stored = Some(Stored { result, inputs });
        runner
    }

    #[test]
    fn a_cancelled_run_keeps_the_previous_result_and_marks_it_cancelled() {
        let mut runner = stored(12, 24);
        assert!(runner.settle(None, 24));
        assert_eq!(runner.result(), Some(&12), "cancelling dropped the result");
        assert!(runner.cancelled());
        assert!(!runner.running());
        assert!(runner.progress().is_none());
        assert!(!runner.is_stale(&24));
        assert!(runner.is_stale(&30));
    }

    #[test]
    fn a_finished_run_replaces_the_result_and_its_inputs() {
        let mut runner = stored(12, 24);
        assert!(!runner.settle(Some(13), 30));
        assert_eq!(runner.result(), Some(&13));
        assert!(!runner.cancelled());
        assert!(!runner.is_stale(&30));
        assert!(runner.is_stale(&24));
    }

    #[test]
    fn a_result_is_stale_only_after_its_inputs_change() {
        let runner = stored(12, 24);
        assert!(!runner.is_stale(&24));
        assert!(runner.is_stale(&25));
        assert!(!TaskRunner::<u32, u32>::new().is_stale(&25));
    }

    #[test]
    fn taking_a_result_clears_it_and_the_cancelled_mark() {
        let mut runner = stored(12, 24);
        runner.cancelled = true;
        assert_eq!(runner.take_result(), Some(12));
        assert_eq!(runner.result(), None);
        assert!(!runner.cancelled());
        assert!(
            !runner.is_stale(&24),
            "a taken result has nothing to compare"
        );
    }

    #[test]
    fn clearing_drops_the_result_without_marking_cancelled() {
        let mut runner = stored(12, 24);
        runner.clear();
        assert_eq!(runner.result(), None);
        assert!(!runner.cancelled());
    }

    #[test]
    fn cancel_stops_a_running_job_and_names_the_pending_stop() {
        let mut runner = TaskRunner::<u32, u32>::new();
        runner.running = true;
        runner.progress = Some(Progress {
            fraction: 0.4,
            message: "Working…".into(),
        });
        let token = Arc::new(AtomicBool::new(false));
        runner.cancel = Some(token.clone());
        assert!(runner.cancel());
        assert!(token.load(Ordering::Relaxed));
        assert_eq!(
            runner.progress().map(|progress| progress.message.as_str()),
            Some("Cancelling…")
        );
        assert!(
            !TaskRunner::<u32, u32>::new().cancel(),
            "an idle runner ignores cancel"
        );
    }

    #[test]
    fn the_state_snapshot_reports_the_result_against_the_current_inputs() {
        let mut runner = stored(12, 24);
        runner.cancelled = true;
        let state = runner.state(&30);
        assert_eq!(state.result, Some(12));
        assert!(state.cancelled);
        assert!(state.stale);
        assert!(!state.running);
        runner.cancelled = false;
        assert!(!runner.state(&24).stale);
    }
}
