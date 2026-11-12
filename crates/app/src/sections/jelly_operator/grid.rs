//! The 18×10 Jelly Operator board: its slots, cells and obstruction.
//!
//! Ports the page's `gridHtml`: the slot classes for locked and obstruction
//! slots, the footprint joins, the cell sprites placed over their footprint
//! area and the obstruction sprite the page draws at column 7, row 3. Slots are
//! buttons, so a click places the picked palette cell or removes the cell under
//! the pointer. The layout tab and the simulator share the board and its
//! palette; only the unlocked slots, the obstruction and the click handler
//! differ.

use crate::state::AppState;
use crate::ui::assets::asset_image_sized;
use ely_gpui_component::primitives::Tooltip;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_jelly::PROXIMITY_ANCHORS;
use idleon_jelly::board::{self, Layout};
use idleon_jelly::data;
use idleon_ui::format;

/// Where a board click goes: the layout tab's editable save board or the
/// simulator's sandbox.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BoardAction {
    Layout,
    Simulator,
}

/// The page's cell pitch and grid padding, in pixels (`JELLY_CELL_PITCH`).
const PITCH: f32 = 37.0;
const PADDING: f32 = 4.0;
/// The game's sprite sizes per cell type (`CELL_SPRITE_SIZES`).
const SPRITE_SIZES: [(f32, f32); 8] = [
    (36.0, 36.0),
    (73.0, 36.0),
    (36.0, 109.0),
    (110.0, 110.0),
    (73.0, 73.0),
    (36.0, 36.0),
    (147.0, 147.0),
    (184.0, 183.0),
];
/// The obstruction sprite is the 50 px source drawn at 300% (`JELLY_OBSTRUCTION_SIZE`).
const OBSTRUCTION_SIZE: f32 = 150.0;

/// The upstream `.t0..t7` cell colours, background and border.
const CELL_COLORS: [(&str, &str); 8] = [
    ("#96767b", "#f8eadc"),
    ("#487154", "#edf9b3"),
    ("#ab4a42", "#ffcc97"),
    ("#8e4ba1", "#ffb7cf"),
    ("#536ab8", "#aefeff"),
    ("#7c5fd7", "#f3cdff"),
    ("#a86d36", "#fffeae"),
    ("#687080", "#e9fcff"),
];

/// What one board view draws: the layout, its sandbox terms and where a click
/// goes. `prefix` names its elements (`<prefix>-board`, `<prefix>-slot-…`), so
/// the layout editor and the simulator never collide.
pub(crate) struct BoardView<'a> {
    pub(crate) prefix: &'static str,
    pub(crate) layout: &'a Layout,
    pub(crate) unlocked: &'a [bool],
    pub(crate) obstruction: usize,
    pub(crate) selected: Option<usize>,
    pub(crate) action: BoardAction,
}

/// The board with its cells, clickable slots and obstruction sprite.
pub(crate) fn board(
    state: &AppState,
    view: BoardView<'_>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let BoardView {
        prefix,
        layout,
        unlocked,
        obstruction,
        selected,
        action,
    } = view;
    let data = data();
    let occupied = board::occupancy(layout);
    let colors = &cx.theme().colors;
    let entity = cx.entity();
    let width = data.cols as f32 * PITCH + PADDING * 2.0;
    let height = data.rows as f32 * PITCH + PADDING * 2.0;

    let mut slots = Vec::with_capacity(data.size());
    for slot in 0..data.size() {
        let cell = occupied.get(&slot).copied();
        let obstruction = data.is_obstruction(slot);
        let open = unlocked.get(slot).copied().unwrap_or(false);
        let column = slot % data.cols;
        let row = slot / data.cols;
        let (background, border) = match cell {
            Some((_, cell_type)) => (
                format::hex(CELL_COLORS[cell_type].0),
                format::hex(CELL_COLORS[cell_type].1),
            ),
            None if obstruction => (format::hex("#4b2638"), format::hex("#a74462")),
            None if !open => (format::hex("#171c2e"), format::hex("#262d43")),
            None => (format::hex("#171c2e"), format::hex("#262d43")),
        };
        let (label, detail) = match cell {
            Some((anchor, cell_type)) => (
                data.cell_name(cell_type).to_string(),
                format!("cell anchored at slot {anchor}"),
            ),
            None if obstruction => (
                "Obstruction".to_string(),
                format!("slot {slot} is blocked by the obstruction"),
            ),
            None if !open => ("Locked".to_string(), format!("slot {slot} is locked")),
            None => (
                "Empty".to_string(),
                format!("slot {slot}, column {column}, row {row}"),
            ),
        };
        let aria = format!("Slot {slot}: {label}, {detail}");
        let hover = format!("Slot {slot} · {label}");
        let tint = cell
            .map(|(_, cell_type)| format::hex(CELL_COLORS[cell_type].0))
            .unwrap_or(colors.border);
        let entity = entity.clone();
        slots.push(
            div()
                .id(format!("{prefix}-slot-{slot}"))
                .role(Role::Button)
                .aria_label(aria.clone())
                .absolute()
                .left(px(PADDING + column as f32 * PITCH))
                .top(px(PADDING + row as f32 * PITCH))
                .w(px(PITCH))
                .h(px(PITCH))
                .border_1()
                .border_color(border)
                .bg(background)
                .when(cell.is_none() && !open && !obstruction, |slot| {
                    slot.opacity(0.35)
                })
                .when(PROXIMITY_ANCHORS.contains(&slot), |slot| {
                    slot.border_color(colors.success)
                })
                .when(cell.is_some_and(|(anchor, _)| anchor == slot), |slot| {
                    // The core slot carries a small marker, as the page's `.core` does.
                    slot.child(
                        div()
                            .absolute()
                            .left(px(1.0))
                            .top(px(1.0))
                            .w(px(5.0))
                            .h(px(5.0))
                            .rounded(px(2.5))
                            .bg(tint),
                    )
                })
                .when_some(cell, |slot, (_, cell_type)| {
                    slot.child(
                        div()
                            .absolute()
                            .left(px(PITCH / 2.0 - 4.0))
                            .top(px(PITCH / 2.0 - 4.0))
                            .w(px(8.0))
                            .h(px(8.0))
                            .rounded(px(4.0))
                            .bg(format::hex(CELL_COLORS[cell_type].1)),
                    )
                })
                .tooltip(Tooltip::text(hover))
                .on_click(move |_event, _window, cx| {
                    let entity = entity.clone();
                    entity.update(cx, |state, cx| match action {
                        BoardAction::Layout => state.jelly_click_slot(slot, cx),
                        BoardAction::Simulator => state.jelly_sim_click_slot(slot, cx),
                    });
                })
                .into_any_element(),
        );
    }

    let mut sprites = Vec::with_capacity(layout.len() + 1);
    for (anchor, cell_type) in layout {
        if *cell_type >= SPRITE_SIZES.len() {
            continue;
        }
        let Some(slots) = board::footprint(*anchor, *cell_type) else {
            continue;
        };
        let xs: Vec<usize> = slots.iter().map(|slot| slot % data.cols).collect();
        let ys: Vec<usize> = slots.iter().map(|slot| slot / data.cols).collect();
        let min_x = xs.iter().copied().min().unwrap_or(0);
        let max_x = xs.iter().copied().max().unwrap_or(0);
        let min_y = ys.iter().copied().min().unwrap_or(0);
        let max_y = ys.iter().copied().max().unwrap_or(0);
        let (sprite_width, sprite_height) = SPRITE_SIZES[*cell_type];
        let area_width = (max_x - min_x + 1) as f32 * PITCH;
        let area_height = (max_y - min_y + 1) as f32 * PITCH;
        let left = PADDING + min_x as f32 * PITCH + (area_width - sprite_width) / 2.0;
        let top = PADDING + min_y as f32 * PITCH + (area_height - sprite_height) / 2.0;
        sprites.push(
            div()
                .id(format!("{prefix}-sprite-{anchor}"))
                .absolute()
                .left(px(left))
                .top(px(top))
                .child(asset_image_sized(
                    format!("{prefix}-sprite-image-{anchor}"),
                    &state.assets,
                    &format!("jelly-operator/JellyUnit{cell_type}.png"),
                    px(sprite_width),
                    px(sprite_height),
                    cx,
                ))
                .into_any_element(),
        );
    }
    let obstruction = obstruction.min(71);
    let obstruction_name = data
        .reward(obstruction)
        .map(|reward| reward.name.clone())
        .unwrap_or_else(|| format!("Obstruction {}", obstruction + 1));
    sprites.push(
        div()
            .id(format!("{prefix}-obstruction-sprite"))
            .absolute()
            .left(px(PADDING
                + 7.0 * PITCH
                + (4.0 * PITCH - OBSTRUCTION_SIZE) / 2.0))
            .top(px(PADDING
                + 3.0 * PITCH
                + (4.0 * PITCH - OBSTRUCTION_SIZE) / 2.0))
            .tooltip(Tooltip::text(obstruction_name))
            .child(asset_image_sized(
                format!("{prefix}-obstruction-image"),
                &state.assets,
                &format!("jelly-operator/JellyOp{obstruction}.png"),
                px(OBSTRUCTION_SIZE),
                px(OBSTRUCTION_SIZE),
                cx,
            ))
            .into_any_element(),
    );

    div()
        .id(format!("{prefix}-board"))
        .role(Role::Group)
        .aria_label(format!(
            "Jelly Operator board, {} cells, {} slots unlocked, palette pick {}",
            layout.len(),
            unlocked.iter().filter(|open| **open).count(),
            selected.map_or("remove mode", |cell_type| data.cell_name(cell_type))
        ))
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(6.0))
        .bg(format::hex("#0a1020"))
        .children(slots)
        .children(sprites)
        .into_any_element()
}

/// The upstream legend: one chip per unlocked cell type.
pub(crate) fn legend(state: &AppState) -> AnyElement {
    let Some(jelly) = state.jelly.as_ref() else {
        return div().into_any_element();
    };
    let owned = jelly.units_owned();
    div()
        .id("jelly-legend")
        .flex()
        .flex_wrap()
        .gap_1()
        .children((0..owned).map(|cell_type| {
            let (background, border) = CELL_COLORS[cell_type.min(7)];
            div()
                .id(("jelly-legend-cell", cell_type))
                .px_2()
                .py_0p5()
                .rounded(px(4.0))
                .border_1()
                .border_color(format::hex(border))
                .bg(format::hex(background))
                .text_size(px(11.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(data().cell_name(cell_type).to_string())
                .into_any_element()
        }))
        .into_any_element()
}

/// A slot tint for one cell type, shared with the palette.
pub(crate) fn cell_color(cell_type: usize) -> (Hsla, Hsla) {
    let (background, border) = CELL_COLORS[cell_type.min(7)];
    (format::hex(background), format::hex(border))
}

/// The eight cell types, with the ones the save has not unlocked disabled.
///
/// The layout tab and the simulator place the same cells, so they share one
/// builder; `prefix` names its buttons (`<prefix>-<type>`).
pub(crate) fn palette(
    state: &AppState,
    owned: usize,
    selected: Option<usize>,
    prefix: &'static str,
    cx: &mut Context<AppState>,
    on_pick: impl Fn(usize, &mut AppState, &mut Context<AppState>) + Clone + 'static,
) -> AnyElement {
    let data = data();
    let entity = cx.entity();
    let buttons = (0..8).map(|cell_type| {
        let unlocked = cell_type < owned;
        let (background, border) = cell_color(cell_type);
        let entity = entity.clone();
        let on_pick = on_pick.clone();
        div()
            .id((prefix, cell_type))
            .role(Role::Button)
            .aria_label(format!(
                "{}{}",
                data.cell_name(cell_type),
                if unlocked { "" } else { ", locked" }
            ))
            .flex()
            .items_center()
            .gap_2()
            .px_2()
            .py_1()
            .rounded(px(6.0))
            .border_1()
            .border_color(if selected == Some(cell_type) {
                format::hex("#00d4ff")
            } else {
                border
            })
            .bg(background)
            .when(!unlocked, |button| button.opacity(0.35))
            .when(unlocked, |button| {
                button.on_click(move |_event, _window, cx| {
                    let entity = entity.clone();
                    let on_pick = on_pick.clone();
                    entity.update(cx, |state, cx| on_pick(cell_type, state, cx));
                })
            })
            .child(asset_image_sized(
                format!("{prefix}-image-{cell_type}"),
                &state.assets,
                &format!("jelly-operator/JellyUnit{cell_type}.png"),
                px(30.0),
                px(30.0),
                cx,
            ))
            .child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_size(px(11.0))
                            .font_weight(gpui::FontWeight::BOLD)
                            .child(data.cell_name(cell_type).to_string()),
                    )
                    .child(
                        div()
                            .text_size(px(9.0))
                            .text_color(cx.theme().colors.fg_muted)
                            .child(format!("{} slot(s)", data.footprint(cell_type).len())),
                    ),
            )
            .into_any_element()
    });
    div()
        .id(format!("{prefix}-choices"))
        .flex()
        .flex_wrap()
        .gap_2()
        .children(buttons)
        .into_any_element()
}
