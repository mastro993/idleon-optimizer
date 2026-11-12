//! The Killroy settings' actions and the trial run through the shared runner.

use super::Mode;
use crate::state::AppState;
use gpui::{App, ClipboardItem, Context};
use idleon_common::Control;
use idleon_common::Progress;
use idleon_killroy::{self as killroy, Comparisons, Plan, PlanInput, RowSummary, Selection};
use idleon_ui::runner::TaskState;

/// What a finished run stores: the plan it ran and one summary row per
/// configuration. The plan rides along so the results export writes the inputs
/// and saved buffs the rows were produced from, not a later edit's.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct KillroyOutcome {
    pub(crate) plan: Plan,
    pub(crate) rows: Vec<RowSummary>,
}

impl AppState {
    /// The inputs a run started now would use.
    pub(crate) fn killroy_plan_inputs(&self) -> PlanInput {
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
            keep_buffs: state.keep_buffs,
            unlimited_inventory: state.unlimited_inventory,
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
    pub(crate) fn killroy_plan(&self) -> Result<Plan, String> {
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
    pub(crate) fn killroy_task(&self, cx: &App) -> TaskState<KillroyOutcome> {
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
    pub(crate) fn run_killroy(&mut self, cx: &mut Context<Self>) {
        let plan = match self.killroy_plan() {
            Ok(plan) => plan,
            Err(error) => {
                self.set_status(error, cx);
                return;
            }
        };
        let attempts = plan.attempts();
        self.start_run(
            self.killroy_run.clone(),
            format!("Running {attempts} Killroy attempts…"),
            plan,
            move |plan, report| {
                let rows = killroy::run(&plan, |done, total| {
                    let progress = Progress {
                        fraction: (done as f32 / total.max(1) as f32).min(0.99),
                        message: format!("{done} of {total} attempts"),
                    };
                    report(progress) == Control::Stop
                })?;
                Some(KillroyOutcome { plan, rows })
            },
            cx,
        );
    }

    /// Ask the run to stop at its next attempt.
    pub(crate) fn cancel_killroy(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(self.killroy_run.clone(), "Cancelling the Killroy run…", cx);
    }

    /// Copy the last finished run's results document, the section's JSON export.
    pub(crate) fn copy_killroy_results(&mut self, cx: &mut Context<Self>) {
        let Some(outcome) = self.killroy_run.read(cx).result().cloned() else {
            self.set_status("Run the trials before exporting the results.", cx);
            return;
        };
        let json = killroy::results_json(&outcome.plan, &outcome.rows);
        cx.write_to_clipboard(ClipboardItem::new_string(json.clone()));
        self.set_status(
            format!(
                "Copied the Killroy results JSON to the clipboard ({} bytes).",
                json.len()
            ),
            cx,
        );
    }

    /// The trials a new save, or the section's first render, starts with.
    pub(crate) fn killroy_default_selection(&mut self) {
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
    pub(crate) fn set_killroy_mode(&mut self, mode: Mode, cx: &mut Context<Self>) {
        if self.killroy.mode == mode {
            return;
        }
        self.killroy.mode = mode;
        cx.notify();
    }

    /// Set the forecast's horizon, dropping checks it no longer covers.
    pub(crate) fn set_killroy_weeks(&mut self, value: f64, cx: &mut Context<Self>) {
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
    pub(crate) fn set_killroy_trials(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.trials = if value.is_finite() {
            value.round().clamp(1.0, 20.0) as usize
        } else {
            3
        };
        cx.notify();
    }

    /// Set the base seed the trial seeds step from.
    pub(crate) fn set_killroy_seed(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.seed = if value.is_finite() {
            (value.round().max(1.0) as u64).min(999_999)
        } else {
            1
        };
        cx.notify();
    }

    /// Set the stand-in kill rate the trials run at.
    pub(crate) fn set_killroy_kills_per_second(&mut self, value: f64, cx: &mut Context<Self>) {
        self.killroy.kills_per_second = if value.is_finite() {
            value.clamp(0.0, 100.0)
        } else {
            3.0
        };
        cx.notify();
    }

    /// Keep the save's buff timers instead of starting from a fresh map entry.
    pub(crate) fn set_killroy_keep_buffs(&mut self, keep: bool, cx: &mut Context<Self>) {
        self.killroy.keep_buffs = keep;
        cx.notify();
    }

    /// Lift the bag-space limit on the run's pickups.
    pub(crate) fn set_killroy_unlimited_inventory(
        &mut self,
        unlimited: bool,
        cx: &mut Context<Self>,
    ) {
        self.killroy.unlimited_inventory = unlimited;
        cx.notify();
    }

    /// Check or uncheck a week.
    pub(crate) fn toggle_killroy_week(
        &mut self,
        week_offset: usize,
        on: bool,
        cx: &mut Context<Self>,
    ) {
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
    pub(crate) fn toggle_killroy_selection(
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
    pub(crate) fn toggle_killroy_upgrade(&mut self, id: usize, on: bool, cx: &mut Context<Self>) {
        self.killroy.upgrades.retain(|other| *other != id);
        if on {
            self.killroy.upgrades.push(id);
            self.killroy.upgrades.sort_unstable();
        }
        cx.notify();
    }
}
