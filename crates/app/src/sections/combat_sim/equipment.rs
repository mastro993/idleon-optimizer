//! The equipment half of the loadout editor: the slot list and one slot's item.
//!
//! A slot list on the left, the chosen slot's item, its added stats and its
//! unique bonuses on the right. Every stat shows as base + added, and the pane
//! notes that a new item drops the old item's saved deltas, which is what its
//! `equipment` patch does.

use super::State;
use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, InteractiveElement, IntoElement, ParentElement, Role,
    SharedString, StatefulInteractiveElement, Styled, Window, div,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_combat::{self as combat, data, loadout::Slot};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, PANEL_GAP, PANEL_PADDING, TEXT_CAPTION, TEXT_MONO, Tone,
};

/// The equipment pane: every slot on the left, the selected slot on the right.
pub(crate) fn pane(
    state: &mut AppState,
    breakpoint: Breakpoint,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let selected = state
        .combat
        .selected
        .min(state.combat.loadout.slots.len().saturating_sub(1));
    let list = slot_list(&state.combat, window, cx);
    let detail = slot_detail(&mut state.combat, selected, window, cx);
    layout::split_pane("combat-gear", breakpoint, list, detail, gpui::px(330.0))
}

/// The slot list: armor first, then the tool row.
fn slot_list(state: &State, window: &Window, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let loadout = &state.loadout;
    let mut groups = div().flex().flex_col().gap(PANEL_GAP);
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
            let active = cx.theme().list_active;
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
                    .rounded(cx.theme().radius)
                    .when(selected, |row| row.bg(active))
                    .cursor_pointer()
                    .on_click(move |_event, _window, cx| {
                        selection.update(cx, |state, cx| state.set_combat_slot(index, cx));
                    })
                    .child(
                        div()
                            .w(gpui::px(110.0))
                            .flex_none()
                            .text_size(TEXT_CAPTION)
                            .text_color(muted)
                            .child(slot_name.clone()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .text_size(TEXT_MONO)
                            .font_weight(if data::is_empty(&slot.item) {
                                FontWeight::NORMAL
                            } else {
                                FontWeight::MEDIUM
                            })
                            .text_color(if data::is_empty(&slot.item) {
                                muted
                            } else {
                                cx.theme().foreground
                            })
                            .child(item_name),
                    )
                    .when(!issues.is_empty(), |row| row.child(check_tag())),
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
                        .text_size(TEXT_CAPTION)
                        .text_color(muted)
                        .child(format!(
                            "{} / {}",
                            loadout.totals().equipped,
                            loadout.slots.len()
                        )),
                ),
        )
        .child(
            div()
                .h(layout::table_height(f32::from(
                    window.viewport_size().height,
                )))
                .p(PANEL_GAP)
                .child(groups)
                .overflow_y_scrollbar()
                .id("combat-slot-scroll"),
        )
        .into_any_element()
}

/// The selected slot's item picker, stats and unique bonuses.
fn slot_detail(
    state: &mut State,
    selected: usize,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let Some(slot) = state.loadout.slot_at(selected) else {
        return Card::new()
            .header(CardHeader::new("Slot"))
            .child(
                div()
                    .p(PANEL_PADDING)
                    .child(Caption::new("No equipment slots.")),
            )
            .into_any_element();
    };
    let issues = state.loadout.slot_issues(slot.row, slot.slot);
    let current = slot.item.clone();
    let row = slot.row;
    let slot_index = slot.slot;
    // The catalog is walked once per slot and item, not on every frame.
    let key = (row, slot_index, current.clone());
    let controls = &mut state.controls;
    if controls.items.as_ref().map(|(shown, _)| shown) != Some(&key) {
        controls.items = Some((key, item_choices(slot)));
    }
    let choices = controls
        .items
        .as_ref()
        .map(|(_, choices)| choices.clone())
        .unwrap_or_default();
    controls.item.show(choices, Some(&current), window, cx);
    let picker = Select::new(controls.item.state())
        .id("combat-item-select")
        .accessibility_label("Item")
        .placeholder("(empty)")
        .small();

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
                    .text_size(TEXT_CAPTION)
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
                        .when(!issues.is_empty(), |row| row.child(check_tag())),
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
                .p(PANEL_PADDING)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(PANEL_GAP)
                        .child(div().flex_1().min_w_0().child(picker))
                        .child(
                            Button::new("combat-unequip")
                                .label("Unequip")
                                .small()
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
                            .text_size(TEXT_CAPTION)
                            .text_color(cx.theme().danger)
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
                                .text_size(TEXT_CAPTION)
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
        .text_size(TEXT_MONO)
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
fn item_choices(slot: &Slot) -> Vec<Choice<String>> {
    let mut choices = vec![Choice::new("Blank".to_string(), "(empty)")];
    for (key, def) in combat::options(slot.row, slot.slot) {
        choices.push(Choice::new(key.to_string(), data::clean_name(&def.name)));
    }
    if slot.def().is_none() && !data::is_empty(&slot.item) {
        choices.insert(1, Choice::new(slot.item.clone(), slot.item.clone()));
    }
    choices
}

/// The mark on a slot the simulator cannot read as saved.
fn check_tag() -> Tag {
    Tag::new()
        .with_variant(Tone::Danger.into())
        .outline()
        .small()
        .child("check")
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
