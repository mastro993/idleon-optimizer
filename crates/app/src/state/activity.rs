//! What the sidebar shows about each section's runs: running with its
//! progress, or finished or failed while another place was open (ADR 0002).
//!
//! `report_run` records every runner event here under the section that owns
//! the runner; the sidebar only reads it.

use gpui::EntityId;
use idleon_ui::runner::Ending;
use std::collections::HashMap;

/// How a section's last run ended while another place was open, kept until the
/// section is opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Mark {
    /// A run stored a result.
    Finished,
    /// A run panicked.
    Failed,
}

/// What a section's sidebar item shows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Shown {
    /// `runs` in flight; `fraction` is the least advanced one's, 0 to 1.
    Running {
        runs: usize,
        fraction: f32,
    },
    Mark(Mark),
}

impl Shown {
    /// The tooltip and accessible text: "Running · 42%", "2 runs · 42%",
    /// "Finished" or "Failed".
    pub(crate) fn label(self) -> String {
        match self {
            Self::Running { runs, fraction } => {
                let percent = format!("{:.0}%", fraction * 100.0);
                if runs == 1 {
                    format!("Running · {percent}")
                } else {
                    format!("{runs} runs · {percent}")
                }
            }
            Self::Mark(Mark::Finished) => "Finished".into(),
            Self::Mark(Mark::Failed) => "Failed".into(),
        }
    }
}

/// One section's runs in flight, by runner, and its mark.
#[derive(Default)]
struct Runs {
    running: HashMap<EntityId, f32>,
    mark: Option<Mark>,
}

/// Every section's activity, keyed by registry id. It lives only as long as
/// the window.
#[derive(Default)]
pub(crate) struct Activity {
    sections: HashMap<&'static str, Runs>,
}

impl Activity {
    /// `runner` of `section` is in flight at `fraction`. A new run replaces
    /// the section's mark with the ring.
    pub(crate) fn report(&mut self, section: &'static str, runner: EntityId, fraction: f32) {
        let runs = self.sections.entry(section).or_default();
        runs.mark = None;
        let fraction = if fraction.is_nan() {
            0.0
        } else {
            fraction.clamp(0.0, 1.0)
        };
        runs.running.insert(runner, fraction);
    }

    /// `runner` of `section` ended. `away` says whether another place was
    /// open: only then does a stored or failed run leave a mark. A cancelled
    /// run never does.
    pub(crate) fn finish(
        &mut self,
        section: &'static str,
        runner: EntityId,
        ending: Ending,
        away: bool,
    ) {
        let runs = self.sections.entry(section).or_default();
        runs.running.remove(&runner);
        let mark = match ending {
            Ending::Stored => Mark::Finished,
            Ending::Failed => Mark::Failed,
            Ending::Cancelled => return,
        };
        if away {
            runs.mark = Some(mark);
        }
    }

    /// The user opened `section`: its mark has been seen.
    pub(crate) fn open(&mut self, section: &'static str) {
        if let Some(runs) = self.sections.get_mut(section) {
            runs.mark = None;
        }
    }

    /// Drop every mark: after an import the results they point to may be gone.
    pub(crate) fn clear_marks(&mut self) {
        for runs in self.sections.values_mut() {
            runs.mark = None;
        }
    }

    /// What `section`'s item shows: the ring while any run is in flight, else
    /// its mark.
    pub(crate) fn shown(&self, section: &str) -> Option<Shown> {
        let runs = self.sections.get(section)?;
        let least = runs.running.values().copied().reduce(f32::min);
        match least {
            Some(fraction) => Some(Shown::Running {
                runs: runs.running.len(),
                fraction,
            }),
            None => runs.mark.map(Shown::Mark),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FARMING: &str = "farming";

    fn runner(id: u64) -> EntityId {
        EntityId::from(id)
    }

    #[test]
    fn a_section_with_no_runs_shows_nothing() {
        assert_eq!(Activity::default().shown(FARMING), None);
    }

    #[test]
    fn the_ring_follows_the_least_advanced_run_and_counts_the_runs() {
        let mut activity = Activity::default();
        activity.report(FARMING, runner(1), 0.8);
        assert_eq!(
            activity.shown(FARMING).map(Shown::label).as_deref(),
            Some("Running · 80%")
        );
        activity.report(FARMING, runner(2), 0.42);
        assert_eq!(
            activity.shown(FARMING),
            Some(Shown::Running {
                runs: 2,
                fraction: 0.42
            })
        );
        assert_eq!(
            activity.shown(FARMING).map(Shown::label).as_deref(),
            Some("2 runs · 42%")
        );
        activity.finish(FARMING, runner(2), Ending::Cancelled, true);
        assert_eq!(
            activity.shown(FARMING).map(Shown::label).as_deref(),
            Some("Running · 80%")
        );
    }

    #[test]
    fn a_run_ending_away_leaves_its_mark_and_one_ending_in_view_leaves_none() {
        let cases = [
            (Ending::Stored, true, Some(Mark::Finished)),
            (Ending::Failed, true, Some(Mark::Failed)),
            (Ending::Cancelled, true, None),
            (Ending::Stored, false, None),
            (Ending::Failed, false, None),
        ];
        for (ending, away, mark) in cases {
            let mut activity = Activity::default();
            activity.report(FARMING, runner(1), 0.5);
            activity.finish(FARMING, runner(1), ending, away);
            assert_eq!(
                activity.shown(FARMING),
                mark.map(Shown::Mark),
                "{ending:?}, away: {away}"
            );
        }
        assert_eq!(Shown::Mark(Mark::Finished).label(), "Finished");
        assert_eq!(Shown::Mark(Mark::Failed).label(), "Failed");
    }

    #[test]
    fn opening_the_section_clears_its_mark_only() {
        let mut activity = Activity::default();
        activity.report(FARMING, runner(1), 0.5);
        activity.finish(FARMING, runner(1), Ending::Stored, true);
        activity.report("sailing", runner(2), 0.5);
        activity.finish("sailing", runner(2), Ending::Failed, true);
        activity.open(FARMING);
        assert_eq!(activity.shown(FARMING), None);
        assert_eq!(activity.shown("sailing"), Some(Shown::Mark(Mark::Failed)));
    }

    #[test]
    fn an_import_clears_every_mark_and_keeps_the_runs_in_flight() {
        let mut activity = Activity::default();
        activity.report(FARMING, runner(1), 0.5);
        activity.finish(FARMING, runner(1), Ending::Stored, true);
        activity.report("sailing", runner(2), 0.25);
        activity.clear_marks();
        assert_eq!(activity.shown(FARMING), None);
        assert_eq!(
            activity.shown("sailing"),
            Some(Shown::Running {
                runs: 1,
                fraction: 0.25
            })
        );
    }

    #[test]
    fn a_new_run_replaces_the_dot_with_the_ring() {
        let mut activity = Activity::default();
        activity.report(FARMING, runner(1), 1.0);
        activity.finish(FARMING, runner(1), Ending::Stored, true);
        activity.report(FARMING, runner(1), 0.0);
        assert_eq!(
            activity.shown(FARMING).map(Shown::label).as_deref(),
            Some("Running · 0%")
        );
        // Cancelling the new run does not bring the old dot back.
        activity.finish(FARMING, runner(1), Ending::Cancelled, true);
        assert_eq!(activity.shown(FARMING), None);
    }
}
