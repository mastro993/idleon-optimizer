//! The all-plot overgrowth curves: every plot's long-run target and the
//! selected plot's full curve, from `optimalOvergrowthTarget`.
//!
//! Upstream renders these as two tables on its evolution tab — an all-plots
//! summary and `Plot N Curve` for the selected plot. Selecting a plot anywhere
//! on the section, including a summary row here, switches the curve.

use crate::sections::farming::field::family_name;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_core::farming::{OvergrowthAdvice, OvergrowthCurve, PLOTS, PlotMetrics, Snapshot};

/// The all-plots summary and the selected plot's curve.
pub(super) fn curves_card(
    state: &mut AppState,
    snapshot: &Snapshot,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let selected = state.farming.selected_plot.min(PLOTS - 1);
    let metrics = state.farm.all_plot_metrics();
    let curves = state.farm.overgrowth_curves(snapshot);
    let muted = cx.theme().colors.fg_muted;
    let advice = format!("Plot {} · {}", selected + 1, advice(&curves[selected]));
    let all = all_plots(&metrics, &curves, selected, cx);
    let curve = plot_curve(&curves[selected]);
    Card::new()
        .header(
            CardHeader::new("Long-run overgrowth curves")
                .description(
                    "Every plot's best long-run target, and the selected plot's full 0-30 curve",
                )
                .action(Badge::new(advice)),
        )
        .child(
            div()
                .id("farming-curves")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(all)
                .child(curve),
        )
        .child(
            div()
                .px_4()
                .pb_3()
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "A target's cycle counts the growth cycles its overgrowth rolls take, so a higher OG pays more per harvest but less per second once the rolls slow down.",
                ),
        )
        .into_any_element()
}

/// One row per plot; the plot label selects that plot's curve.
fn all_plots(
    metrics: &[PlotMetrics],
    curves: &[OvergrowthCurve],
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let accent = cx.theme().colors.accent;
    let hover = cx.theme().colors.hover;
    let mut table = Table::new([
        "Plot",
        "Family",
        "Current OG",
        "Next chance",
        "Long-run target",
        "Long-run / day",
        "Action",
    ]);
    for (metric, curve) in metrics.iter().zip(curves) {
        let entity = cx.entity();
        let index = metric.plot;
        let rate = format::number(curve.best.long_run_rate * 86_400.0);
        table = table.row([
            div()
                .id(("farming-curve-plot", index))
                .role(gpui::Role::Button)
                .aria_label(format!(
                    "Plot {}: current OG {} · target OG {} · rate {}",
                    index + 1,
                    curve.current_og,
                    curve.best.target,
                    rate
                ))
                .aria_selected(index == selected)
                .rounded(px(3.0))
                .px_1()
                .cursor_pointer()
                .when(index == selected, |label| label.text_color(accent))
                .hover(move |style| style.bg(hover))
                .on_click(move |_event, window, cx| {
                    entity.update(cx, |state, cx| state.select_plot(index, window, cx));
                })
                .child(format!("Plot {}", index + 1))
                .into_any_element(),
            div()
                .child(family_name(metric.seed_family))
                .into_any_element(),
            div().child(curve.current_og.to_string()).into_any_element(),
            div()
                .child(format::chance(metric.next_overgrowth_chance))
                .into_any_element(),
            div()
                .child(format!("OG {}", curve.best.target))
                .into_any_element(),
            div().child(rate).into_any_element(),
            div().child(advice(curve)).into_any_element(),
        ]);
    }
    table.into_any_element()
}

/// The selected plot's 0-30 curve, trimmed to the rows upstream shows.
fn plot_curve(curve: &OvergrowthCurve) -> AnyElement {
    let mut table = Table::new([
        "Target OG",
        "Reward",
        "Full-cycle time",
        "Long-run / day",
        "ETA now",
        "Best",
    ]);
    for row in curve.display_rows() {
        let best = row.target == curve.best.target;
        let eta = if row.target <= curve.current_og {
            "Ready".to_string()
        } else {
            format::duration(row.eta_from_current)
        };
        table = table.row([
            div()
                .id(("farming-curve-row", row.target as usize))
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Target OG {}: reward {}x · {} · {}",
                    row.target,
                    format::number(row.reward),
                    format::duration(row.repeat_seconds),
                    eta
                ))
                .child(format!("OG {}", row.target))
                .into_any_element(),
            div()
                .child(format!("{}x", format::number(row.reward)))
                .into_any_element(),
            div()
                .child(format::duration(row.repeat_seconds))
                .into_any_element(),
            div()
                .child(format::number(row.long_run_rate * 86_400.0))
                .into_any_element(),
            div().child(eta).into_any_element(),
            if best {
                Badge::new("Best").tone(Tone::Success).into_any_element()
            } else {
                div().into_any_element()
            },
        ]);
    }
    div()
        .id("farming-curve")
        .role(gpui::Role::Group)
        .aria_label(format!(
            "Plot {} curve: target OG {}",
            curve.plot + 1,
            curve.best.target
        ))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("farming-curve-title")
                .role(gpui::Role::Label)
                .aria_label(format!("Plot {} curve", curve.plot + 1))
                .text_size(px(13.0))
                .font_weight(gpui::FontWeight::BOLD)
                .child(format!(
                    "Plot {} curve · current OG {} · target OG {}",
                    curve.plot + 1,
                    curve.current_og,
                    curve.best.target
                )),
        )
        .child(table)
        .into_any_element()
}

/// `Harvest` once the plot is at its target, else how long the climb takes.
fn advice(curve: &OvergrowthCurve) -> String {
    match curve.advice() {
        OvergrowthAdvice::Harvest => "Harvest".into(),
        OvergrowthAdvice::Wait(seconds) => format!("Wait {}", format::duration(seconds)),
    }
}
