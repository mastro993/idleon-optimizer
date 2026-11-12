//! The operation estimate: the run controls, its aggregate and its outlook.

use super::outlook;
use crate::state::AppState;
use crate::ui::format;
use crate::ui::task;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::NumberInput;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_core::jelly::{Estimate, Jelly};

/// A number with a game suffix (`1.23M`), as the KPI cards read.
pub(crate) fn stat(id: &'static str, label: &'static str, value: f64) -> Statistic {
    let (scaled, suffix) = format::scaled(value);
    let decimals = if suffix.is_empty() { 1 } else { 2 };
    Statistic::new(id, label, scaled)
        .decimals(decimals)
        .suffix(suffix)
}

/// A headline that is not a number, in the KPI row's own type scale.
pub(crate) fn text_kpi(
    id: &'static str,
    label: &'static str,
    value: String,
    caption: String,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = &cx.theme().colors;
    div()
        .id(id)
        .flex()
        .flex_col()
        .gap_1()
        .min_w(px(150.0))
        .p_3()
        .rounded(px(8.0))
        .border_1()
        .border_color(colors.border)
        .bg(colors.surface)
        .child(
            div()
                .text_size(px(11.0))
                .text_color(colors.fg_muted)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_size(px(20.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(value),
        )
        .child(
            div()
                .text_size(px(10.0))
                .text_color(colors.fg_muted)
                .child(caption),
        )
        .into_any_element()
}

pub fn pane(state: &mut AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let task = state.jelly_task(cx);
    let inputs = state.jelly_state.inputs(Some(jelly));
    let trials = state.jelly_state.trials;
    let seed = state.jelly_state.seed;
    let running = task.running;

    let entity = cx.entity();
    let trials_entity = entity.clone();
    let trials_input = NumberInput::new("jelly-trials", f64::from(trials))
        .label("Trials")
        .range(1.0, 1024.0)
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let trials_entity = trials_entity.clone();
            trials_entity.update(cx, |state, cx| state.set_jelly_trials(value, cx));
        });
    let seed_entity = entity.clone();
    let seed_input = NumberInput::new("jelly-seed", f64::from(seed))
        .label("Seed")
        .range(0.0, 4_294_967_295.0)
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let seed_entity = seed_entity.clone();
            seed_entity.update(cx, |state, cx| state.set_jelly_seed(value, cx));
        });
    let run_entity = entity.clone();
    let run = Button::new("jelly-run-estimate", "Run estimate")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            let run_entity = run_entity.clone();
            run_entity.update(cx, |state, cx| state.run_jelly_estimate(cx));
        });
    let stale = task.stale;

    let mut body = div().id("jelly-estimate").flex().flex_col().gap_3().child(
        div()
            .id("jelly-estimate-controls")
            .flex()
            .flex_wrap()
            .items_end()
            .gap_3()
            .child(trials_input)
            .child(seed_input)
            .child(run)
            .child(
                Caption::new(
                    "One seeded 60 FPS operation per trial, aggregated like the upstream estimate.",
                )
                .text_color(cx.theme().colors.fg_muted),
            ),
    );
    if running && let Some(progress) = task.progress.clone() {
        let entity = cx.entity();
        body = body.child(task::progress_card(
            "jelly",
            "Estimating the operation",
            "Seeded trials of the saved board.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_jelly_estimate(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(task::cancelled_banner("jelly-cancelled", stale));
    }
    let Some(result) = task.result.clone() else {
        if !running && !task.cancelled {
            body = body.child(
                Banner::new(
                    "jelly-estimate-empty",
                    Severity::Info,
                    "Run the estimate to see the clear probability, the Bloodcell gain, the cell contributions and the outlook for the next obstructions.",
                ),
            );
        }
        return body.into_any_element();
    };
    if stale {
        body = body.child(task::stale_banner("jelly-estimate-stale"));
    }
    let stale_badge = stale.then(|| task::stale_badge("jelly-estimate"));
    body = body
        .child(
            div()
                .id("jelly-estimate-result")
                .flex()
                .flex_col()
                .gap_3()
                .child(result_cards(&result, stale_badge))
                .child(outlook::insights_card(jelly, &inputs, &result, cx)),
        )
        .child(outlook::contribution_card(&result))
        .child(outlook::timeline_card(&result, cx));
    body.into_any_element()
}

fn result_cards(result: &Estimate, stale_badge: Option<impl IntoElement>) -> AnyElement {
    if !result.valid {
        return Card::new()
            .header(CardHeader::new("Estimate result"))
            .child(div().p_3().child(Banner::new(
                "jelly-estimate-invalid",
                Severity::Warning,
                format!("The board is not a valid layout: {}", result.reason),
            )))
            .into_any_element();
    }
    let clear = 100.0 * result.clear_probability;
    let mut header = CardHeader::new("Estimate result").description(format!(
        "{} / {} seeds cleared · mean {:.0}% boss damage",
        result.success_count,
        result.trials,
        100.0 * result.mean_damage / result.representative.hp.max(1.0)
    ));
    if let Some(badge) = stale_badge {
        header = header.action(badge);
    }
    Card::new()
        .header(header)
        .child(
            div()
                .p_3()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(
                    KpiCard::new(
                        Statistic::new(
                            "jelly-estimate-clear-probability",
                            "Clear probability",
                            clear,
                        )
                        .decimals(1)
                        .suffix("%"),
                    )
                    .caption(format!(
                        "{} to {:.1}% Wilson interval",
                        100.0 * result.clear_probability_low,
                        100.0 * result.clear_probability_high
                    )),
                )
                .child(
                    KpiCard::new(stat(
                        "jelly-estimate-clear-time",
                        "Median clear",
                        result.median_time.unwrap_or(0.0),
                    ))
                    .caption(match result.p90_time {
                        Some(p90) => format!("{} · 90th {:.2}s", format::duration(p90), p90),
                        None => "No clears observed".to_string(),
                    }),
                )
                .child(
                    KpiCard::new(stat(
                        "jelly-estimate-damage",
                        "Mean damage",
                        result.mean_damage,
                    ))
                    .caption(format!(
                        "Normal {} · critical {}",
                        format::value(result.mean_normal_damage),
                        format::value(result.mean_critical_damage)
                    )),
                )
                .child(
                    KpiCard::new(stat(
                        "jelly-estimate-bloodcells",
                        "Bloodcells / attempt",
                        result.mean_bloodcells,
                    ))
                    .caption(format!(
                        "Median {} · max {}",
                        format::value(result.median_bloodcells.unwrap_or(0.0)),
                        format::value(result.max_bloodcells)
                    )),
                )
                .child(
                    KpiCard::new(stat(
                        "jelly-estimate-cell-exp",
                        "Cell EXP / attempt",
                        result.mean_cell_exp,
                    ))
                    .caption("Spread over the cells the shots hit"),
                )
                .child(
                    KpiCard::new(
                        Statistic::new(
                            "jelly-estimate-critical",
                            "Critical time",
                            result.mean_critical_time,
                        )
                        .decimals(2)
                        .suffix("s"),
                    )
                    .caption(format!(
                        "{} deaths · {} revives",
                        format::number(result.mean_deaths),
                        format::number(result.mean_revives)
                    )),
                )
                .child(
                    KpiCard::new(stat(
                        "jelly-estimate-peak-dps",
                        "Mean peak DPS",
                        result.mean_peak_dps,
                    ))
                    .caption(format!(
                        "Best DPS ×{:.3} Bloodcells",
                        result.mean_best_dps_multiplier
                    )),
                )
                .child(
                    KpiCard::new(Statistic::new(
                        "jelly-estimate-live-slots",
                        "Live slots",
                        result.mean_live_slots,
                    ))
                    .caption(format!(
                        "{} boss attacks · {} hits",
                        format::number(result.mean_boss_attacks),
                        format::number(result.mean_hits)
                    )),
                ),
        )
        .into_any_element()
}
