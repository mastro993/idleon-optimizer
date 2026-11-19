//! The all-plot overgrowth curves: every plot's long-run target and the
//! selected plot's full curve.
//!
//! Two tables on the evolution tab: an all-plots summary and `Plot N Curve`
//! for the selected plot. Selecting a plot anywhere on the section, including
//! a summary row here, switches the curve.

use super::controls::Field;
use super::field::family_name;
use super::table::{Col, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::{ActiveTheme as _, v_flex};
use idleon_farming::{OvergrowthAdvice, OvergrowthCurve, PLOTS, PlotMetrics};
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader, TEXT_BODY, Tone};

/// The all-plots summary and the selected plot's curve.
pub(super) fn curves_card(
    state: &AppState,
    field: &Field,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let selected = state.farming.selected_plot.min(PLOTS - 1);
    let advice = format!(
        "Plot {} · {}",
        selected + 1,
        advice(&field.curves[selected])
    );
    let all = all_plots(&field.plots, &field.curves, selected, cx);
    let curve = plot_curve(&field.curves[selected], cx);
    Card::new()
        .header(
            CardHeader::new("Long-run overgrowth curves")
                .description(
                    "Every plot's best long-run target, and the selected plot's full 0-30 curve",
                )
                .action(tag(advice, Tone::Neutral)),
        )
        .child(v_flex().id("farming-curves").gap_3().child(all).child(curve))
        .child(note(
            "A target's cycle counts the growth cycles its overgrowth rolls take, so a higher OG pays more per harvest but less per second once the rolls slow down.",
            cx,
        ))
        .into_any_element()
}

/// One row per plot; the plot label selects that plot's curve.
fn all_plots(
    metrics: &[PlotMetrics],
    curves: &[OvergrowthCurve],
    selected: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let accent = cx.theme().primary;
    let hover = cx.theme().accent;
    let radius = cx.theme().radius;
    let entity = cx.entity();
    let rows = metrics.iter().zip(curves).map(|(metric, curve)| {
        let entity = entity.clone();
        let index = metric.plot;
        let rate = format::number(curve.best.long_run_rate * 86_400.0);
        [
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
                .rounded(radius)
                .px_1()
                .cursor_pointer()
                .when(index == selected, |label| label.text_color(accent))
                .hover(move |style| style.bg(hover))
                .on_click(move |_event, window, cx| {
                    entity.update(cx, |state, cx| state.select_plot(index, window, cx));
                })
                .child(format!("Plot {}", index + 1))
                .into_any_element(),
            text(family_name(metric.seed_family)),
            text(curve.current_og.to_string()),
            text(format::chance(metric.next_overgrowth_chance)),
            text(format!("OG {}", curve.best.target)),
            text(rate),
            text(advice(curve)),
        ]
    });
    table(
        "farming-curves-all",
        &[
            Col::Text("Plot"),
            Col::Text("Family"),
            Col::Number("Current OG"),
            Col::Number("Next chance"),
            Col::Text("Long-run target"),
            Col::Number("Long-run / day"),
            Col::Text("Action"),
        ],
        rows,
        cx,
    )
    .into_any_element()
}

/// The selected plot's 0-30 curve, trimmed to the rows the table shows.
fn plot_curve(curve: &OvergrowthCurve, cx: &App) -> AnyElement {
    let rows = curve.display_rows().map(|row| {
        let best = row.target == curve.best.target;
        let eta = if row.target <= curve.current_og {
            "Ready".to_string()
        } else {
            format::duration(row.eta_from_current)
        };
        [
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
            text(format!("{}x", format::number(row.reward))),
            text(format::duration(row.repeat_seconds)),
            text(format::number(row.long_run_rate * 86_400.0)),
            text(eta),
            if best {
                tag("Best", Tone::Success).into_any_element()
            } else {
                div().into_any_element()
            },
        ]
    });
    let table = table(
        "farming-curve",
        &[
            Col::Text("Target OG"),
            Col::Number("Reward"),
            Col::Number("Full-cycle time"),
            Col::Number("Long-run / day"),
            Col::Number("ETA now"),
            Col::Text("Best"),
        ],
        rows,
        cx,
    );
    v_flex()
        .id("farming-curve")
        .role(gpui::Role::Group)
        .aria_label(format!(
            "Plot {} curve: target OG {}",
            curve.plot + 1,
            curve.best.target
        ))
        .gap_2()
        .child(
            div()
                .id("farming-curve-title")
                .role(gpui::Role::Label)
                .aria_label(format!("Plot {} curve", curve.plot + 1))
                .text_size(TEXT_BODY)
                .font_weight(gpui::FontWeight::SEMIBOLD)
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
