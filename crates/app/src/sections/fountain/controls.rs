//! The section's kit form controls, the tables its runs fill and what each
//! tab reads from the cavern, built once.
//!
//! The persisted selections stay on [`State`](super::State)
//! (`docs/section-state.md`). The controls only show them: edits flow control →
//! selection through the subscriptions made here, and selections flow back in
//! render through `show`/`show_number` (`docs/ui-kit/forms.md`). The rankings,
//! currency tables and upgrade rows are read from the cavern when the save or a
//! selection changes ([`cached`]), and a stored run becomes rows when it lands,
//! so a frame that only redraws reads what is already there
//! (`docs/ui-kit/data.md`).

use super::planner;
use super::sim::{TimelineRow, timeline_rows, timeline_table};
use super::state::{SIM_MAX_HOURS, SIM_MIN_HOURS, STEP_CHOICES};
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_fountain::{
    CoinStack, CrossBonus, CurrencyRow, DesiredRow, Goal, Income, LevelStep, MarblePlan,
    MarbleRequest, MarbleRow, OptimizerPlan, PlanRequest, PlannerRow, SimResult, UnlockRow,
    UpgradeRow, WaterMultiplier,
};
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{Choice, ChoiceSelect, RowsDelegate, number_state, number_value};
use std::rc::Rc;

/// The cards of the Overview tab that read lists from the cavern.
pub(crate) struct Overview {
    pub(crate) income: Income,
    pub(crate) levels: Vec<LevelStep>,
    pub(crate) stacks: Vec<CoinStack>,
    pub(crate) cross: Vec<CrossBonus>,
}

/// The Currency tab's tables at one desired currency.
pub(crate) struct Currency {
    pub(crate) values: Vec<CurrencyRow>,
    pub(crate) waters: Vec<WaterMultiplier>,
    pub(crate) desired: Vec<DesiredRow>,
    pub(crate) unlocks: Vec<UnlockRow>,
}

/// The Upgrades tab's rows for one water, and the picked upgrade's curve.
pub(crate) struct Upgrades {
    pub(crate) rows: Vec<UpgradeRow>,
    /// `log10` of each of the next levels' costs.
    pub(crate) curve: Vec<f64>,
}

/// A value read from the cavern, and the key it was read for.
type Cached<K, V> = Option<(K, Rc<V>)>;

/// The value in `slot` when it was read for `key`, else `build`'s, kept.
pub(crate) fn cached<K: PartialEq, V>(
    slot: &mut Cached<K, V>,
    key: K,
    build: impl FnOnce() -> V,
) -> Rc<V> {
    match slot {
        Some((at, value)) if *at == key => value.clone(),
        _ => {
            let value = Rc::new(build());
            *slot = Some((key, value.clone()));
            value
        }
    }
}

/// The section's kit controls, run tables and per-tab caches.
pub(crate) struct Controls {
    pub(crate) currency_desired: ChoiceSelect<Option<usize>>,
    pub(crate) curve_upgrade: ChoiceSelect<usize>,
    pub(crate) planner_goal: ChoiceSelect<Goal>,
    pub(crate) planner_desired: ChoiceSelect<Option<usize>>,
    pub(crate) optimizer_goal: ChoiceSelect<Goal>,
    pub(crate) optimizer_steps: ChoiceSelect<usize>,
    pub(crate) marble_goal: ChoiceSelect<Goal>,
    pub(crate) marble_desired: ChoiceSelect<Option<usize>>,
    pub(crate) marble_optimizer_goal: ChoiceSelect<Goal>,
    pub(crate) marble_optimizer_steps: ChoiceSelect<usize>,
    pub(crate) sim_hours: Entity<InputState>,
    /// The stored runs, kept once for render instead of cloned per frame.
    pub(crate) plan: Option<Rc<OptimizerPlan>>,
    pub(crate) marble_plan: Option<Rc<MarblePlan>>,
    pub(crate) sim: Option<Rc<SimResult>>,
    /// The optimizer's steps and the simulation's timeline, which can run to
    /// hundreds of rows.
    pub(crate) plan_steps: Entity<TableState<RowsDelegate<planner::StepRow>>>,
    pub(crate) timeline: Entity<TableState<RowsDelegate<TimelineRow>>>,
    /// How many rows `timeline` holds.
    pub(crate) timeline_len: usize,
    pub(crate) overview: Cached<u64, Overview>,
    pub(crate) currency: Cached<(u64, usize), Currency>,
    pub(crate) upgrades: Cached<(u64, usize, usize), Upgrades>,
    pub(crate) planner: Cached<(u64, PlanRequest), Vec<PlannerRow>>,
    pub(crate) marble: Cached<(u64, MarbleRequest), Vec<MarbleRow>>,
    /// The save revision the multiplier tree was last given.
    pub(crate) tree_revision: Option<u64>,
    _subscriptions: Vec<Subscription>,
}

/// The goal list every goal select offers.
pub(crate) fn goal_choices() -> Vec<Choice<Goal>> {
    Goal::ALL.map(|goal| Choice::new(goal, goal.label())).into()
}

/// The step caps the optimizers offer.
pub(crate) fn step_choices() -> Vec<Choice<usize>> {
    STEP_CHOICES
        .map(|count| Choice::new(count, count.to_string()))
        .into()
}

/// The section's runners, whose stored results the tabs draw.
pub(crate) struct Runs<'a> {
    pub(crate) plan: &'a Entity<TaskRunner<OptimizerPlan, PlanRequest>>,
    pub(crate) marble: &'a Entity<TaskRunner<MarblePlan, super::MarblePlanInputs>>,
    pub(crate) sim: &'a Entity<TaskRunner<SimResult, super::SimInputs>>,
}

impl Controls {
    pub(crate) fn new(runs: Runs, window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let goal = |window: &mut Window, cx: &mut Context<AppState>| {
            ChoiceSelect::new(goal_choices(), Some(&Goal::default()), window, cx)
        };
        let steps = |window: &mut Window, cx: &mut Context<AppState>| {
            ChoiceSelect::new(step_choices(), Some(&50), window, cx)
        };
        let controls = Self {
            currency_desired: ChoiceSelect::new(Vec::new(), None, window, cx),
            curve_upgrade: ChoiceSelect::new(Vec::new(), None, window, cx),
            planner_goal: goal(window, cx),
            planner_desired: ChoiceSelect::new(Vec::new(), None, window, cx),
            optimizer_goal: goal(window, cx),
            optimizer_steps: steps(window, cx),
            marble_goal: goal(window, cx),
            marble_desired: ChoiceSelect::new(Vec::new(), None, window, cx),
            marble_optimizer_goal: goal(window, cx),
            marble_optimizer_steps: steps(window, cx),
            sim_hours: number_state(SIM_MIN_HOURS, SIM_MAX_HOURS, 1.0, window, cx),
            plan: None,
            marble_plan: None,
            sim: None,
            plan_steps: cx.new(|cx| TableState::new(planner::step_table(), window, cx)),
            timeline: cx.new(|cx| TableState::new(timeline_table(), window, cx)),
            timeline_len: 0,
            overview: None,
            currency: None,
            upgrades: None,
            planner: None,
            marble: None,
            tree_revision: None,
            _subscriptions: Vec::new(),
        };
        // Edits flow control -> selection, through the section's setters.
        let mut subscriptions = vec![
            on_pick(cx, &controls.currency_desired, |state, desired, cx| {
                state.set_fountain_desired(desired, cx);
            }),
            on_pick(cx, &controls.curve_upgrade, |state, upgrade, cx| {
                state.set_fountain_upgrade(Some(upgrade), cx);
            }),
            on_pick(cx, &controls.planner_goal, |state, goal, cx| {
                state.edit_fountain_state(cx, |state| state.planner_goal = goal);
            }),
            on_pick(cx, &controls.planner_desired, |state, desired, cx| {
                state.edit_fountain_state(cx, |state| state.planner_desired = desired);
            }),
            on_pick(cx, &controls.optimizer_goal, |state, goal, cx| {
                state.edit_fountain_state(cx, |state| state.optimizer_goal = goal);
            }),
            on_pick(cx, &controls.optimizer_steps, |state, steps, cx| {
                state.edit_fountain_state(cx, |state| state.optimizer_steps = steps);
            }),
            on_pick(cx, &controls.marble_goal, |state, goal, cx| {
                state.edit_fountain_state(cx, |state| state.marble_goal = goal);
            }),
            on_pick(cx, &controls.marble_desired, |state, desired, cx| {
                state.edit_fountain_state(cx, |state| state.marble_desired = desired);
            }),
            on_pick(cx, &controls.marble_optimizer_goal, |state, goal, cx| {
                state.edit_fountain_state(cx, |state| state.marble_optimizer_goal = goal);
            }),
            on_pick(cx, &controls.marble_optimizer_steps, |state, steps, cx| {
                state.edit_fountain_state(cx, |state| state.marble_optimizer_steps = steps);
            }),
            cx.subscribe(&controls.sim_hours, |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(hours) = number_value(field.read(cx))
                {
                    state.set_fountain_sim_hours(hours, cx);
                }
            }),
        ];
        // A stored run is kept once, and its long tables get their rows here.
        subscriptions.push(on_stored(cx, runs.plan, |state, plan, cx| {
            let rows = planner::step_rows(&plan);
            state.fountain_controls.plan = Some(Rc::new(plan));
            state.fountain_controls.plan_steps.update(cx, |table, cx| {
                table.delegate_mut().set_rows(rows);
                cx.notify();
            });
        }));
        subscriptions.push(on_stored(cx, runs.marble, |state, plan, _| {
            state.fountain_controls.marble_plan = Some(Rc::new(plan));
        }));
        subscriptions.push(on_stored(cx, runs.sim, |state, result, cx| {
            let rows = timeline_rows(&result);
            state.fountain_controls.timeline_len = rows.len();
            state.fountain_controls.sim = Some(Rc::new(result));
            state.fountain_controls.timeline.update(cx, |table, cx| {
                table.delegate_mut().set_rows(rows);
                cx.notify();
            });
        }));
        Self {
            _subscriptions: subscriptions,
            ..controls
        }
    }
}

/// Call `edit` with the value a select confirms.
fn on_pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    edit: impl Fn(&mut AppState, V, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            let SelectEvent::Confirm(Some(value)) = event else {
                return;
            };
            edit(state, value.clone(), cx);
        },
    )
}

/// Call `keep` with a run's result once the runner stores it.
fn on_stored<R: Clone + 'static, I: 'static>(
    cx: &mut Context<AppState>,
    runner: &Entity<TaskRunner<R, I>>,
    keep: impl Fn(&mut AppState, R, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(runner, move |state, runner, event: &TaskEvent, cx| {
        if matches!(event, TaskEvent::Finished(Ending::Stored))
            && let Some(result) = runner.read(cx).result().cloned()
        {
            keep(state, result, cx);
        }
    })
}
