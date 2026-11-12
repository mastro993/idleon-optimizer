//! The mechanics overrides pane (CS-02).
//!
//! Ported from the Mechanics overrides tab in `combat-sim-calc.html` and
//! `js/stats/systems/common/combat-sim-toggles.js`: a searchable catalog grouped
//! by mechanic family, one mode per row (Saved, Off, Set, Scale ×), a value field
//! for the two valued modes, and a reset that puts every mechanic back to its
//! saved value. The active rules are also chips above the editor, so what a run
//! would change is visible from any tab.
//!
//! The saved values the Scale mode multiplies arrive with the encounter run's
//! inputs (CS-03); until then the pane still takes every rule and persists it.

use crate::state::AppState;
use crate::ui::layout;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tag, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::forms::{Checkbox, Choice, NumberInput, SearchInput, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, Styled, Window, div, px,
};
use idleon_core::combat::overrides::{self, Mode};

/// The mechanics pane: the toolbar and one row per mechanic.
pub fn pane(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let active = state.combat.overrides.len();
    let query = state.combat.override_query(cx).trim().to_lowercase();
    let group = state.combat.override_group.clone();
    let only_active = state.combat.override_only_active;
    let search_focus = state.combat.override_search.read(cx).focus().clone();
    let entity = cx.entity();
    let reset = entity.clone();
    let reset_enabled = active > 0;

    let matching: Vec<&'static overrides::Toggle> = overrides::data()
        .toggles
        .iter()
        .filter(|toggle| group.is_empty() || toggle.group == group)
        .filter(|toggle| !only_active || state.combat.overrides.active(&toggle.id))
        .filter(|toggle| {
            query.is_empty()
                || toggle.label.to_lowercase().contains(&query)
                || toggle.desc.to_lowercase().contains(&query)
                || toggle.group.to_lowercase().contains(&query)
                || toggle.id.to_lowercase().contains(&query)
                || toggle
                    .keys
                    .iter()
                    .any(|key| key.to_lowercase().contains(&query))
        })
        .collect();

    let body = if matching.is_empty() {
        EmptyState::new(
            "combat-overrides-no-match",
            IconName::SearchX,
            "No mechanic matches",
        )
        .body("Clear the search or pick another group to reach the rest.")
        .into_any_element()
    } else {
        let mut table = Table::new(["Mechanic", "Group", "Mode", "Value"]);
        for toggle in &matching {
            table = table.row(mechanic_row(state, toggle, cx));
        }
        ScrollArea::new("combat-overrides-scroll")
            .vertical()
            .h(layout::table_height(f32::from(
                window.viewport_size().height,
            )))
            .child(table)
            .into_any_element()
    };

    Card::new()
        .header(
            CardHeader::new("Mechanics overrides")
                .description(
                    "Turn a mechanic off, replace its saved input with a value, or scale it. \
                     Overrides belong to the working loadout and every run reads them; Reset \
                     puts each input back to the save.",
                )
                .action(
                    div()
                        .id("combat-overrides-count")
                        .role(Role::Label)
                        .aria_label(format!("{active} active overrides"))
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(format!("{active} active")),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .min_h_0()
                .p_3()
                .child(
                    div()
                        .id("combat-overrides-toolbar")
                        .flex()
                        .flex_wrap()
                        .items_end()
                        .gap_2()
                        .child(
                            div()
                                .id("combat-overrides-search")
                                .role(Role::Search)
                                .aria_label("Find a mechanic")
                                .w(px(240.0))
                                .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                                .child(SearchInput::new(
                                    "combat-overrides-search-field",
                                    &state.combat.override_search,
                                )),
                        )
                        .child(group_select(state, cx))
                        .child(only_active_check(state, cx))
                        .child(div().flex_1())
                        .child(
                            Button::new("combat-overrides-reset", "Reset all")
                                .size(ControlSize::Sm)
                                .disabled(!reset_enabled)
                                .on_click(move |_event, _window, cx| {
                                    reset.update(cx, |state, cx| {
                                        state.reset_combat_overrides(cx);
                                    });
                                }),
                        ),
                )
                .child(Caption::new(if state.save_loaded {
                    "Saved values and the value Scale multiplies arrive with the encounter run, \
                     which reads these rules against the selected character and map."
                } else {
                    "Import a save so the encounter run can resolve these rules against its inputs."
                }))
                .child(body),
        )
        .into_any_element()
}

/// The chips above the editor: one removable tag per active rule.
pub fn chips(state: &AppState, cx: &mut Context<AppState>) -> Option<AnyElement> {
    let chips = state.combat.overrides.chips();
    if chips.is_empty() {
        return None;
    }
    let entity = cx.entity();
    let summary = chips
        .iter()
        .map(|(_, line)| line.as_str())
        .collect::<Vec<_>>()
        .join(" · ");
    let mut row = div()
        .id("combat-override-chips")
        .role(Role::Label)
        .aria_label(summary)
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2();
    for (id, line) in chips {
        let remove = entity.clone();
        let key = id.clone();
        row = row.child(
            Tag::new(
                SharedString::from(format!("combat-override-chip-{id}")),
                SharedString::from(line),
            )
            .tone(Tone::Accent)
            .on_remove(move |_window, cx| {
                remove.update(cx, |state, cx| {
                    state.remove_combat_override(&key, cx);
                });
            }),
        );
    }
    Some(
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(cx.theme().colors.fg_muted)
                    .child("Overrides"),
            )
            .child(row)
            .into_any_element(),
    )
}

/// One mechanic's row: what it is, its group, its mode and its value.
fn mechanic_row(
    state: &AppState,
    toggle: &overrides::Toggle,
    cx: &mut Context<AppState>,
) -> [AnyElement; 4] {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let id = toggle.id.clone();
    let rule = state.combat.overrides.rule(&toggle.id);
    let mode = rule.map_or("saved", |rule| rule.mode.id());
    let valued = matches!(rule.map(|rule| rule.mode), Some(Mode::Set | Mode::Scale));

    let mechanic = div()
        .id(SharedString::from(format!("combat-override-{}", toggle.id)))
        .role(Role::Label)
        .aria_label(format!("{}: {}", toggle.group, toggle.label))
        .flex()
        .flex_col()
        .gap_1()
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(12.0))
                        .font_weight(FontWeight::MEDIUM)
                        .child(SharedString::from(toggle.label.clone())),
                )
                .when(rule.is_some(), |row| {
                    row.child(Badge::new("override").tone(Tone::Accent))
                }),
        )
        .child(
            div()
                .text_size(px(11.0))
                .text_color(muted)
                .child(SharedString::from(toggle.desc.clone())),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(muted)
                .child(SharedString::from(toggle.keys.join(", "))),
        )
        .into_any_element();

    let group = div()
        .text_size(px(11.0))
        .text_color(muted)
        .child(SharedString::from(toggle.group.clone()))
        .into_any_element();

    let mut choices = vec![Choice::new("saved", "Saved")];
    for mode in toggle.modes() {
        choices.push(Choice::new(mode.id(), mode.label()));
    }
    let switch = entity.clone();
    let switch_id = id.clone();
    let modes = Select::new(
        SharedString::from(format!("combat-override-{}-mode", toggle.id)),
        choices,
    )
    .label(format!("{} mode", toggle.label))
    .selected(SharedString::from(mode))
    .on_change(move |value, _window, cx| {
        let mode = Mode::ALL
            .into_iter()
            .find(|mode| mode.id() == value.as_ref());
        switch.update(cx, |state, cx| {
            state.set_combat_override_mode(&switch_id, mode, cx);
        });
    });

    let value = if valued {
        let commit = entity.clone();
        let commit_id = id.clone();
        NumberInput::new(
            SharedString::from(format!("combat-override-{id}-value")),
            rule.and_then(|rule| rule.value).unwrap_or(0.0),
        )
        .label(format!("{} value", toggle.label))
        .precision(4)
        .step(0.1)
        .on_commit(move |value, _window, cx| {
            commit.update(cx, |state, cx| {
                state.set_combat_override_value(&commit_id, value, cx);
            });
        })
        .into_any_element()
    } else {
        div()
            .text_size(px(12.0))
            .text_color(muted)
            .child("\u{2014}")
            .into_any_element()
    };

    [mechanic, group, modes.into_any_element(), value]
}

/// The group filter: every group, then each catalog group.
fn group_select(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut choices = vec![Choice::new("", "All groups")];
    for group in overrides::groups() {
        choices.push(Choice::new(group.clone(), group.clone()));
    }
    Select::new("combat-overrides-group", choices)
        .label("Mechanic group")
        .selected(SharedString::from(state.combat.override_group.clone()))
        .on_change(move |value, _window, cx| {
            let group = value.to_string();
            entity.update(cx, |state, cx| {
                state.set_combat_override_group(&group, cx);
            });
        })
        .into_any_element()
}

/// The "only changed" filter.
fn only_active_check(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Checkbox::new(
        "combat-overrides-only-active",
        state.combat.override_only_active,
    )
    .label("Only changed")
    .on_change(move |checked, _window, cx| {
        entity.update(cx, |state, cx| {
            state.set_combat_overrides_only_active(checked, cx);
        });
    })
    .into_any_element()
}
