//! The inventory grid: the game's 39 ninja slots, the Gold charm markers and the
//! swap into the selected loadout slot.
//!
//! The 39-slot grid, the mark on the Gold charm the game's cache actually
//! selects, and the `NJbonusPerms` scan behind it. The grid is read-only unless
//! a loadout slot is selected, and a swap moves the two items rather than
//! cloning them.

use super::controls::{Figures, SlotFigures};
use super::loadout::tag;
use crate::state::AppState;
use crate::ui::assets::asset_image;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, FontWeight, IntoElement, ParentElement, Role, SharedString, Styled, div,
    px,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_sneaking::{Loadout, SlotKind};
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_CAPTION, Tone};

/// The inventory card: the selected target, the unequip action and the grid.
pub(crate) fn card(
    state: &AppState,
    loadout: &Loadout,
    figures: &Figures,
    cx: &mut Context<AppState>,
) -> AnyElement {
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
        .when_some(selected, |row, label| row.child(tag(label, Tone::Accent)))
        .when(filled, |row| {
            row.child(
                Button::new("sneak-unequip")
                    .label("Unequip")
                    .small()
                    .on_click(move |_event, _window, cx| {
                        unequip_entity.update(cx, |state, cx| state.sneak_unequip(cx));
                    }),
            )
        });
    let mut grid = div()
        .id("sneaking-inventory")
        .grid()
        .grid_cols(13u16)
        .gap_1();
    for row in 0..loadout.inventory().len() {
        grid = grid.child(slot(state, loadout, figures, row, cx));
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
        .child(Caption::new(
            "Green border and the “best” mark = the Gold charm the game's cache selects for its \
             bonus type, symbols included. It is not necessarily the highest-level copy.",
        ))
        .into_any_element()
}

fn slot(
    state: &AppState,
    loadout: &Loadout,
    figures: &Figures,
    row: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (border, success, primary, background, hover, fg, muted, radius) = (
        theme.border,
        theme.success,
        theme.primary,
        theme.secondary,
        theme.secondary_hover,
        theme.foreground,
        theme.muted_foreground,
        theme.radius,
    );
    let item = &loadout.inventory()[row];
    let SlotFigures { best, fits, symbol } = figures.inventory[row];
    let entity = cx.entity();
    let element = div()
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
        .rounded(radius)
        .border_1()
        .border_color(if best { success } else { border })
        .bg(background)
        .hover(move |style| style.bg(hover))
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.sneak_equip(row, cx));
        });
    if item.is_blank() {
        return element
            .aria_label(format!("Inventory {}: empty", row + 1))
            .child(
                div()
                    .text_size(TEXT_CAPTION)
                    .text_color(muted)
                    .child(format!("{}", row + 1)),
            )
            .into_any_element();
    }
    let tooltip = SharedString::from(match idleon_sneaking::equip_info(item) {
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
    });
    element
        .aria_label(format!(
            "Inventory {}: {} +{}",
            row + 1,
            item.name(),
            item.level
        ))
        .tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
        .text_color(if fits { fg } else { muted })
        .when(fits, |slot| slot.border_color(primary))
        .child(asset_image(
            format!("sneak-inv-art-{row}"),
            &state.assets,
            &format!("sneaking/{}.png", item.key),
            px(20.0),
            cx,
        ))
        .child(
            div()
                .text_size(TEXT_CAPTION)
                .font_weight(FontWeight::MEDIUM)
                .child(format!("+{}", item.level)),
        )
        .when(best, |slot| {
            slot.child(
                div()
                    .text_size(TEXT_CAPTION)
                    .text_color(success)
                    .child("best"),
            )
        })
        .into_any_element()
}
