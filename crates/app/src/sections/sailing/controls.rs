//! The section's kit form controls, the tables its tabs fill and what each tab
//! reads from the fleet, built once.
//!
//! The persisted values stay on [`State`](super::State) and the route context
//! (`docs/section-state.md`). The controls only show them: edits flow control →
//! value through the subscriptions made here, and values flow back in render
//! through `show`/`show_number` (`docs/ui-kit/forms.md`). Each tab reads the
//! fleet once per [`Key`] ([`cached`]) and a stored simulation becomes rows when
//! it lands, so a frame that only redraws formats nothing
//! (`docs/ui-kit/data.md`).

use super::simulation::{self, RunInputs};
use super::{artifacts, breakdowns, captains, fleet, odds, replacements, ships, table};
use crate::state::AppState;
use gpui::{App, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_sailing::{
    AssignmentMode, Goal, MAX_HOURS, MAX_TRIALS, MIN_HOURS, MIN_TRIALS, RouteContext, SimResult,
    island_artifact_count, island_distance,
};
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::rc::Rc;

/// What every tab's figures depend on: the save, the shared fleet controls and
/// the route context (minimum tier, Unending Loot Search, …).
#[derive(Clone, PartialEq)]
pub(crate) struct Key {
    revision: u64,
    island: usize,
    goal: Goal,
    mode: AssignmentMode,
    context: RouteContext,
}

impl AppState {
    /// The inputs the sailing tabs read now.
    pub(crate) fn sailing_key(&self) -> Key {
        Key {
            revision: self.save_revision,
            island: self.sailing_state.island,
            goal: self.sailing_state.goal,
            mode: self.sailing_state.mode,
            context: self.sailing.clone(),
        }
    }
}

/// A value read from the fleet, and the key it was read for.
pub(crate) type Cached<K, V> = Option<(K, Rc<V>)>;

/// The value in `slot` when it was read for `key`, else `build`'s, kept.
pub(crate) fn cached<K: PartialEq, V>(
    state: &mut AppState,
    slot: fn(&mut Controls) -> &mut Cached<K, V>,
    key: K,
    cx: &mut App,
    build: impl FnOnce(&AppState, &mut App) -> V,
) -> Rc<V> {
    if let Some((at, value)) = slot(&mut state.sailing_controls)
        && *at == key
    {
        return value.clone();
    }
    let value = Rc::new(build(state, cx));
    *slot(&mut state.sailing_controls) = Some((key, value.clone()));
    value
}

/// The section's kit controls, tab tables and per-tab caches.
pub(crate) struct Controls {
    pub(crate) island: ChoiceSelect<usize>,
    pub(crate) minimum_tier: ChoiceSelect<u32>,
    pub(crate) breakdown_boat: ChoiceSelect<usize>,
    pub(crate) sim_island: ChoiceSelect<usize>,
    pub(crate) sim_hours: Entity<InputState>,
    pub(crate) sim_trials: Entity<InputState>,
    pub(crate) unending_loot_search: Entity<InputState>,
    /// The tables whose rows grow with the fleet.
    pub(crate) fleet_table: Entity<TableState<table::Rows<fleet::Row>>>,
    pub(crate) ships_table: Entity<TableState<table::Rows<ships::Row>>>,
    pub(crate) captains_table: Entity<TableState<table::Rows<captains::Row>>>,
    pub(crate) replacements_table: Entity<TableState<table::Rows<replacements::Row>>>,
    pub(crate) odds_boats_table: Entity<TableState<table::Rows<odds::BoatRow>>>,
    /// The destinations the save can reach, as both island selects list them.
    pub(crate) islands: Cached<usize, Vec<Choice<usize>>>,
    pub(crate) fleet: Cached<Key, fleet::View>,
    pub(crate) ships: Cached<(Key, bool), ships::View>,
    pub(crate) captains: Cached<Key, captains::View>,
    pub(crate) replacements: Cached<Key, replacements::View>,
    pub(crate) odds: Cached<Key, odds::View>,
    pub(crate) artifacts: Cached<(Key, usize), Option<artifacts::View>>,
    pub(crate) breakdowns: Cached<(Key, usize), Option<breakdowns::View>>,
    /// The stored simulation, formatted once when it lands.
    pub(crate) sim: Option<Rc<simulation::View>>,
    _subscriptions: Vec<Subscription>,
}

/// The minimum chest tiers the filter offers.
pub(crate) fn tier_choices() -> Vec<Choice<u32>> {
    (0..5)
        .map(|tier| Choice::new(tier, format!("Tier {tier}+")))
        .collect()
}

/// Every island the save can reach, with its distance and artifact count.
pub(crate) fn island_choices(available: usize) -> Vec<Choice<usize>> {
    (0..available)
        .map(|island| {
            Choice::new(
                island,
                format!(
                    "Island {} · {} dist · {} artifacts",
                    island + 1,
                    island_distance(island),
                    island_artifact_count(island)
                ),
            )
        })
        .collect()
}

impl Controls {
    pub(crate) fn new(
        sim: &Entity<TaskRunner<SimResult, RunInputs>>,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) -> Self {
        let controls = Self {
            island: ChoiceSelect::new(island_choices(1), Some(&0), window, cx),
            minimum_tier: ChoiceSelect::new(tier_choices(), None, window, cx),
            breakdown_boat: ChoiceSelect::new(Vec::new(), None, window, cx),
            sim_island: ChoiceSelect::new(island_choices(1), Some(&0), window, cx),
            sim_hours: number_state(MIN_HOURS, MAX_HOURS, 0.25, window, cx),
            sim_trials: number_state(MIN_TRIALS as f64, MAX_TRIALS as f64, 100.0, window, cx),
            unending_loot_search: number_state(0.0, 100.0, 1.0, window, cx),
            fleet_table: table::state(fleet::columns(), window, cx),
            ships_table: table::state(ships::columns(), window, cx),
            captains_table: table::state(captains::columns(), window, cx),
            replacements_table: table::state(replacements::columns(), window, cx),
            odds_boats_table: table::state(odds::boat_columns(), window, cx),
            islands: None,
            fleet: None,
            ships: None,
            captains: None,
            replacements: None,
            odds: None,
            artifacts: None,
            breakdowns: None,
            sim: None,
            _subscriptions: Vec::new(),
        };
        // Edits flow control -> value, through the section's setters.
        let subscriptions = vec![
            on_pick(cx, &controls.island, |state, island, cx| {
                state.set_fleet_island(island, cx);
            }),
            on_pick(cx, &controls.minimum_tier, |state, tier, cx| {
                state.set_fleet_minimum_tier(tier, cx);
            }),
            on_pick(cx, &controls.breakdown_boat, |state, boat, cx| {
                state.set_sailing_breakdown_boat(boat, cx);
            }),
            on_pick(cx, &controls.sim_island, |state, island, cx| {
                state.set_sailing_sim_island(island, cx);
            }),
            on_number(cx, &controls.sim_hours, |state, hours, cx| {
                state.set_sailing_sim_hours(hours, cx);
            }),
            on_number(cx, &controls.sim_trials, |state, trials, cx| {
                state.set_sailing_sim_trials(trials.round().max(0.0) as usize, cx);
            }),
            on_number(cx, &controls.unending_loot_search, |state, percent, cx| {
                state.set_unending_loot_search(percent, cx);
            }),
            // A stored run is formatted once, when it lands.
            cx.subscribe(sim, |state, runner, event: &TaskEvent, cx| {
                if matches!(event, TaskEvent::Finished(Ending::Stored))
                    && let Some(result) = runner.read(cx).result()
                {
                    state.sailing_controls.sim = Some(Rc::new(simulation::View::of(result)));
                    cx.notify();
                }
            }),
        ];
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

/// Call `edit` with each number a field is changed to.
fn on_number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    edit: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            edit(state, value, cx);
        }
    })
}
