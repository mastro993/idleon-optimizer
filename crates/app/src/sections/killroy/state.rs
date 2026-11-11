//! The Killroy settings' actions and the trial run through the shared runner.

use super::Mode;
use crate::state::AppState;
use crate::task::TaskState;
use gpui::{App, Context};
use idleon_core::killroy::{self, Comparisons, Plan, PlanInput, RowSummary, Selection};
use idleon_core::model::{Control, Progress};

/// What the section's runner stores: one summary row per configuration.
pub type KillroyOutcome = Vec<RowSummary>;

impl AppState {
    /// The inputs a run started now would use.
    pub fn killroy_plan_inputs(&self) -> PlanInput {
        let state = &self.killroy;
        let comparisons = match state.mode {
            Mode::Baseline => Comparisons::Off,
            Mode::Upgrades => Comparisons::Only(state.upgrades.clone()),
        };
        PlanInput {
            selections: self.killroy_selections(),
            seeds: killroy::trial_seeds(state.seed, state.trials),
            comparisons,
            kills_per_second: state.kills_per_second,
        }
    }

    /// The checked encounters, limited to the checked weeks the forecast covers.
    fn killroy_selections(&self) -> Vec<Selection> {
        let state = &self.killroy;
        state
            .selection
            .iter()
            .copied()
            .filter(|selection| {
                selection.week_offset <= state.weeks && state.week_checked(selection.week_offset)
            })
            .collect()
    }

    /// The plan a run started now would execute, or why it cannot run.
    pub fn killroy_plan(&self) -> Result<Plan, String> {
        let save = self
            .save
            .as_ref()
            .ok_or_else(|| "Import a save to run Killroy".to_string())?;
        if self.killroy.checked_weeks.is_empty() {
            return Err("Check at least one week to simulate".into());
        }
        if self.killroy.mode == Mode::Upgrades && self.killroy.upgrades.is_empty() {
            return Err("Choose at least one upgrade to compare".into());
        }
        let input = self.killroy_plan_inputs();
        killroy::plan(save, &input).map_err(|error| error.to_string())
    }

    /// The run in flight, its progress and its result.
    ///
    /// The stored result is stale once the plan a run would execute now differs;
    /// a plan that cannot build at all (no checked week, no upgrade) is stale too.
    pub fn killroy_task(&self, cx: &App) -> TaskState<KillroyOutcome> {
        let runner = self.killroy_run.read(cx);
        let stale = match self.killroy_plan() {
            Ok(plan) => runner.is_stale(&plan),
            Err(_) => runner.result().is_some(),
        };
        TaskState {
            running: runner.running(),
            cancelled: runner.cancelled(),
            progress: runner.progress().cloned(),
            result: runner.result().cloned(),
            stale,
        }
    }

    /// Run the trials off the UI thread through the shared runner.
    pub fn run_killroy(&mut self, cx: &mut Context<Self>) {
        let plan = match self.killroy_plan() {
            Ok(plan) => plan,
            Err(error) => {
                self.status = error;
                cx.notify();
                return;
            }
        };
        let attempts = plan.attempts();
        let started = self.killroy_run.update(cx, |runner, cx| {
            runner.start(cx, plan, move |plan, report| {
                killroy::run(&plan, |done, total| {
                    let progress = Progress {
                        fraction: (done as f32 / total.max(1) as f32).min(0.99),
                        message: format!("{done} of {total} attempts"),
                    };
                    report(progress) == Control::Stop
                })
            })
        });
        if started {
            self.status = format!("Running {attempts} Killroy attempts…");
            cx.notify();
        }
    }

    /// Ask the run to stop at its next attempt.
    pub fn cancel_killroy(&mut self, cx: &mut Context<Self>) {
        if self.killroy_run.update(cx, |runner, _| runner.cancel()) {
            self.status = "Cancelling the Killroy run…".into();
            cx.notify();
        }
    }

    /// The trials a new save, or the section's first render, starts with.
    pub fn killroy_default_selection(&mut self) {
        let Some(save) = self.save.as_ref() else {
            return;
        };
        let checked = self.killroy.checked_weeks.clone();
        let Ok(forecast) = killroy::forecast(save, self.killroy.weeks) else {
            return;
        };
        self.killroy.selection = forecast
            .weeks
            .iter()
            .filter(|week| checked.contains(&week.week_offset))
            .flat_map(|week| {
                week.characters.iter().flat_map(|character| {
                    character
                        .encounters
                        .iter()
                        .filter(|encounter| encounter.eligible)
                        .map(|encounter| Selection {
                            week_offset: week.week_offset,
                            ci: character.ci,
                            slot: encounter.slot,
                        })
                })
            })
            .collect();
    }

    /// Switch the section's tab.
    pub fn set_killroy_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.killroy.mode == mode {
            return;
        }
        self.killroy.mode = mode;
        cx.notify();
    }

    /// Set the forecast's horizon, dropping checks it no longer covers.
    pub fn set_killroy_weeks(&mut self, value: f64, cx: &mut Context<Self>) {
        let weeks = if value.is_finite() {
            value.round().clamp(1.0, 52.0) as usize
        } else {
            12
        };
        self.killroy.weeks = weeks;
        self.killroy.checked_weeks.retain(|week| *week <= weeks);
        if self.killroy.checked_weeks.is_empty() {
            self.killroy.checked_weeks.push(0);
        }
        self.killroy
            .selection
            .retain(|selection| selection.week_offset <= weeks);
        cx.notify();
    }

    /// Set the trials a run averages.
    pub fn set_killroy_trials(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.trials = if value.is_finite() {
            value.round().clamp(1.0, 20.0) as usize
        } else {
            3
        };
        cx.notify();
    }

    /// Set the base seed the trial seeds step from.
    pub fn set_killroy_seed(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.seed = if value.is_finite() {
            (value.round().max(1.0) as u64).min(999_999)
        } else {
            1
        };
        cx.notify();
    }

    /// Set the stand-in kill rate the trials run at.
    pub fn set_killroy_kills_per_second(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.kills_per_second = if value.is_finite() {
            value.clamp(0.0, 100.0)
        } else {
            3.0
        };
        cx.notify();
    }

    /// Check or uncheck a week.
    pub fn toggle_killroy_week(&mut self, week_offset: usize, on: bool, cx: &mut Context<Self>) {
        self.killroy
            .checked_weeks
            .retain(|week| *week != week_offset);
        if on {
            self.killroy.checked_weeks.push(week_offset);
            self.killroy.checked_weeks.sort_unstable();
        }
        if self.killroy.checked_weeks.is_empty() {
            self.killroy.checked_weeks.push(0);
        }
        cx.notify();
    }

    /// Check or uncheck one encounter.
    pub fn toggle_killroy_selection(
        &mut self,
        selection: Selection,
        on: bool,
        cx: &mut Context<Self>,
    ) {
        self.killroy.selection.retain(|other| *other != selection);
        if on {
            self.killroy.selection.push(selection);
            self.killroy.selection.sort_unstable();
            // Selecting an encounter queues its week, as the upstream list does.
            if !self.killroy.checked_weeks.contains(&selection.week_offset) {
                self.killroy.checked_weeks.push(selection.week_offset);
                self.killroy.checked_weeks.sort_unstable();
            }
        }
        cx.notify();
    }

    /// Check or uncheck a `+1` comparison.
    pub fn toggle_killroy_upgrade(&mut self, id: usize, on: bool, cx: &mut Context<Self>) {
        self.killroy.upgrades.retain(|other| *other != id);
        if on {
            self.killroy.upgrades.push(id);
            self.killroy.upgrades.sort_unstable();
        }
        cx.notify();
    }
}
