//! The layout tab: the cell palette, the board, the legend and its metrics.

use super::grid;
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_jelly::Jelly;
use idleon_jelly::metrics::MetricOptions;
use idleon_ui::format;

pub(crate) fn layout_editor(
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
                            idleon_jelly::board::unlocked_count(jelly)
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
                        .child(grid::board(
                            state,
                            grid::BoardView {
                                prefix: "jelly",
                                layout: &layout,
                                unlocked: &idleon_jelly::board::unlocked_slots(jelly),
                                highlight: &[],
                                obstruction: jelly.progress.obstruction,
                                selected: Some(state.jelly_state.selected),
                                action: grid::BoardAction::Layout,
                            },
                            cx,
                        ))
                        .child(message(state, cx))
                        .child(stats(&metrics)),
                ),
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
        .child(div().p_3().child(buttons))
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
fn stats(metrics: &idleon_jelly::Metrics) -> AnyElement {
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
