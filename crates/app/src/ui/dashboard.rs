//! Rates pane: the EXP breakdown and the observation board.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::IconButton;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::{game, sim};

pub fn dashboard_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let rate = state.rate();
    let points = state.grid_points();
    let cards = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("kpi-rate", "Research EXP/hr", rate.total)
                    .decimals(0)
                    .size(ely_gpui_component::theme::TextSize::Lg),
            )
            .icon(IconName::Zap)
            .caption("observations × multipliers"),
        )
        .child(
            KpiCard::new(Statistic::new("kpi-multi", "Multiplier", rate.multi).decimals(2))
                .icon(IconName::Layers)
                .caption("additive and grid bonuses"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-obs", "Observation EXP", rate.obs_base).decimals(1),
            )
            .icon(IconName::Gauge)
            .caption("before multipliers"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-points", "Free grid points", points.free as f64)
                    .decimals(0),
            )
            .icon(IconName::Target)
            .caption(format!("{} spent of {}", points.spent, points.earned)),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-shapes", "Shapes", state.simulation.placements.len() as f64)
                    .decimals(0),
            )
            .icon(IconName::Layers)
            .caption(format!("{} unlocked", state.shapes_owned())),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-mags", "Magnifiers", state.simulation.mags.len() as f64)
                    .decimals(0),
            )
            .icon(IconName::Gauge)
            .caption(format!("{} owned", state.magnifiers_owned())),
        );

    div()
        .id("dashboard")
        .flex()
        .flex_col()
        .gap_3()
        .size_full()
        .min_h_0()
        .child(cards)
        .child(observations_card(state, cx))
}

fn observations_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let level = state.simulation.research_level;
    let overlay = state.simulation.overlay();
    let kal_map = sim::kaleidoscope_map(&state.simulation.mags, game().observations.len());
    let kal_base = sim::kalei_base(&state.simulation.grid, &overlay, &state.context);
    let to_find = sim::observations_to_find(level, &state.simulation.observations.found);
    let entity = cx.entity();

    let mut table = Table::new(["#", "Observation", "Insight", "Magnifiers", "Insight/hr", ""]);
    for obs in 0..to_find.min(game().observations.len()) {
        let observation = &game().observations[obs];
        let found = state.simulation.observations.found[obs];
        let usable = sim::observation_usable(obs, level, &state.simulation.observations);
        let insight_level = state.simulation.observations.insight_level[obs];
        let insight_rate = sim::insight_rate(
            obs,
            &state.simulation,
            &overlay,
            &state.context,
            &kal_map,
            kal_base,
        );
        let mags = state
            .simulation
            .mags
            .iter()
            .filter(|mag| mag.slot == Some(obs))
            .count();
        let toggle = entity.clone();
        let bump = entity.clone();
        table = table.row([
            div().child(format!("{}", obs + 1)).into_any_element(),
            div()
                .flex()
                .flex_col()
                .child(observation.name.replace('_', " "))
                .child(
                    Caption::new(format!("needs research {}", observation.roll_req as u32)),
                )
                .into_any_element(),
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(format!("Lv {insight_level}"))
                .child(
                    IconButton::new(("obs-plus", obs), IconName::Plus)
                        .size(ControlSize::Sm)
                        .tooltip("Raise insight")
                        .on_click(move |_event, _window, cx| {
                            bump.update(cx, |state, cx| state.bump_insight(obs, 1, cx));
                        }),
                )
                .into_any_element(),
            div()
                .child(if usable {
                    format!("{mags}")
                } else if found {
                    "locked".to_string()
                } else {
                    "unfound".to_string()
                })
                .into_any_element(),
            div().child(format::rate(insight_rate)).into_any_element(),
            div()
                .child(
                    IconButton::new(("obs-toggle", obs), IconName::RefreshCw)
                        .size(ControlSize::Sm)
                        .tooltip("Toggle found")
                        .on_click(move |_event, _window, cx| {
                            toggle.update(cx, |state, cx| state.toggle_found(obs, cx));
                        }),
                )
                .into_any_element(),
        ]);
    }

    Card::new()
        .header(
            CardHeader::new("Observations")
                .description(format!(
                    "{to_find} of {} found at research level {level}",
                    game().observations.len()
                )),
        )
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(
                    ScrollArea::new("observations-scroll").child(
                        div()
                            .flex()
                            .flex_col()
                            .text_color(colors.fg)
                            .child(table),
                    ),
                ),
        )
        .h(px(520.0))
}
