//! The layout tab: the cell palette, the board, the legend and its metrics.

use super::grid;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use idleon_core::jelly::Jelly;
use idleon_core::jelly::data;
use idleon_core::jelly::metrics::MetricOptions;

pub fn layout_editor(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let layout = state.jelly_state.layout.clone();
    let metrics = jelly.layout_metrics(
        &layout,
        MetricOptions {
            fever: Some(jelly.progress.fever),
            ..MetricOptions::default()
        },
    );
    let saved = layout == jelly.layout;
    let entity = cx.entity();
    let reset = Button::new("jelly-reset", "Reset board")
        .variant(ButtonVariant::Subtle)
        .size(ControlSize::Sm)
        .icon(IconName::RotateCcw)
        .disabled(saved)
        .on_click(move |_event, _window, cx| {
            let entity = entity.clone();
            entity.update(cx, |state, cx| state.jelly_reset_layout(cx));
        });
    div()
        .id("jelly-layout")
        .flex()
        .flex_col()
        .gap_3()
        .child(palette(state, cx, jelly))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Board")
                        .description(format!(
                            "{} cells placed · {} slot(s) unlocked · the save's board, editable",
                            layout.len(),
                            idleon_core::jelly::board::unlocked_count(jelly)
                        ))
                        .action(reset),
                )
                .child(
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(grid::legend(state))
                        .child(grid::board(state, &layout, cx))
                        .child(message(state, cx))
                        .child(stats(&metrics)),
                ),
        )
        .into_any_element()
}

/// The eight cell types, with the ones the save has not unlocked disabled.
fn palette(state: &mut AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let owned = jelly.units_owned();
    let selected = state.jelly_state.selected;
    let data = data();
    let entity = cx.entity();
    let buttons = (0..8).map(|cell_type| {
        let unlocked = cell_type < owned;
        let (background, border) = grid::cell_color(cell_type);
        let entity = entity.clone();
        div()
            .id(("jelly-palette", cell_type))
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
            .border_color(if selected == cell_type {
                format::hex("#00d4ff")
            } else {
                border
            })
            .bg(background)
            .when(!unlocked, |button| button.opacity(0.35))
            .when(unlocked, |button| {
                button.on_click(move |_event, _window, cx| {
                    let entity = entity.clone();
                    entity.update(cx, |state, cx| state.jelly_select(cell_type, cx));
                })
            })
            .child(crate::ui::assets::asset_image_sized(
                format!("jelly-palette-image-{cell_type}"),
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
    Card::new()
        .header(CardHeader::new("Cell palette").description(
            "Pick a cell, then click a slot to place it. Click a placed cell to remove it.",
        ))
        .child(
            div()
                .id("jelly-palette")
                .p_3()
                .flex()
                .flex_wrap()
                .gap_2()
                .children(buttons),
        )
        .into_any_element()
}

/// The last placement answer.
fn message(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    if state.jelly_state.message.is_empty() {
        return div().into_any_element();
    }
    let muted = cx.theme().colors.fg_muted;
    div()
        .id("jelly-message")
        .role(Role::Label)
        .aria_label(state.jelly_state.message.clone())
        .child(Caption::new(state.jelly_state.message.clone()).text_color(muted))
        .into_any_element()
}

/// The layout stats the page shows under the board (`layoutStats`).
fn stats(metrics: &idleon_core::jelly::Metrics) -> AnyElement {
    if !metrics.valid {
        return div()
            .id("jelly-layout-invalid")
            .child(Caption::new(metrics.reason.clone()))
            .into_any_element();
    }
    div()
        .id("jelly-layout-stats")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(Statistic::new(
                "jelly-damage-passive",
                "Damage passive",
                metrics.damage_passive,
            ))
            .caption("Cell damage passives"),
        )
        .child(
            KpiCard::new(Statistic::new(
                "jelly-speed-passive",
                "Speed passive",
                metrics.speed_passive,
            ))
            .caption("Cell speed passives"),
        )
        .child(
            KpiCard::new(Statistic::new(
                "jelly-virus-multiplier",
                "Virus multiplier",
                metrics.virus_multiplier,
            ))
            .caption(format!(
                "{} infected footprint slot(s)",
                metrics.infected_slots
            )),
        )
        .child(
            KpiCard::new(Statistic::new(
                "jelly-steady-dps",
                "Steady DPS",
                metrics.dps,
            ))
            .caption(format!(
                "{} attacks/s before projectile travel",
                format::number(metrics.attacks_per_second)
            )),
        )
        .into_any_element()
}
