//! The section's kit form controls and the virtualized views, built once.
//!
//! The persisted values stay on [`State`](super::State) (`docs/section-state.md`).
//! These entities only show them: edits flow control → value through the
//! subscriptions made here, and values flow back in render through
//! `show`/`show_number` (`docs/ui-kit/forms.md`). The kitchen pins and the meal
//! targets depend on the save, so they are added as a save needs them.

use super::outcomes::{self, OutcomeSort};
use super::planner::ScheduleRow;
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, ListAlignment, ListState, Subscription, Window, px};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_ladle::Panel;
use idleon_ladle::kitchen::{self, LayoutMode, Objective, Policy, Scenario};
use idleon_ui::widgets::{Choice, ChoiceSelect, RowsDelegate, number_state, number_value};

/// One number field: its label, element id suffix, range, step and the
/// decimals it shows.
pub(super) struct Field {
    pub(super) label: &'static str,
    pub(super) id: &'static str,
    pub(super) min: f64,
    pub(super) max: f64,
    pub(super) step: f64,
    pub(super) precision: usize,
}

const fn field(
    label: &'static str,
    id: &'static str,
    min: f64,
    max: f64,
    step: f64,
    precision: usize,
) -> Field {
    Field {
        label,
        id,
        min,
        max,
        step,
        precision,
    }
}

/// The AFK panel values, as `ladle-panel-<id>`.
pub(super) const PANEL: [Field; 5] = [
    field("Cooking efficiency", "efficiency", 0.0, 1e15, 100.0, 2),
    field("AFK gain rate", "afk", 0.0, 100.0, 0.05, 2),
    field("EXP multiplier", "exp", 0.0, 1000.0, 0.05, 2),
    field("Skill prowess", "prowess", 0.0, 0.1, 0.001, 3),
    field("Talent 148 %", "talent", 0.0, 10_000.0, 1.0, 2),
];

/// The panel value the field at `index` in [`PANEL`] edits.
pub(super) fn panel_value(panel: &mut Panel, index: usize) -> &mut f64 {
    match index {
        0 => &mut panel.efficiency,
        1 => &mut panel.afk,
        2 => &mut panel.exp_multi,
        3 => &mut panel.prowess,
        _ => &mut panel.talent_148,
    }
}

/// The meal cost terms, as `kitchens-cost-<id>`.
pub(super) const COST: [Field; 5] = [
    field("Cost companion 162", "companion", 0.0, 1e6, 1.0, 0),
    field("Achievement 233", "achievement", 0.0, 1.0, 1.0, 0),
    field("Cloud 33", "cloud", 0.0, 1.0, 0.05, 2),
    field("Dream days banked", "discount", 0.0, 3650.0, 1.0, 0),
    field("Dream days cap", "dream", 0.0, 3650.0, 1.0, 0),
];

/// The cost term the field at `index` in [`COST`] edits.
pub(super) fn cost_value(inputs: &mut kitchen::Inputs, index: usize) -> &mut f64 {
    let cost = &mut inputs.cost;
    match index {
        0 => &mut cost.companion_162,
        1 => &mut cost.achievement_233,
        2 => &mut cost.cloud_33,
        3 => &mut cost.discount_days,
        _ => &mut cost.dream_11,
    }
}

/// A kitchen pin: `None` lets the optimizer choose, `Some(-1)` keeps it idle,
/// otherwise the meal it cooks.
pub(super) type Pin = Option<i32>;

/// Every kit control the tabs draw, and the views a save fills.
pub(crate) struct Controls {
    pub(super) ladle_character: ChoiceSelect<usize>,
    pub(super) breakdown_character: ChoiceSelect<usize>,
    pub(super) claim_hours: Entity<InputState>,
    pub(super) panel: [Entity<InputState>; 5],
    pub(super) balloon: ChoiceSelect<usize>,
    pub(super) target_level: Entity<InputState>,
    pub(super) objective: ChoiceSelect<Objective>,
    pub(super) policy: ChoiceSelect<Policy>,
    pub(super) share: Entity<InputState>,
    pub(super) fixed: Entity<InputState>,
    pub(super) owned: Entity<InputState>,
    pub(super) days: Entity<InputState>,
    pub(super) speed: Entity<InputState>,
    pub(super) max_level: Entity<InputState>,
    pub(super) cost: [Entity<InputState>; 5],
    pub(super) reset: ChoiceSelect<usize>,
    pub(super) layout_mode: ChoiceSelect<LayoutMode>,
    /// One pin select per kitchen, by `Cooking` row index.
    pub(super) pins: Vec<ChoiceSelect<Pin>>,
    pub(super) scenario: ChoiceSelect<Scenario>,
    pub(super) sort: ChoiceSelect<OutcomeSort>,
    /// One target field per meal, by meal index.
    pub(super) targets: Vec<Entity<InputState>>,
    pub(super) outcomes: Entity<TableState<outcomes::Rows>>,
    pub(super) schedule: Entity<TableState<RowsDelegate<ScheduleRow>>>,
    /// The forecast's resets, one list item each; only those in view are drawn.
    pub(super) days_list: ListState,
    subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(super) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let number = |field: &Field, window: &mut Window, cx: &mut Context<AppState>| {
            number_state(field.min, field.max, field.step, window, cx)
        };
        let controls = Self {
            ladle_character: ChoiceSelect::new(Vec::new(), None, window, cx),
            breakdown_character: ChoiceSelect::new(Vec::new(), None, window, cx),
            claim_hours: number_state(0.1, 24.0, 0.5, window, cx),
            panel: [
                number(&PANEL[0], window, cx),
                number(&PANEL[1], window, cx),
                number(&PANEL[2], window, cx),
                number(&PANEL[3], window, cx),
                number(&PANEL[4], window, cx),
            ],
            balloon: ChoiceSelect::new(Vec::new(), None, window, cx),
            target_level: number_state(0.0, 1000.0, 1.0, window, cx),
            objective: ChoiceSelect::new(objective_choices(), None, window, cx),
            policy: ChoiceSelect::new(policy_choices(), None, window, cx),
            share: number_state(0.0, 100.0, 5.0, window, cx),
            fixed: number_state(0.0, 100_000.0, 1.0, window, cx),
            owned: number_state(0.0, 100.0, 5.0, window, cx),
            days: number_state(1.0, 365.0, 1.0, window, cx),
            speed: number_state(0.0, 1e12, 10.0, window, cx),
            max_level: number_state(2.0, 1000.0, 1.0, window, cx),
            cost: [
                number(&COST[0], window, cx),
                number(&COST[1], window, cx),
                number(&COST[2], window, cx),
                number(&COST[3], window, cx),
                number(&COST[4], window, cx),
            ],
            reset: ChoiceSelect::new(Vec::new(), None, window, cx),
            layout_mode: ChoiceSelect::new(layout_mode_choices(), None, window, cx),
            pins: Vec::new(),
            scenario: ChoiceSelect::new(scenario_choices(), None, window, cx),
            sort: ChoiceSelect::new(sort_choices(), None, window, cx),
            targets: Vec::new(),
            outcomes: outcomes::state(Vec::new(), window, cx),
            schedule: cx.new(|cx| TableState::new(super::planner::schedule_table(), window, cx)),
            days_list: ListState::new(0, ListAlignment::Top, px(200.0)),
            subscriptions: Vec::new(),
        };
        let subscriptions = controls.subscribe(cx);
        Self {
            subscriptions,
            ..controls
        }
    }

    /// Edits flow control → section value.
    fn subscribe(&self, cx: &mut Context<AppState>) -> Vec<Subscription> {
        let mut subscriptions = vec![
            on_pick(cx, &self.ladle_character, |state, index| {
                state.ladle_state.ladle_character = index;
            }),
            on_pick(cx, &self.breakdown_character, |state, index| {
                state.ladle_state.breakdown_character = index;
            }),
            on_number(cx, &self.claim_hours, |state, value| {
                state.ladle_state.claim_hours = value.clamp(0.1, 24.0);
            }),
            on_pick(cx, &self.balloon, |state, index| {
                state.ladle_state.balloon = index.min(idleon_ladle::EXP_BALLOONS.len() - 1);
            }),
            on_number(cx, &self.target_level, |state, value| {
                state.ladle_state.target_level = value.clamp(0.0, 1000.0).round() as u32;
            }),
            on_pick(cx, &self.objective, |state, objective| {
                state
                    .ladle_state
                    .set_kitchen(|inputs| inputs.objective = objective);
            }),
            on_pick(cx, &self.policy, |state, policy| {
                state.ladle_state.set_kitchen(|inputs| {
                    inputs.policy = policy;
                    // The policy and an applied schedule are alternatives.
                    inputs.donations = None;
                });
            }),
            on_number(cx, &self.share, |state, value| {
                state.ladle_state.set_kitchen(|inputs| {
                    inputs.income_fraction = (value / 100.0).clamp(0.0, 1.0);
                    inputs.donations = None;
                });
            }),
            on_number(cx, &self.fixed, |state, value| {
                state.ladle_state.set_kitchen(|inputs| {
                    inputs.donation_per_day = value.clamp(0.0, 100_000.0);
                    inputs.donations = None;
                });
            }),
            on_number(cx, &self.owned, |state, value| {
                state.ladle_state.set_kitchen(|inputs| {
                    inputs.owned_fraction = (value / 100.0).clamp(0.0, 1.0);
                    inputs.donations = None;
                });
            }),
            on_number(cx, &self.days, |state, value| {
                let ladle = &mut state.ladle_state;
                ladle.set_kitchen(|inputs| inputs.days = value.clamp(1.0, 365.0).round() as u32);
                // The forecast's selected reset must stay inside the horizon.
                ladle.day = ladle.day.clamp(1, ladle.kitchen.days as usize);
            }),
            on_number(cx, &self.speed, |state, value| {
                state
                    .ladle_state
                    .set_kitchen(|inputs| inputs.base_speed = value.clamp(0.0, 1e12));
            }),
            on_number(cx, &self.max_level, |state, value| {
                state
                    .ladle_state
                    .set_kitchen(|inputs| inputs.max_level = value.clamp(2.0, 1000.0));
            }),
            on_pick(cx, &self.reset, |state, day| {
                state.ladle_state.day = day;
            }),
            on_pick(cx, &self.layout_mode, |state, mode| {
                state
                    .ladle_state
                    .set_kitchen(|inputs| inputs.layout.mode = mode);
            }),
            on_pick(cx, &self.scenario, |state, scenario| {
                state.ladle_state.scenario = scenario;
            }),
            on_pick(cx, &self.sort, |state, sort| {
                state.ladle_state.outcome_sort = sort;
            }),
        ];
        for (index, field) in self.panel.iter().enumerate() {
            let Field { min, max, .. } = PANEL[index];
            subscriptions.push(on_number(cx, field, move |state, value| {
                *panel_value(&mut state.ladle_state.panel, index) = value.clamp(min, max);
            }));
        }
        for (index, field) in self.cost.iter().enumerate() {
            let Field { min, max, .. } = COST[index];
            subscriptions.push(on_number(cx, field, move |state, value| {
                state.ladle_state.set_kitchen(|inputs| {
                    *cost_value(inputs, index) = value.clamp(min, max);
                });
            }));
        }
        subscriptions
    }

    /// Give every kitchen up to `kitchens` its pin select.
    pub(super) fn ensure_pins(
        &mut self,
        kitchens: usize,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        while self.pins.len() < kitchens {
            let index = self.pins.len();
            let select = ChoiceSelect::new(Vec::new(), None, window, cx);
            self.subscriptions
                .push(on_pick(cx, &select, move |state, pin: Pin| {
                    state.ladle_state.set_pin(index, pin);
                }));
            self.pins.push(select);
        }
    }

    /// Give every meal up to `meals` its target field, and hand the outcomes
    /// table the fields it draws.
    pub(super) fn ensure_targets(
        &mut self,
        meals: usize,
        window: &mut Window,
        cx: &mut Context<AppState>,
    ) {
        if self.targets.len() >= meals {
            return;
        }
        while self.targets.len() < meals {
            let meal = self.targets.len();
            let field = number_state(0.0, 1000.0, 1.0, window, cx);
            self.subscriptions
                .push(on_number(cx, &field, move |state, level| {
                    let ladle = &mut state.ladle_state;
                    // A target past the meal cap is the cap.
                    let cap = ladle.kitchen.max_level.max(1.0);
                    ladle.set_target(meal, level.min(cap));
                }));
            self.targets.push(field);
        }
        let targets = self.targets.clone();
        self.outcomes.update(cx, |table, _| {
            table.delegate_mut().targets = targets;
        });
    }
}

/// A typed number reaches the section once it parses.
fn on_number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    apply: impl Fn(&mut AppState, f64) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            apply(state, value);
            cx.notify();
        }
    })
}

/// A confirmed pick reaches the section; a cleared one keeps the value.
fn on_pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    apply: impl Fn(&mut AppState, V) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            let SelectEvent::Confirm(value) = event;
            if let Some(value) = value.clone() {
                apply(state, value);
                cx.notify();
            }
        },
    )
}

pub(super) fn objective_choices() -> Vec<Choice<Objective>> {
    [Objective::Ranks, Objective::Targets]
        .into_iter()
        .map(|objective| Choice::new(objective, outcomes::objective_label(objective)))
        .collect()
}

pub(super) fn policy_choices() -> Vec<Choice<Policy>> {
    [
        (Policy::None, "No new donations"),
        (Policy::All, "Donate all daily income"),
        (Policy::Share, "Donate a share of daily income"),
        (Policy::Fixed, "Donate a fixed amount daily"),
    ]
    .into_iter()
    .map(|(policy, label)| Choice::new(policy, label))
    .collect()
}

pub(super) fn layout_mode_choices() -> Vec<Choice<LayoutMode>> {
    vec![
        Choice::new(LayoutMode::Optimized, "Optimizer chooses the meals"),
        Choice::new(LayoutMode::Saved, "Keep the saved assignments"),
    ]
}

pub(super) fn scenario_choices() -> Vec<Choice<Scenario>> {
    kitchen::SCENARIOS
        .into_iter()
        .map(|scenario| Choice::new(scenario, scenario.label()))
        .collect()
}

pub(super) fn sort_choices() -> Vec<Choice<OutcomeSort>> {
    OutcomeSort::ALL
        .into_iter()
        .map(|sort| Choice::new(sort, sort.label()))
        .collect()
}
