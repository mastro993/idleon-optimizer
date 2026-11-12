//! The inventory grid: the game's 39 ninja slots, the Gold charm markers and the
//! swap into the selected loadout slot.
//!
//! Ported from the inventory half of `sneaking-calc.html`: the 39-slot grid, the
//! mark on the Gold charm the game's cache actually selects, and the
//! `NJbonusPerms` scan behind it. The grid is read-only unless a loadout slot
//! is selected, and a swap moves the two items rather than cloning them.

use crate::state::AppState;
use crate::ui::assets::asset_image;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_sneaking::{Loadout, SlotKind};

/// The inventory card: the selected target, the unequip action and the grid.
pub(crate) fn card(state: &AppState, loadout: &Loadout, cx: &mut Context<AppState>) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let target = state.sneaking_state.target(loadout.players());
    let selected = target.map(|(twin, slot)| {
        let name = loadout
            .twin(twin)
            .map_or_else(String::new, |entry| entry.name.clone());
        let slot = SlotKind::of(slot).unwrap_or(SlotKind::Hat);
        format!("{name} · {}", slot.label())
    });
    let unequip_entity = cx.entity();
    let filled = target.is_some_and(|(twin, slot)| {
        loadout.twin(twin).is_some_and(|entry| {
            !entry
                .slot(SlotKind::of(slot).unwrap_or(SlotKind::Hat))
                .is_blank()
        })
    });
    let action = div()
        .flex()
        .items_center()
        .gap_2()
        .when_some(selected, |row, label| {
            row.child(Badge::new(label).tone(Tone::Accent))
        })
        .when(filled, |row| {
            row.child(Button::new("sneak-unequip", "Unequip").on_click(
                move |_event, _window, cx| {
                    unequip_entity.update(cx, |state, cx| state.sneak_unequip(cx));
                },
            ))
        });
    let mut grid = div()
        .id("sneaking-inventory")
        .grid()
        .grid_cols(13u16)
        .gap_1();
    let gold = loadout.gold_bonuses();
    for row in 0..loadout.inventory().len() {
        let item = &loadout.inventory()[row];
        let best = !item.is_blank()
            && item.def().is_some_and(|def| {
                gold.selected(def.bonus_type)
                    .is_some_and(|pick| pick.row == row)
            });
        grid = grid.child(slot(state, loadout, row, best, cx));
    }
    Card::new()
        .header(
            CardHeader::new("Inventory")
                .description(
                    "The game's 39 ninja slots. Pick a loadout slot above, then the charm, weapon or \
                     hat to swap in here; the two items trade places.",
                )
                .action(action),
        )
        .child(grid)
        .child(
            div()
                .text_size(px(10.0))
                .text_color(colors.fg_muted)
                .child(Caption::new(
                    "Green border and the “best” mark = the Gold charm the game's cache selects \
                     for its bonus type, symbols included. It is not necessarily the \
                     highest-level copy.",
                )),
        )
        .into_any_element()
}

fn slot(
    state: &AppState,
    loadout: &Loadout,
    row: usize,
    best: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let item = &loadout.inventory()[row];
    let symbol = loadout.inventory_symbol(row);
    let target = state.sneaking_state.target(loadout.players());
    let entity = cx.entity();
    let mut element = div()
        .id(SharedString::from(format!("sneak-inv-{row}")))
        .role(Role::Button)
        .tab_index(0)
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_0p5()
        .w(px(52.0))
        .h(px(56.0))
        .p_1()
        .rounded(px(6.0))
        .border_1()
        .border_color(if best { colors.success } else { colors.border })
        .bg(colors.surface)
        .hover(|style| style.bg(colors.hover))
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.sneak_equip(row, cx));
        });
    if item.is_blank() {
        return element
            .aria_label(format!("Inventory {}: empty", row + 1))
            .child(
                div()
                    .text_size(px(9.0))
                    .text_color(colors.fg_disabled)
                    .child(format!("{}", row + 1)),
            )
            .into_any_element();
    }
    let info = idleon_sneaking::equip_info(item);
    let tooltip = match &info {
        Some(info) => format!(
            "Inventory {}\n{}\n+{}\n{}\n{}\nSymbol +{}%",
            row + 1,
            info.name,
            item.level,
            info.label,
            info.detail,
            50.0 * symbol
        ),
        None => format!("Inventory {}\n{}", row + 1, item.key),
    };
    let fits = target.is_some_and(|(twin, slot)| {
        let slot = SlotKind::of(slot).unwrap_or(SlotKind::Hat);
        loadout.check_slot(twin, slot, item).is_ok()
    });
    element = element
        .aria_label(format!(
            "Inventory {}: {} +{}",
            row + 1,
            item.name(),
            item.level
        ))
        .tooltip(Tooltip::text(tooltip))
        .text_color(if fits { colors.fg } else { colors.fg_muted })
        .when(fits, |slot| slot.border_color(colors.accent));
    element
        .child(asset_image(
            format!("sneak-inv-art-{row}"),
            &state.assets,
            &format!("sneaking/{}.png", item.key),
            px(20.0),
            cx,
        ))
        .child(
            div()
                .text_size(px(9.0))
                .font_weight(gpui::FontWeight::MEDIUM)
                .child(format!("+{}", item.level)),
        )
        .when(best, |slot| {
            slot.child(
                div()
                    .text_size(px(8.0))
                    .text_color(colors.success)
                    .child("best"),
            )
        })
        .into_any_element()
}
