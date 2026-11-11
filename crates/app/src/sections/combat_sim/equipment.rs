//! The equipment half of the loadout editor: the slot list and one slot's item.
//!
//! Ported from the upstream `renderGear`/`gearDetail` pane (`js/ui/combat-sim-setup-ui.js`):
//! a slot list on the left, the chosen slot's item, its added stats and its
//! unique bonuses on the right. Upstream shows every stat as base + added; this
//! pane shows the same split and notes that a new item drops the old item's
//! saved deltas, which is what its `equipment` patch does.

use super::State;
use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement, Styled, div,
};
use idleon_core::combat::{self, data, loadout::Slot};

/// The equipment pane: every slot on the left, the selected slot on the right.
pub fn pane(
    state: &mut AppState,
    breakpoint: Breakpoint,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let (list, detail) = {
        let loadout = &state.combat.loadout;
        let selected = state
            .combat
            .selected
            .min(loadout.slots.len().saturating_sub(1));
        (
            slot_list(&state.combat, cx),
            slot_detail(&state.combat, selected, cx),
        )
    };
    layout::split_pane("combat-gear", breakpoint, list, detail, gpui::px(330.0))
}

/// The slot list: armor first, then the tool row.
fn slot_list(state: &State, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let loadout = &state.loadout;
    let mut groups = div().flex().flex_col().gap_2();
    for (label, row) in [("Armor", 0usize), ("Tools", 1)] {
        let mut rows = div()
            .id(SharedString::from(format!("combat-slots-{row}")))
            .flex()
            .flex_col();
        for (index, slot) in loadout
            .slots
            .iter()
            .enumerate()
            .filter(|(_, slot)| slot.row == row)
        {
            let issues = loadout.slot_issues(slot.row, slot.slot);
            let selected = state.selected == index;
            let accent = cx.theme().colors.accent;
            let selection = entity.clone();
            let slot_name = slot.name();
            let item_name = slot.item_name();
            rows = rows.child(
                div()
                    .id(SharedString::from(format!(
                        "combat-slot-{}-{}",
                        slot.row, slot.slot
                    )))
                    .role(Role::Button)
                    .aria_label(format!("{slot_name}: {item_name}"))
                    .flex()
                    .items_center()
                    .gap_2()
                    .h(gpui::px(26.0))
                    .px_1()
                    .rounded(gpui::px(4.0))
                    .when(selected, |row| row.bg(accent.opacity(0.12)))
                    .cursor_pointer()
                    .on_click(move |_event, _window, cx| {
                        selection.update(cx, |state, cx| state.set_combat_slot(index, cx));
                    })
                    .child(
                        div()
                            .w(gpui::px(110.0))
                            .flex_none()
                            .text_size(gpui::px(11.0))
                            .text_color(muted)
                            .child(slot_name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(gpui::px(12.0))
                            .font_weight(if data::is_empty(&slot.item) {
                                FontWeight::NORMAL
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if data::is_empty(&slot.item) {
                                muted
                            } else {
                                cx.theme().colors.fg
                            })
                            .child(item_name),
                    )
                    .when(!issues.is_empty(), |row| {
                        row.child(Badge::new("check".to_string()).tone(Tone::Danger))
                    }),
            );
        }
        groups = groups.child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(label).text_color(muted))
                .child(rows),
        );
    }

    Card::new()
        .header(
            CardHeader::new("Equipment")
                .description("Click a slot to change its item")
                .action(
                    div()
                        .id("combat-equipped")
                        .role(Role::Label)
                        .aria_label(format!(
                            "{} of {} slots equipped",
                            loadout.totals().equipped,
                            loadout.slots.len()
                        ))
                        .text_size(gpui::px(11.0))
                        .text_color(muted)
                        .child(format!(
                            "{} / {}",
                            loadout.totals().equipped,
                            loadout.slots.len()
                        )),
                ),
        )
        .child(
            ScrollArea::new("combat-slot-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .p_2()
                .child(groups),
        )
        .into_any_element()
}

/// The selected slot's item picker, stats and unique bonuses.
fn slot_detail(state: &State, selected: usize, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(slot) = state.loadout.slot_at(selected) else {
        return Card::new()
            .header(CardHeader::new("Slot"))
            .child(div().p_3().child(Caption::new("No equipment slots.")))
            .into_any_element();
    };
    let issues = state.loadout.slot_issues(slot.row, slot.slot);
    let entity = cx.entity();
    let choices = item_choices(slot);
    let current = slot.item.clone();
    let row = slot.row;
    let slot_index = slot.slot;
    let picker = Select::new("combat-item-select", choices)
        .label("Item")
        .placeholder("(empty)")
        .selected(current.clone())
        .on_change(move |value, _window, cx| {
            entity.update(cx, |state, cx| {
                state.set_combat_item(row, slot_index, value.as_ref(), cx);
            });
        });

    let mut rows = div().flex().flex_col().gap_1();
    for stat in data::GEAR_STATS {
        let base = slot.base_stat(stat);
        let added = slot.added.get(stat).copied().unwrap_or(0.0);
        let total = base + added;
        if base == 0.0 && added == 0.0 {
            continue;
        }
        rows = rows.child(stat_row(stat, base, added, total, muted));
    }
    if slot.def().is_none() && !data::is_empty(&slot.item) {
        rows = rows.child(Caption::new(
            "This item is not in the catalog; no base stats.",
        ));
    }

    let mut unique = div().flex().flex_col().gap_1();
    let mut has_unique = false;
    for (label, value) in [
        ("Bonus 1", slot.def().and_then(|def| def.uq1.clone())),
        ("Bonus 2", slot.def().and_then(|def| def.uq2.clone())),
    ] {
        if let Some((stat, value)) = value {
            has_unique = true;
            unique = unique.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .text_size(gpui::px(11.0))
                    .child(Caption::new(label).text_color(muted))
                    .child(format!("{} {}", format_stat(&stat), value)),
            );
        }
    }

    Card::new()
        .header(
            CardHeader::new(slot.name())
                .description(slot.item_name())
                .action(
                    div()
                        .id("combat-slot-issues")
                        .role(Role::Label)
                        .aria_label(
                            issues
                                .iter()
                                .map(|issue| issue.detail.clone())
                                .collect::<Vec<_>>()
                                .join(" · "),
                        )
                        .when(!issues.is_empty(), |row| {
                            row.child(Badge::new("check".to_string()).tone(Tone::Danger))
                        }),
                ),
        )
        .child(
            div()
                .id("combat-slot-detail")
                .flex()
                .flex_col()
                .gap_3()
                .flex_1()
                .min_h_0()
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_2()
                        .child(div().flex_1().min_w_0().child(picker))
                        .child(
                            Button::new("combat-unequip", "Unequip")
                                .size(ControlSize::Sm)
                                .disabled(data::is_empty(&slot.item))
                                .on_click({
                                    let entity = cx.entity();
                                    move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.set_combat_item(row, slot_index, "Blank", cx);
                                        });
                                    }
                                }),
                        ),
                )
                .when(!issues.is_empty(), |panel| {
                    panel.children(issues.iter().map(|issue| {
                        div()
                            .text_size(gpui::px(11.0))
                            .text_color(cx.theme().colors.danger)
                            .child(issue.detail.clone())
                    }))
                })
                .child(
                    div()
                        .id("combat-slot-stats")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(gpui::px(11.0))
                                .text_color(muted)
                                .child(div().w(gpui::px(120.0)).child("Stat"))
                                .child(div().w(gpui::px(48.0)).child("Base"))
                                .child(div().w(gpui::px(48.0)).child("Added"))
                                .child("Total"),
                        )
                        .child(rows),
                )
                .when(has_unique, |panel| panel.child(unique))
                .child(Caption::new(
                    "Added stats are the save's upgrade deltas; equipping another item drops them.",
                )),
        )
        .into_any_element()
}

/// One stat line: name, base, added and total.
fn stat_row(stat: &str, base: f64, added: f64, total: f64, muted: gpui::Hsla) -> AnyElement {
    div()
        .id(SharedString::from(format!("combat-stat-{stat}")))
        .role(Role::Label)
        .aria_label(format!(
            "{}: base {base:.0}, added {added:.1}, total {total:.0}",
            data::stat_label(stat)
        ))
        .flex()
        .items_center()
        .gap_2()
        .h(gpui::px(22.0))
        .text_size(gpui::px(12.0))
        .child(
            div()
                .w(gpui::px(120.0))
                .flex_none()
                .child(data::stat_label(stat)),
        )
        .child(
            div()
                .w(gpui::px(48.0))
                .text_color(muted)
                .child(format!("{base:.0}")),
        )
        .child(
            div()
                .w(gpui::px(48.0))
                .text_color(muted)
                .child(if added == 0.0 {
                    "—".to_string()
                } else {
                    format!("+{added:.1}")
                }),
        )
        .child(
            div()
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{total:.0}")),
        )
        .into_any_element()
}

/// The picker's options: `(empty)`, every item the slot accepts, and the
/// current key when the catalog does not know it.
fn item_choices(slot: &Slot) -> Vec<Choice> {
    let mut choices = vec![Choice::new("Blank", "(empty)")];
    for (key, def) in combat::options(slot.row, slot.slot) {
        choices.push(Choice::new(key, data::clean_name(&def.name)));
    }
    if slot.def().is_none() && !data::is_empty(&slot.item) {
        choices.insert(1, Choice::new(slot.item.clone(), slot.item.clone()));
    }
    choices
}

/// A unique-bonus stat code as a label, keeping its leading percent.
fn format_stat(stat: &str) -> String {
    let text = stat.trim_start_matches(['%', '_']);
    let label = text.replace('_', " ");
    if stat.starts_with('%') {
        format!("% {label}")
    } else {
        label
    }
}
