//! The section's kit form controls, its ranking table and what the page reads
//! from the evaluator, built once.
//!
//! The persisted values stay on [`State`](super::State)
//! (`docs/section-state.md`). The controls only show them: edits flow control →
//! value through the subscriptions made here, and values flow back in render
//! through `show`/`show_number` (`docs/ui-kit/forms.md`). Every pool, the 225
//! lure/line combinations and the Trench projection are evaluated once per
//! [`Key`] ([`cached`]), so a frame that only redraws formats nothing
//! (`docs/ui-kit/data.md`).

use super::ranking::{self, Ranking};
use super::{State, trench};
use crate::state::AppState;
use gpui::{App, AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_fishing::{self as fishing, AccountInputs, HelperMode, Objective};
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};
use std::rc::Rc;

/// What the pools and the ranking read: the save and every toolbar and account
/// value. The Trench also reads its own settings ([`TrenchKey`]).
#[derive(Clone, PartialEq)]
pub(crate) struct Key {
    revision: u64,
    character: usize,
    lure: usize,
    line: usize,
    location: String,
    hours: f64,
    objective: Objective,
    account: AccountInputs,
}

/// What the Trench projection reads.
#[derive(Clone, PartialEq)]
pub(crate) struct TrenchKey {
    revision: u64,
    state: State,
}

impl AppState {
    pub(crate) fn fishing_key(&self) -> Key {
        let config = &self.fishing;
        Key {
            revision: self.save_revision,
            character: config.character,
            lure: config.lure,
            line: config.line,
            location: config.location.clone(),
            hours: config.hours,
            objective: config.objective,
            account: config.account,
        }
    }

    pub(crate) fn fishing_trench_key(&self) -> TrenchKey {
        TrenchKey {
            revision: self.save_revision,
            state: self.fishing.clone(),
        }
    }
}

/// A value read from the evaluator, and the key it was read for.
pub(crate) type Cached<K, V> = Option<(K, Rc<V>)>;

/// The value in `slot` when it was read for `key`, else `build`'s, kept.
pub(crate) fn cached<K: PartialEq, V>(
    state: &mut AppState,
    slot: fn(&mut Controls) -> &mut Cached<K, V>,
    key: K,
    cx: &mut App,
    build: impl FnOnce(&AppState, &mut App) -> V,
) -> Rc<V> {
    if let Some((at, value)) = slot(&mut state.fishing_controls)
        && *at == key
    {
        return value.clone();
    }
    let value = Rc::new(build(state, cx));
    *slot(&mut state.fishing_controls) = Some((key, value.clone()));
    value
}

/// One account value the card edits: its field, range and how it maps onto
/// [`AccountInputs`].
pub(crate) struct AccountField {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    min: f64,
    max: f64,
    step: f64,
    pub(crate) precision: usize,
    pub(crate) get: fn(&AccountInputs) -> f64,
    set: fn(&mut AccountInputs, f64),
}

/// The account card's fields, in reading order.
pub(crate) const ACCOUNT_FIELDS: [AccountField; 10] = [
    AccountField {
        id: "fishing-input-efficiency",
        label: "Fishing efficiency",
        min: 0.0,
        max: 1e12,
        step: 1000.0,
        precision: 0,
        get: |account| account.efficiency,
        set: |account, value| account.efficiency = value,
    },
    AccountField {
        id: "fishing-input-exp",
        label: "EXP multiplier",
        min: 0.0,
        max: 1e6,
        step: 0.05,
        precision: 2,
        get: |account| account.exp_multi,
        set: |account, value| account.exp_multi = value,
    },
    AccountField {
        id: "fishing-input-afk",
        label: "AFK gains rate %",
        min: 0.0,
        max: 100_000.0,
        step: 1.0,
        precision: 2,
        get: |account| account.afk_rate * 100.0,
        set: |account, value| account.afk_rate = value / 100.0,
    },
    AccountField {
        id: "fishing-input-multi",
        label: "Multi-Fish %",
        min: 0.0,
        max: 10_000.0,
        step: 1.0,
        precision: 2,
        get: |account| account.multi_fish,
        set: |account, value| account.multi_fish = value,
    },
    AccountField {
        id: "fishing-input-speed",
        label: "Speed bonus %",
        min: 0.0,
        max: 100_000.0,
        step: 1.0,
        precision: 2,
        get: |account| account.speed_bonus,
        set: |account, value| account.speed_bonus = value,
    },
    AccountField {
        id: "fishing-input-attack",
        label: "Attack loadout factor",
        min: 1.0,
        max: 3.0,
        step: 0.05,
        precision: 2,
        get: |account| account.attack_factor,
        set: |account, value| account.attack_factor = value,
    },
    AccountField {
        id: "fishing-input-prowess",
        label: "Prowess",
        min: 0.0,
        max: 100.0,
        step: 0.01,
        precision: 4,
        get: |account| account.prowess,
        set: |account, value| account.prowess = value,
    },
    AccountField {
        id: "fishing-input-d1",
        label: "Equipment D1",
        min: 0.0,
        max: 1e6,
        step: 1.0,
        precision: 0,
        get: |account| account.equipment_depth[0],
        set: |account, value| account.equipment_depth[0] = value,
    },
    AccountField {
        id: "fishing-input-d2",
        label: "Equipment D2",
        min: 0.0,
        max: 1e6,
        step: 1.0,
        precision: 0,
        get: |account| account.equipment_depth[1],
        set: |account, value| account.equipment_depth[1] = value,
    },
    AccountField {
        id: "fishing-input-d3",
        label: "Equipment D3",
        min: 0.0,
        max: 1e6,
        step: 1.0,
        precision: 0,
        get: |account| account.equipment_depth[2],
        set: |account, value| account.equipment_depth[2] = value,
    },
];

/// The section's kit controls, ranking table and caches.
pub(crate) struct Controls {
    pub(crate) character: ChoiceSelect<usize>,
    pub(crate) lure: ChoiceSelect<usize>,
    pub(crate) line: ChoiceSelect<usize>,
    pub(crate) location: ChoiceSelect<String>,
    pub(crate) objective: ChoiceSelect<Objective>,
    pub(crate) helper_mode: ChoiceSelect<HelperMode>,
    pub(crate) trench_depths: ChoiceSelect<usize>,
    pub(crate) hours: Entity<InputState>,
    pub(crate) account: [Entity<InputState>; 10],
    /// The lists that never change: every lure, line and location.
    pub(crate) lures: Vec<Choice<usize>>,
    pub(crate) lines: Vec<Choice<usize>>,
    pub(crate) locations: Vec<Choice<String>>,
    /// The 225 lure/line combinations, best first.
    pub(crate) ranking: Entity<TableState<Ranking>>,
    pub(crate) characters: Cached<u64, Vec<Choice<usize>>>,
    pub(crate) page: Cached<Key, super::Page>,
    pub(crate) trench: Cached<TrenchKey, Option<trench::View>>,
    _subscriptions: Vec<Subscription>,
}

/// What the ranking can sort by.
pub(crate) fn objective_choices() -> Vec<Choice<Objective>> {
    [Objective::Exp, Objective::Fish]
        .map(|objective| Choice::new(objective, objective.label()))
        .into()
}

/// How the other Trench characters pick their toolkit.
pub(crate) fn helper_choices() -> Vec<Choice<HelperMode>> {
    [HelperMode::Saved, HelperMode::Best]
        .map(|mode| Choice::new(mode, mode.label()))
        .into()
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let data = fishing::data();
        let lures: Vec<Choice<usize>> = data
            .lures
            .iter()
            .map(|lure| Choice::new(lure.idx, with_note(&lure.name, &stats_text(&lure.stats))))
            .collect();
        let lines: Vec<Choice<usize>> = data
            .lines
            .iter()
            .map(|line| Choice::new(line.idx, with_note(&line.name, &stats_text(&line.stats))))
            .collect();
        let locations: Vec<Choice<String>> = data
            .locations
            .iter()
            .map(|location| {
                Choice::new(
                    location.id.clone(),
                    with_note(
                        &location.label,
                        &format!(
                            "{} · {} · depth {}",
                            location.pool_name,
                            location.map_name,
                            super::spot_text(&location.bonus)
                        ),
                    ),
                )
            })
            .collect();
        let app = cx.weak_entity();
        let controls = Self {
            character: ChoiceSelect::new(Vec::new(), None, window, cx),
            lure: ChoiceSelect::new(lures.clone(), None, window, cx),
            line: ChoiceSelect::new(lines.clone(), None, window, cx),
            location: ChoiceSelect::new(locations.clone(), None, window, cx),
            objective: ChoiceSelect::new(objective_choices(), None, window, cx),
            helper_mode: ChoiceSelect::new(helper_choices(), None, window, cx),
            trench_depths: ChoiceSelect::new(Vec::new(), None, window, cx),
            hours: number_state(0.04, 10_000.0, 0.5, window, cx),
            account: std::array::from_fn(|index| {
                let field = &ACCOUNT_FIELDS[index];
                number_state(field.min, field.max, field.step, window, cx)
            }),
            lures,
            lines,
            locations,
            ranking: cx.new(|cx| ranking::state(app, window, cx)),
            characters: None,
            page: None,
            trench: None,
            _subscriptions: Vec::new(),
        };
        // Edits flow control -> value, through the section's setters.
        let mut subscriptions = vec![
            on_pick(cx, &controls.character, |state, index, cx| {
                state.set_fishing_character(index, cx);
            }),
            on_pick(cx, &controls.lure, |state, index, cx| {
                state.set_fishing_lure(index, cx);
            }),
            on_pick(cx, &controls.line, |state, index, cx| {
                state.set_fishing_line(index, cx);
            }),
            on_pick(cx, &controls.location, |state, id, cx| {
                state.set_fishing_location(id, cx);
            }),
            on_pick(cx, &controls.objective, |state, objective, cx| {
                state.set_fishing_objective(objective, cx);
            }),
            on_pick(cx, &controls.helper_mode, |state, mode, cx| {
                state.set_fishing_helper_mode(mode, cx);
            }),
            on_pick(cx, &controls.trench_depths, |state, depths, cx| {
                state.set_fishing_trench_depths(depths, cx);
            }),
            on_number(cx, &controls.hours, |state, hours, cx| {
                state.set_fishing_hours(hours, cx);
            }),
        ];
        for (index, field) in controls.account.iter().enumerate() {
            subscriptions.push(on_number(cx, field, move |state, value, cx| {
                let set = ACCOUNT_FIELDS[index].set;
                state.set_fishing_account(|account| set(account, value), cx);
            }));
        }
        Self {
            _subscriptions: subscriptions,
            ..controls
        }
    }
}

/// `Name · note`: a kit option has no separate note.
fn with_note(name: &str, note: &str) -> String {
    if note.is_empty() {
        name.to_string()
    } else {
        format!("{name} · {note}")
    }
}

/// The toolkit stats the selectors list.
fn stats_text(stats: &fishing::ToolkitStats) -> String {
    [
        ("D0", stats.d0),
        ("D1", stats.d1),
        ("D2", stats.d2),
        ("D3", stats.d3),
        ("EXP", stats.exp),
        ("SPEED", stats.speed),
        ("POW", stats.pow),
    ]
    .iter()
    .filter(|(_, value)| *value != 0.0)
    .map(|(name, value)| format!("{name} {value}"))
    .collect::<Vec<_>>()
    .join(" · ")
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
