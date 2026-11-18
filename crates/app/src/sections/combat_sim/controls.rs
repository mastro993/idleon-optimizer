//! The kit controls the section owns (`docs/ui-kit/forms.md`).
//!
//! Every select, number field and table is built once here and kept next to
//! the section's values. Render pushes those values into the controls; an edit
//! comes back through a subscription made here, which calls the section's
//! setter. The controls never hold a persisted value.

use super::{compare, overrides, talents};
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::slider::SliderState;
use gpui_kit::component::table::TableState;
use idleon_combat::encounter;
use idleon_ui::widgets::{Choice, ChoiceSelect, number_state, number_value};

/// The simulated-duration presets, in seconds.
pub(crate) const DURATIONS: [u64; 8] = [60, 300, 600, 1800, 3600, 21_600, 43_200, 86_400];

/// A slot's `(row, slot, item)`: what the item picker's options depend on.
pub(crate) type ItemSlot = (usize, usize, String);

pub(crate) struct Controls {
    pub(crate) character: ChoiceSelect<usize>,
    pub(crate) item: ChoiceSelect<String>,
    /// The item picker's options and the `(row, slot, item)` they list, so the
    /// catalog is walked again only when the slot or its item changes.
    pub(crate) items: Option<(ItemSlot, Vec<Choice<String>>)>,
    pub(crate) group: ChoiceSelect<String>,
    pub(crate) map: ChoiceSelect<usize>,
    pub(crate) maps: Vec<Choice<usize>>,
    /// Whether the run uses the idle player.
    pub(crate) player: ChoiceSelect<bool>,
    pub(crate) duration: ChoiceSelect<u64>,
    pub(crate) kill_every: Entity<InputState>,
    pub(crate) seed: Entity<InputState>,
    /// The comparison's "Add a map" picker; it always shows its placeholder.
    pub(crate) compare_map: ChoiceSelect<usize>,
    pub(crate) compare_maps: Vec<Choice<usize>>,
    pub(crate) talents: Entity<TableState<talents::Rows>>,
    pub(crate) mechanics: Entity<TableState<overrides::Rows>>,
    pub(crate) comparison: Entity<TableState<compare::Rows>>,
    /// The replay scrubber and the last frame it was built for; a run with
    /// another frame count gets a new one.
    pub(crate) scrubber: Option<(usize, Entity<SliderState>, Subscription)>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let maps: Vec<Choice<usize>> = encounter::maps()
            .iter()
            .map(|map| Choice::new(map.index, encounter::map_name(map)))
            .collect();
        let compare_maps: Vec<Choice<usize>> = encounter::maps()
            .iter()
            .map(|map| {
                Choice::new(
                    map.index,
                    format!("{} · {}", map.index, encounter::map_name(map)),
                )
            })
            .collect();
        let character = ChoiceSelect::new(Vec::new(), None, window, cx);
        let item = ChoiceSelect::new(Vec::new(), None, window, cx);
        let group = ChoiceSelect::new(group_choices(), Some(&String::new()), window, cx);
        let map = ChoiceSelect::new(maps.clone(), None, window, cx);
        let player = ChoiceSelect::new(player_choices(), Some(&false), window, cx);
        let duration = ChoiceSelect::new(duration_choices(), Some(&300), window, cx);
        let kill_every = number_state(10.0, 60_000.0, 10.0, window, cx);
        let seed = number_state(1.0, 4_294_967_295.0, 1.0, window, cx);
        let compare_map = ChoiceSelect::new(compare_maps.clone(), None, window, cx);
        let app = cx.weak_entity();
        let talents = cx.new(|cx| TableState::new(talents::Rows::new(app.clone()), window, cx));
        let mechanics = cx.new(|cx| TableState::new(overrides::Rows::new(app.clone()), window, cx));
        let comparison = cx.new(|cx| TableState::new(compare::Rows::default(), window, cx));

        let subscriptions = vec![
            cx.subscribe(
                character.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(index)) = event {
                        state.set_combat_character(*index, cx);
                    }
                },
            ),
            cx.subscribe(
                item.state(),
                |state, _, event: &SelectEvent<Vec<Choice<String>>>, cx| {
                    let SelectEvent::Confirm(Some(key)) = event else {
                        return;
                    };
                    let slot = state.combat.loadout.slot_at(state.combat.selected);
                    if let Some((row, slot)) = slot.map(|slot| (slot.row, slot.slot)) {
                        state.set_combat_item(row, slot, key, cx);
                    }
                },
            ),
            cx.subscribe(
                group.state(),
                |state, _, event: &SelectEvent<Vec<Choice<String>>>, cx| {
                    if let SelectEvent::Confirm(Some(group)) = event {
                        state.set_combat_override_group(group, cx);
                    }
                },
            ),
            cx.subscribe(
                map.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(index)) = event {
                        state.set_combat_encounter_map(*index, cx);
                    }
                },
            ),
            cx.subscribe(
                player.state(),
                |state, _, event: &SelectEvent<Vec<Choice<bool>>>, cx| {
                    if let SelectEvent::Confirm(Some(idle)) = event {
                        state.set_combat_encounter_idle(*idle, cx);
                    }
                },
            ),
            cx.subscribe(
                duration.state(),
                |state, _, event: &SelectEvent<Vec<Choice<u64>>>, cx| {
                    if let SelectEvent::Confirm(Some(seconds)) = event {
                        state.set_combat_encounter_duration(*seconds, cx);
                    }
                },
            ),
            cx.subscribe(&kill_every, |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    state.set_combat_encounter_kill_every(value, cx);
                }
            }),
            cx.subscribe(&seed, |state, field, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change)
                    && let Some(value) = number_value(field.read(cx))
                {
                    state.set_combat_encounter_seed(value, cx);
                }
            }),
            cx.subscribe(
                compare_map.state(),
                |state, _, event: &SelectEvent<Vec<Choice<usize>>>, cx| {
                    if let SelectEvent::Confirm(Some(map)) = event {
                        state.add_combat_compare_map(*map, cx);
                    }
                },
            ),
        ];
        Self {
            character,
            item,
            items: None,
            group,
            map,
            maps,
            player,
            duration,
            kill_every,
            seed,
            compare_map,
            compare_maps,
            talents,
            mechanics,
            comparison,
            scrubber: None,
            _subscriptions: subscriptions,
        }
    }
}

/// The group filter: every group, then each catalog group.
pub(crate) fn group_choices() -> Vec<Choice<String>> {
    std::iter::once(Choice::new(String::new(), "All groups"))
        .chain(
            idleon_combat::overrides::groups()
                .iter()
                .map(|group| Choice::new(group.clone(), group.clone())),
        )
        .collect()
}

pub(crate) fn player_choices() -> Vec<Choice<bool>> {
    vec![
        Choice::new(false, "Fixed kill interval (stand-in)"),
        Choice::new(true, "Idle (no attacks)"),
    ]
}

pub(crate) fn duration_choices() -> Vec<Choice<u64>> {
    DURATIONS
        .iter()
        .map(|seconds| Choice::new(*seconds, duration_label(*seconds)))
        .collect()
}

pub(crate) fn duration_label(seconds: u64) -> String {
    match seconds {
        60 => "1 minute".into(),
        300 => "5 minutes".into(),
        600 => "10 minutes".into(),
        1800 => "30 minutes".into(),
        3600 => "1 hour".into(),
        21_600 => "6 hours".into(),
        43_200 => "12 hours".into(),
        86_400 => "24 hours".into(),
        _ => format!("{seconds}s"),
    }
}
