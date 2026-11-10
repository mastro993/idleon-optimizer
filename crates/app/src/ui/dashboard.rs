//! Rates pane: the EXP breakdown and the observation board.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::IconButton;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
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
            .caption("observations × multipliers"),
        )
        .child(
            KpiCard::new(Statistic::new("kpi-multi", "Multiplier", rate.multi).decimals(2))
                .caption("additive and grid bonuses"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-obs", "Observation EXP", rate.obs_base).decimals(1),
            )
            .caption("before multipliers"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-points", "Free grid points", points.free as f64)
                    .decimals(0),
            )
            .caption(format!("{} spent of {}", points.spent, points.earned)),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-shapes", "Shapes", state.simulation.placements.len() as f64)
                    .decimals(0),
            )
            .caption(format!("{} unlocked", state.shapes_owned())),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-mags", "Magnifiers", state.simulation.mags.len() as f64)
                    .decimals(0),
            )
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
    let (border, muted, surface) = {
        let palette = &cx.theme().colors;
        (palette.border, palette.fg_muted, palette.sunken)
    };
    let level = state.simulation.research_level;
    let to_find = sim::observations_to_find(level, &state.simulation.observations.found);
    let rows: Vec<usize> = (0..to_find.min(game().observations.len()))
        .filter(|obs| sim::observation_usable(*obs, level, &state.simulation.observations))
        .collect();
    let shown = rows.len();
    let entity = cx.entity();
    let overlay = state.simulation.overlay();
    let kal_map = sim::kaleidoscope_map(&state.simulation.mags, game().observations.len());
    let kal_base = sim::kalei_base(&state.simulation.grid, &overlay, &state.context);

    let half = rows.len().div_ceil(2);
    let mut columns = div().flex().gap_3().min_h_0();
    for chunk in rows.chunks(half.max(1)) {
        let mut column = div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(24.0))
                    .px_1()
                    .text_size(px(11.0))
                    .text_color(muted)
                    .child(div().w(px(20.0)).child("#"))
                    .child(div().flex_1().min_w_0().child("Observation"))
                    .child(div().w(px(44.0)).child("Insight"))
                    .child(div().w(px(24.0)).child("+"))
                    .child(div().w(px(34.0)).child("Mags"))
                    .child(div().w(px(56.0)).child("Ins/hr"))
                    .child(div().w(px(24.0)).child("")),
            );
        for obs in chunk {
            let obs = *obs;
            let observation = &game().observations[obs];
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
            let bump = entity.clone();
            let toggle = entity.clone();
            column = column.child(
                div()
                    .id(("obs-row", obs))
                    .flex()
                    .items_center()
                    .gap_1()
                    .h(px(30.0))
                    .px_1()
                    .border_b_1()
                    .border_color(border)
                    .text_size(px(12.0))
                    .child(div().w(px(20.0)).child(format!("{}", obs + 1)))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .child(observation.name.replace('_', " ")),
                    )
                    .child(div().w(px(44.0)).child(format!("Lv {insight_level}")))
                    .child(
                        IconButton::new(("obs-plus", obs), IconName::Plus)
                            .size(ControlSize::Sm)
                            .tooltip("Raise insight")
                            .on_click(move |_event, _window, cx| {
                                bump.update(cx, |state, cx| state.bump_insight(obs, 1, cx));
                            }),
                    )
                    .child(div().w(px(34.0)).child(format!("{mags}")))
                    .child(div().w(px(56.0)).child(format::rate(insight_rate)))
                    .child(
                        IconButton::new(("obs-toggle", obs), IconName::RefreshCw)
                            .size(ControlSize::Sm)
                            .tooltip("Toggle found")
                            .on_click(move |_event, _window, cx| {
                                toggle.update(cx, |state, cx| state.toggle_found(obs, cx));
                            }),
                    ),
            );
        }
        columns = columns.child(column);
    }

    Card::new()
        .header(CardHeader::new("Observations").description(format!(
            "{shown} usable of {to_find} found at research level {level}"
        )))
        .child(
            div()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .p_3()
                .child(div().h(px(1.0)).bg(surface))
                .child(columns),
        )
        .child(
            div().px_3().pb_2().text_size(px(11.0)).text_color(muted).child(
                "Insight/hr counts monocles only; magnifiers marked here sit on that observation.",
            ),
        )
}
