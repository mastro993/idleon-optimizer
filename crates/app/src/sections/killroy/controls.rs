//! What the section draws from but never saves: the kit number fields, the
//! long tables, and the rows derived from the save, the settings and the run.
//!
//! The persisted settings stay in [`super::State`]. The fields are built once
//! here and edit it only through `AppState`'s setters (`docs/ui-kit/forms.md`);
//! the derived rows are rebuilt by [`AppState::sync_killroy`] when what they
//! read changes, so a frame only draws them.

use super::forecast_view::{WeekRow, week_rows};
use super::table::{
    self, BuffRow, QueueRow, ResultRow, Rows, buff_columns, buff_rows, queue_columns, queue_rows,
    result_columns, result_rows,
};
use crate::state::AppState;
use gpui::{Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::table::TableState;
use idleon_killroy::{Plan, RowSummary, UpgradeChoice};
use idleon_ui::widgets::{number_state, number_value};
use std::sync::Arc;

/// An `AppState` setter a number field writes through.
type Setter = fn(&mut AppState, f64, &mut Context<AppState>);

/// The last finished run, as the comparison card draws it.
pub(crate) struct RunView {
    /// The run's rows; their identity tells a new result from the one drawn.
    rows: Arc<[RowSummary]>,
    pub(crate) comparing: bool,
    pub(crate) trials: usize,
    pub(crate) details: String,
    pub(crate) warnings: Vec<String>,
}

/// The section's live controls and render caches.
pub(crate) struct Controls {
    pub(crate) weeks: Entity<InputState>,
    pub(crate) trials: Entity<InputState>,
    pub(crate) kills: Entity<InputState>,
    pub(crate) seed: Entity<InputState>,
    pub(crate) queue: Entity<TableState<Rows<QueueRow>>>,
    pub(crate) results: Entity<TableState<Rows<ResultRow>>>,
    pub(crate) buffs: Entity<TableState<Rows<BuffRow>>>,
    /// The forecast's week rows, for `forecast_key`.
    pub(crate) weeks_rows: Vec<WeekRow>,
    forecast_key: Option<(u64, usize)>,
    /// The plan a run started now would execute, or why it cannot run.
    pub(crate) plan: Result<Plan, String>,
    /// The save revision and settings `plan` and the buff rows were built from.
    plan_key: Option<(u64, super::State)>,
    /// The save's `+1` list, for `upgrades_key`.
    pub(crate) upgrades: Vec<UpgradeChoice>,
    upgrades_key: Option<u64>,
    /// How many saved-buff rows the selected characters start with.
    pub(crate) buff_count: usize,
    pub(crate) run: Option<RunView>,
    /// Whether the stored result came from inputs other than `plan`.
    pub(crate) stale: bool,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let weeks = number_state(1.0, 52.0, 1.0, window, cx);
        let trials = number_state(1.0, 20.0, 1.0, window, cx);
        let kills = number_state(0.0, 100.0, 1.0, window, cx);
        let seed = number_state(1.0, 999_999.0, 1.0, window, cx);
        let setters: [(&Entity<InputState>, Setter); 4] = [
            (&weeks, AppState::set_killroy_weeks),
            (&trials, AppState::set_killroy_trials),
            (&kills, AppState::set_killroy_kills_per_second),
            (&seed, AppState::set_killroy_seed),
        ];
        // Edits flow field -> setting, through the section's setters.
        let subscriptions = setters
            .into_iter()
            .map(|(field, set)| {
                cx.subscribe(field, move |state, field, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change)
                        && let Some(value) = number_value(field.read(cx))
                    {
                        set(state, value, cx);
                    }
                })
            })
            .collect();
        Self {
            queue: table::state(queue_columns(), window, cx),
            results: table::state(result_columns(), window, cx),
            buffs: table::state(buff_columns(), window, cx),
            weeks,
            trials,
            kills,
            seed,
            weeks_rows: Vec::new(),
            forecast_key: None,
            plan: Err(String::new()),
            plan_key: None,
            upgrades: Vec::new(),
            upgrades_key: None,
            buff_count: 0,
            run: None,
            stale: false,
            _subscriptions: subscriptions,
        }
    }
}

impl AppState {
    /// Bring the section's derived rows in line with the save, the settings
    /// and the last run. Each part rebuilds only when what it reads changed,
    /// so the call is a few comparisons on most frames.
    pub(crate) fn sync_killroy(&mut self, cx: &mut Context<Self>) {
        self.ensure_killroy_forecast();
        if self.killroy_controls.forecast_key != self.killroy_forecast_key {
            self.killroy_controls.weeks_rows = match &self.killroy_forecast {
                Some(Ok(forecast)) => week_rows(forecast),
                _ => Vec::new(),
            };
            self.killroy_controls.forecast_key = self.killroy_forecast_key;
        }

        let revision = self.save_revision;
        let planned = self
            .killroy_controls
            .plan_key
            .as_ref()
            .is_some_and(|(at, settings)| *at == revision && *settings == self.killroy);
        if !planned {
            let plan = self.killroy_plan();
            table::set_rows(
                &self.killroy_controls.queue,
                plan.as_ref().map(queue_rows).unwrap_or_default(),
                cx,
            );
            let characters = match &self.save {
                Some(save) => {
                    let mut cis: Vec<usize> = self
                        .killroy
                        .selection
                        .iter()
                        .map(|selection| selection.ci)
                        .collect();
                    cis.sort_unstable();
                    cis.dedup();
                    idleon_killroy::character_buffs(save, &cis, self.killroy.keep_buffs)
                }
                None => Vec::new(),
            };
            let buffs = buff_rows(&characters);
            let controls = &mut self.killroy_controls;
            controls.buff_count = buffs.len();
            table::set_rows(&controls.buffs, buffs, cx);
            if controls.upgrades_key != Some(revision) {
                controls.upgrades = self
                    .save
                    .as_ref()
                    .map(idleon_killroy::upgrade_choices)
                    .unwrap_or_default();
                controls.upgrades_key = Some(revision);
            }
            controls.plan = plan;
            controls.plan_key = Some((revision, self.killroy.clone()));
        }

        let runner = self.killroy_run.read(cx);
        let rows = runner.result().map(|outcome| &outcome.rows);
        let drawn = self.killroy_controls.run.as_ref().map(|run| &run.rows);
        let fresh = match (rows, drawn) {
            (Some(rows), Some(drawn)) => !Arc::ptr_eq(rows, drawn),
            (None, None) => false,
            _ => true,
        };
        if !planned || fresh {
            self.killroy_controls.stale = match &self.killroy_controls.plan {
                Ok(plan) => runner.is_stale(plan),
                Err(_) => runner.result().is_some(),
            };
        }
        if fresh {
            let run = runner.result().map(|outcome| RunView {
                rows: outcome.rows.clone(),
                comparing: outcome.rows.iter().any(RowSummary::comparing),
                trials: outcome.rows.iter().map(|row| row.runs).max().unwrap_or(0),
                details: super::comparison::run_details(&outcome.plan),
                warnings: outcome.plan.warnings.clone(),
            });
            let rows = run
                .as_ref()
                .map(|run| result_rows(&run.rows))
                .unwrap_or_default();
            table::set_rows(&self.killroy_controls.results, rows, cx);
            self.killroy_controls.run = run;
        }
    }
}
