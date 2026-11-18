//! The layout tab: the cell palette, the board, the legend and its metrics.

use super::grid;
use super::parts::{figure, strips};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_jelly::Jelly;
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP};

pub(crate) fn layout_editor(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let layout = &state.jelly_state.layout;
    let saved = *layout == jelly.layout;
    let entity = cx.entity();
    let reset = Button::new("jelly-reset")
        .label("Reset board")
        .secondary()
        .small()
        .icon(IconName::RotateCcw)
        .disabled(saved)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.jelly_reset_layout(cx));
        });
    let unlocked = &state.jelly_state.save.unlocked;
    let board = grid::board(
        state,
        grid::BoardView {
            prefix: "jelly",
            layout,
            unlocked,
            highlight: &[],
            obstruction: jelly.progress.obstruction,
            selected: Some(state.jelly_state.selected),
            action: grid::BoardAction::Layout,
        },
        cx,
    );
    div()
        .id("jelly-layout")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(palette(state, cx, jelly))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Board")
                        .description(format!(
                            "{} cells placed · {} slot(s) unlocked · the save's board, editable",
                            layout.len(),
                            unlocked.iter().filter(|open| **open).count()
                        ))
                        .action(reset),
                )
                .child(grid::legend(state))
                .child(board)
                .child(message(state))
                .child(stats(&state.jelly_state.board.metrics)),
        )
        .into_any_element()
}

/// The eight cell types, with the ones the save has not unlocked disabled.
fn palette(state: &AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let buttons = grid::palette(
        state,
        jelly.units_owned(),
        Some(state.jelly_state.selected),
        "jelly-palette",
        cx,
        |cell_type, state, cx| state.jelly_select(cell_type, cx),
    );
    Card::new()
        .header(CardHeader::new("Cell palette").description(
            "Pick a cell, then click a slot to place it. Click a placed cell to remove it.",
        ))
        .child(buttons)
        .into_any_element()
}

/// The last placement answer.
fn message(state: &AppState) -> AnyElement {
    let message = &state.jelly_state.message;
    if message.is_empty() {
        return div().into_any_element();
    }
    div()
        .id("jelly-message")
        .role(Role::Label)
        .aria_label(message.clone())
        .child(Caption::new(message.clone()))
        .into_any_element()
}

/// The layout stats shown under the board.
fn stats(metrics: &idleon_jelly::Metrics) -> AnyElement {
    if !metrics.valid {
        return div()
            .id("jelly-layout-invalid")
            .child(Caption::new(metrics.reason.clone()))
            .into_any_element();
    }
    strips(
        "jelly-layout-stats",
        vec![
            figure(
                "jelly-damage-passive",
                "Damage passive",
                metrics.damage_passive,
                0,
            )
            .caption("Cell damage passives"),
            figure(
                "jelly-speed-passive",
                "Speed passive",
                metrics.speed_passive,
                0,
            )
            .caption("Cell speed passives"),
            figure(
                "jelly-virus-multiplier",
                "Virus multiplier",
                metrics.virus_multiplier,
                0,
            )
            .caption(format!(
                "{} infected footprint slot(s)",
                metrics.infected_slots
            )),
            figure("jelly-steady-dps", "Steady DPS", metrics.dps, 0).caption(format!(
                "{} attacks/s before projectile travel",
                format::number(metrics.attacks_per_second)
            )),
        ],
    )
}
