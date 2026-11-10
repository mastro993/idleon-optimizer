//! Rates pane: the EXP breakdown and the observation board.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, IconButton};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::forms::{Input, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_core::{game, sim};

pub fn rates_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let rate = state.rate();
    let points = state.grid_points();
    let cards = div()
        .id("kpi-row")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Research EXP/hr {} · multiplier {:.2}",
            format::rate(rate.total),
            rate.multi
        ))
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
            KpiCard::new(Statistic::new("kpi-obs", "Observation EXP", rate.obs_base).decimals(1))
                .caption("before multipliers"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-points", "Free grid points", points.free as f64).decimals(0),
            )
            .caption(format!("{} spent of {}", points.spent, points.earned)),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kpi-shapes",
                    "Shapes",
                    state.simulation.placements.len() as f64,
                )
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
        .child(
            div()
                .id("rates-body")
                .flex()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(
                    div()
                        .id("observations-column")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .child(observations_card(state, cx)),
                )
                .child(
                    div()
                        .id("supplements-column")
                        .flex()
                        .flex_col()
                        .flex_none()
                        .w(px(340.0))
                        .min_h_0()
                        .gap_3()
                        .child(supplements_card(state, cx))
                        .child(save_card(state, cx)),
                ),
        )
}

/// One supplement row: its name, its value, and a stepper that edits it.
fn supplement_row(
    id: &'static str,
    label: &'static str,
    value: f64,
    (min, max, step, precision): (f64, f64, f64, usize),
    set: fn(&mut AppState, f64, &mut Context<AppState>),
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let shown = format!("{value:.precision$}");
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label}: {shown}"))
        .aria_value(shown)
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .h(px(30.0))
        .child(Caption::new(label).text_color(muted))
        .child(
            Stepper::new(SharedString::from(format!("{id}-stepper")), value)
                .range(min, max)
                .step(step)
                .precision(precision)
                .on_change(move |value, _window, cx| {
                    entity.update(cx, |state, cx| set(state, value, cx));
                }),
        )
        .into_any_element()
}

fn supplements_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let muted = cx.theme().colors.fg_muted;
    let open = state.supplements_open;
    let entity = cx.entity();
    let toggle = entity.clone();
    let reset = entity.clone();
    let header = CardHeader::new("Supplements")
        .description("Save-derived multipliers that live outside the grid")
        .action(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(
                    Button::new("supp-reset", "Reset")
                        .size(ControlSize::Sm)
                        .disabled(!open)
                        .on_click(move |_event, _window, cx| {
                            reset.update(cx, |state, cx| state.reset_supplements(cx));
                        }),
                )
                .child(
                    Button::new("supp-toggle", if open { "Hide" } else { "Show" })
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            toggle.update(cx, |state, cx| state.set_supplements_open(!open, cx));
                        }),
                ),
        );
    let card = Card::new().header(header);
    if !open {
        return card.child(div().px_3().pb_2().child(
            Caption::new("Hidden; the simulated multipliers keep their values.").text_color(muted),
        ));
    }
    let context = state.context.clone();
    card.child(
        div()
            .id("supplements-card")
            .flex()
            .flex_col()
            .gap_1()
            .px_3()
            .pb_2()
            .child(supplement_row(
                "supp-svrxp",
                "Server EXP scale",
                context.server_var_res_xp,
                (1.0, 10.0, 0.05, 2),
                AppState::set_server_var_res_xp,
                cx,
            ))
            .child(supplement_row(
                "supp-abm",
                "All-bonus multiplier",
                context.all_bonus_multi,
                (1.0, 100.0, 0.05, 2),
                AppState::set_all_bonus_multi,
                cx,
            ))
            .child(supplement_row(
                "supp-extra-pct",
                "Extra account %",
                context.extra_pct,
                (0.0, 10_000.0, 5.0, 0),
                AppState::set_extra_pct,
                cx,
            ))
            .child(supplement_row(
                "supp-extra-multi",
                "Extra account ×",
                context.extra_multiplier,
                (0.0, 100.0, 0.05, 2),
                AppState::set_extra_multiplier,
                cx,
            ))
            .child(supplement_row(
                "supp-points",
                "Bonus grid points",
                context.bonus_grid_points as f64,
                (0.0, 100_000.0, 10.0, 0),
                AppState::set_bonus_grid_points,
                cx,
            ))
            .child(supplement_row(
                "supp-shapes",
                "Extra shapes",
                context.extra_shapes as f64,
                (0.0, 100.0, 1.0, 0),
                AppState::set_extra_shapes,
                cx,
            ))
            .child(supplement_row(
                "supp-mags",
                "Extra magnifiers",
                context.extra_magnifiers as f64,
                (0.0, 200.0, 1.0, 0),
                AppState::set_extra_magnifiers,
                cx,
            )),
    )
}

fn save_card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let open = state.save_open;
    let message = state.save_message.clone();
    let ok = state.save_ok;
    let entity = cx.entity();
    let toggle = entity.clone();
    let import = entity.clone();
    let header = CardHeader::new("Save import")
        .description("Paste a decoded save (JSON)")
        .action(
            Button::new("save-toggle", if open { "Hide" } else { "Show" })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    toggle.update(cx, |state, cx| state.set_save_open(!open, cx));
                }),
        );
    let card = Card::new().header(header);
    if !open {
        return card.child(
            div().px_3().pb_2().child(
                Caption::new("Hidden. Nothing is imported until you paste a save.")
                    .text_color(colors.fg_muted),
            ),
        );
    }
    card.child(
        div()
            .id("save-panel")
            .flex()
            .flex_col()
            .gap_2()
            .px_3()
            .pb_3()
            .child({
                let input_focus = state.save_input.read(cx).focus().clone();
                div()
                    .id("save-input")
                    .aria_label("Game save JSON")
                    .w_full()
                    .on_click(move |_event, window, cx| window.focus(&input_focus, cx))
                    .child(Input::new(&state.save_input).size(ControlSize::Sm))
            })
            .child(
                Button::new("save-import", "Import save")
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        import.update(cx, |state, cx| state.import_save(cx));
                    }),
            )
            .child(
                div()
                    .id("save-status")
                    .role(gpui::Role::Label)
                    .aria_label(message.clone())
                    .w_full()
                    .line_clamp(4)
                    .text_size(px(11.0))
                    .text_color(if ok { colors.fg_muted } else { colors.danger })
                    .child(message),
            ),
    )
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
        let mut column = div().flex().flex_col().flex_1().min_w_0().child(
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
