//! What the section draws from but never saves: the kit controls, the run
//! requests its panes compare against, and the engine results they read.
//!
//! The persisted values stay in [`super::State`]. The controls are built once
//! here and edit it only through `AppState`'s setters (`docs/ui-kit/forms.md`).
//! [`AppState::sync_royal`] drops the cached results when a new save loads and
//! rebuilds a run request only when its inputs move, so a frame formats rows
//! instead of calling the engine.

use super::upgrades::View as UpgradeView;
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use idleon_royal_guardian::{
    self as royal_guardian, Allocation, Banked, Breakdown, Category, Counters, CurrencyIncome,
    Endpoint, Objective, Request, RoyalGuardian, ShelfRequest, Target, UnitExpRequest,
};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::cell::{OnceCell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;

/// The section's live controls and render caches.
pub(crate) struct Controls {
    pub(crate) outpost: ChoiceSelect<usize>,
    pub(crate) resource: ChoiceSelect<usize>,
    pub(crate) currency: ChoiceSelect<i64>,
    pub(crate) upgrade_view: ChoiceSelect<UpgradeView>,
    pub(crate) plan_shelf: ChoiceSelect<usize>,
    pub(crate) horizon: ChoiceSelect<u32>,
    pub(crate) objective: ChoiceSelect<Objective>,
    pub(crate) unit_exp_outpost: ChoiceSelect<usize>,
    /// One rank field per [`Target::ALL`] entry.
    pub(crate) targets: [Entity<InputState>; 3],
    pub(crate) shelf_purchases: Entity<InputState>,
    /// The forecast's collect hour and menu-open window, read when added.
    pub(crate) collect: Entity<InputState>,
    pub(crate) window_start: Entity<InputState>,
    pub(crate) window_end: Entity<InputState>,
    /// The engine results the panes read, for `revision`.
    pub(crate) derived: Derived,
    revision: Option<u64>,
    /// The breakdown the tree shows, with the category it was built for.
    pub(crate) breakdown: Option<(Category, Breakdown)>,
    /// The requests the runs' stored results are compared against.
    pub(crate) optimizer: Option<Request>,
    pub(crate) unit_exp: Option<UnitExpRequest>,
    pub(crate) shelf: Option<ShelfRequest>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let outpost = ChoiceSelect::new(Vec::new(), None, window, cx);
        let resource = ChoiceSelect::new(Vec::new(), None, window, cx);
        let currency = ChoiceSelect::new(Vec::new(), None, window, cx);
        let upgrade_view = ChoiceSelect::new(super::upgrades::view_choices(), None, window, cx);
        let plan_shelf = ChoiceSelect::new(Vec::new(), None, window, cx);
        let horizon = ChoiceSelect::new(super::forecast::horizon_choices(), None, window, cx);
        let objective = ChoiceSelect::new(super::optimizer::objective_choices(), None, window, cx);
        let unit_exp_outpost = ChoiceSelect::new(Vec::new(), None, window, cx);
        let targets = std::array::from_fn(|_| number_state(0.0, 500.0, 1.0, window, cx));
        let shelf_purchases = number_state(1.0, 50.0, 1.0, window, cx);
        // An hour field takes digits and a decimal point only.
        let mut hours_field = |placeholder: &'static str| {
            cx.new(|cx| {
                InputState::new(window, cx)
                    .placeholder(placeholder)
                    .validate(|text, _| text.chars().all(|ch| ch.is_ascii_digit() || ch == '.'))
            })
        };
        let collect = hours_field("e.g. 6");
        let window_start = hours_field("e.g. 20");
        let window_end = hours_field("e.g. 22");

        // Edits flow control -> section value, through the section's setters.
        let mut subscriptions = vec![
            pick(cx, &outpost, AppState::set_royal_outpost),
            pick(cx, &resource, AppState::set_royal_resource),
            pick(cx, &currency, AppState::set_royal_currency),
            pick(cx, &upgrade_view, AppState::set_royal_upgrade_view),
            pick(cx, &plan_shelf, |state, order, cx| {
                state.set_royal_armory_plan(Some(order), cx);
            }),
            pick(cx, &horizon, |state, hours, cx| {
                state.set_royal_forecast_hours(f64::from(hours), cx);
            }),
            pick(cx, &objective, AppState::set_royal_optimizer_objective),
            pick(cx, &unit_exp_outpost, AppState::set_royal_unit_exp_map),
            number(cx, &shelf_purchases, |state, value, cx| {
                state.set_royal_shelf_purchases(value.round() as usize, cx);
            }),
        ];
        for (target, field) in Target::ALL.into_iter().zip(&targets) {
            subscriptions.push(cx.subscribe(field, move |state, field, event, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(rank) = number_value(field.read(cx))
                {
                    state.set_royal_unit_exp_target(target, rank.max(0.0).round() as u32, cx);
                }
            }));
        }
        for field in [&collect, &window_start, &window_end] {
            subscriptions.push(crate::state::redraw_on_edit(cx, field));
        }
        Self {
            outpost,
            resource,
            currency,
            upgrade_view,
            plan_shelf,
            horizon,
            objective,
            unit_exp_outpost,
            targets,
            shelf_purchases,
            collect,
            window_start,
            window_end,
            derived: Derived::default(),
            revision: None,
            breakdown: None,
            optimizer: None,
            unit_exp: None,
            shelf: None,
            _subscriptions: subscriptions,
        }
    }
}

/// A select whose pick calls `set`.
fn pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    set: impl Fn(&mut AppState, V, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            let SelectEvent::Confirm(value) = event;
            if let Some(value) = value.clone() {
                set(state, value, cx);
            }
        },
    )
}

/// A number field whose edits call `set`.
fn number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    set: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            set(state, value, cx);
        }
    })
}

/// The engine results one save answers, computed the first time a pane asks.
#[derive(Default)]
pub(crate) struct Derived {
    counters: OnceCell<Counters>,
    progress: OnceCell<Vec<WorldProgress>>,
    allocation: OnceCell<Allocation>,
    banked: OnceCell<Banked>,
    streamed: OnceCell<Vec<usize>>,
    incomes: RefCell<BTreeMap<i64, Rc<CurrencyIncome>>>,
    worlds: [OnceCell<WorldMap>; royal_guardian::WORLDS],
}

/// One world's progress card.
pub(crate) struct WorldProgress {
    pub(crate) built: usize,
    pub(crate) outposts: usize,
    pub(crate) clearing: usize,
    pub(crate) unlocked: bool,
}

/// One world's outpost table and map, as the Outposts pane draws them.
pub(crate) struct WorldMap {
    pub(crate) outposts: Vec<OutpostRow>,
    /// Each currency-paying node and whether an outpost reaches it.
    pub(crate) nodes: Vec<(usize, bool)>,
    pub(crate) lines: Vec<super::outposts::Line>,
}

/// One outpost the world lists.
pub(crate) struct OutpostRow {
    pub(crate) map: usize,
    pub(crate) kind: f64,
    pub(crate) range: f64,
    pub(crate) units: f64,
    pub(crate) links: String,
    pub(crate) supports: usize,
    pub(crate) rate: f64,
}

impl Derived {
    pub(crate) fn counters(&self, royal: &RoyalGuardian) -> &Counters {
        self.counters.get_or_init(|| royal.counters())
    }

    pub(crate) fn progress(&self, royal: &RoyalGuardian) -> &[WorldProgress] {
        self.progress.get_or_init(|| {
            (0..royal_guardian::WORLDS)
                .map(|world| {
                    let outposts = royal.world_outposts(world);
                    WorldProgress {
                        built: outposts.iter().filter(|outpost| outpost.built).count(),
                        outposts: outposts.len(),
                        clearing: outposts
                            .iter()
                            .filter(|outpost| {
                                !outpost.built
                                    && royal.clear(outpost.map).state
                                        == royal_guardian::ClearState::Clearing
                            })
                            .count(),
                        unlocked: royal.world_unlocked(world),
                    }
                })
                .collect()
        })
    }

    pub(crate) fn allocation(&self, royal: &RoyalGuardian) -> &Allocation {
        self.allocation.get_or_init(|| royal.allocation_to_reset())
    }

    pub(crate) fn banked(&self, royal: &RoyalGuardian) -> &Banked {
        self.banked.get_or_init(|| royal.banked())
    }

    /// The nodes a saved stream drains, in stream order.
    pub(crate) fn streamed(&self, royal: &RoyalGuardian) -> &[usize] {
        self.streamed.get_or_init(|| {
            royal
                .resource_streams()
                .0
                .iter()
                .map(|stream| stream.resource)
                .collect()
        })
    }

    pub(crate) fn income(&self, royal: &RoyalGuardian, slot: i64) -> Rc<CurrencyIncome> {
        self.incomes
            .borrow_mut()
            .entry(slot)
            .or_insert_with(|| Rc::new(royal.currency_income(slot)))
            .clone()
    }

    pub(crate) fn world(&self, royal: &RoyalGuardian, world: usize) -> &WorldMap {
        self.worlds[world].get_or_init(|| WorldMap {
            outposts: royal
                .world_outposts(world)
                .iter()
                .map(|outpost| outpost_row(royal, outpost.map))
                .collect(),
            nodes: royal
                .world_resources(world)
                .into_iter()
                .filter(|node| node.currency_slot >= 0)
                .map(|node| {
                    let reachable = royal
                        .reachable_outposts(node.resource)
                        .iter()
                        .any(|outpost| outpost.reachable);
                    (node.resource, reachable)
                })
                .collect(),
            lines: super::outposts::connection_lines(royal, world),
        })
    }
}

fn outpost_row(royal: &RoyalGuardian, map: usize) -> OutpostRow {
    let links = royal
        .outpost_connections(map)
        .iter()
        .filter_map(|connection| match connection.endpoint {
            Endpoint::Resource(resource) => Some(format!("R{}", resource + 1)),
            Endpoint::Map(target) => Some(format!("W{}", target + 1)),
            Endpoint::Empty => None,
        })
        .collect::<Vec<_>>()
        .join(" + ");
    OutpostRow {
        map,
        kind: royal.outpost_type(map),
        range: royal.range(map),
        units: royal.total_units(map),
        links,
        supports: royal.support_count(map),
        rate: royal.outpost_collection_rate(map).value,
    }
}

impl AppState {
    /// Bring the section's caches in line with the save and its inputs: a new
    /// save drops every result, and a run request or the breakdown is rebuilt
    /// only when what it reads moved, so most frames make a few comparisons.
    pub(crate) fn sync_royal(&mut self, cx: &mut Context<Self>) {
        let revision = self.save_revision;
        let controls = &mut self.royal_controls;
        if controls.revision != Some(revision) {
            controls.derived = Derived::default();
            controls.revision = Some(revision);
            controls.breakdown = None;
            controls.optimizer = None;
            controls.unit_exp = None;
            controls.shelf = None;
        }
        let Some(royal) = self.royal.as_ref() else {
            return;
        };
        let objective = self.royal_state.optimizer;
        if controls
            .optimizer
            .as_ref()
            .is_none_or(|request| request.objective != objective)
        {
            controls.optimizer = Some(Request::new(royal, objective));
        }
        let purchases = self.royal_state.shelf_purchases;
        if controls
            .shelf
            .as_ref()
            .is_none_or(|request| request.purchases != purchases)
        {
            controls.shelf = Some(ShelfRequest::new(royal, purchases));
        }
        let map = self.royal_unit_exp_map();
        let controls = &mut self.royal_controls;
        let targets = self.royal_state.unit_exp_targets;
        let move_transients = self.royal_state.unit_exp_move_transients;
        let unit_exp_moved = match (&controls.unit_exp, map) {
            (Some(request), Some(map)) => {
                request.map != map
                    || request.targets != targets
                    || request.move_transients != move_transients
            }
            (None, None) => false,
            _ => true,
        };
        if unit_exp_moved {
            controls.unit_exp = map.map(|map| UnitExpRequest {
                royal: royal.clone(),
                map,
                targets,
                move_transients,
            });
        }
        let category = self.royal_state.breakdown;
        if self.royal_state.tab == super::Tab::Breakdowns
            && controls
                .breakdown
                .as_ref()
                .is_none_or(|(built, _)| *built != category)
        {
            let breakdown = royal.global_breakdown(category);
            let root = breakdown.root.clone();
            self.royal_breakdown
                .update(cx, |tree, cx| tree.set_root(root, cx));
            self.royal_controls.breakdown = Some((category, breakdown));
        }
    }
}
