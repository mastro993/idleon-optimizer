//! The Evolution & OG tab: the chain to the target crop, its hardest remaining
//! steps, the overgrowth stopping advice and the evolution assignment.
//!
//! The chain cards and the "Hardest Remaining Steps" table come from the
//! evolution chain plan, the stopping cards from the optimal overgrowth target,
//! and the plot grid from the evolution objective's plot assignment. The
//! per-plot action table and the selected plot's curve are the shared
//! overgrowth card below them.

use super::assignment::assignment_card;
use super::curves::curves_card;
use super::field::family_name;
use super::table::{Col, figure, note, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, div};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::v_flex;
use idleon_farming::{EvolutionChain, EvolutionStep, OvergrowthAdvice, OvergrowthCurve, PLOTS};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, KpiStrip, PANEL_GAP, TEXT_BODY, Tone,
};

/// How many of the slowest stages the table lists.
const HARDEST_STEPS: usize = 20;

/// The chain, the stopping advice and the evolution assignment.
pub(super) fn evolution(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let field = state.farming_field();
    let evolution = state.farming_evolution();
    let target = i64::from(state.farming.target_crop);
    let selected = state.farming.selected_plot.min(PLOTS - 1);
    let body = v_flex()
        .id("farming-evolution")
        .role(gpui::Role::Group)
        .aria_label("Farming evolution and overgrowth")
        .gap(PANEL_GAP)
        .child(chain_card(target, evolution.chain.as_ref(), cx))
        .child(stopping_card(selected, &field.curves[selected]))
        .child(curves_card(state, &field, cx))
        .child(assignment_card(&evolution.assignment, cx));
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("farming-evolution-scroll")
        .into_any_element()
}

/// The six chain figures and the hardest remaining steps.
fn chain_card(target: i64, chain: Option<&EvolutionChain>, cx: &App) -> AnyElement {
    let header = CardHeader::new(format!("Evolution chain to crop #{target}"))
        .description("Every evolution roll left, priced at the plots that can roll it");
    let Some(chain) = chain else {
        return Card::new()
            .header(header)
            .child(
                div().id("farming-chain-missing").child(
                    Callout::new(
                        Tone::Warning,
                        "Target crop is outside the known seed families",
                    )
                    .child(Caption::new(
                        "The families end at crop 329; aim the crop target inside them.",
                    )),
                ),
            )
            .into_any_element();
    };
    let remaining = if chain.step_count() == 0 {
        "No steps left".to_string()
    } else {
        format::number(chain.step_count() as f64)
    };
    let figures = [
        figure(
            "farming-chain-plots",
            "Parallel plots",
            format::number(chain.plot_count as f64),
            family_name(chain.family as i64),
        ),
        figure(
            "farming-chain-steps",
            "Evolution steps",
            remaining,
            format!(
                "Current depth {} to {}",
                depth_label(chain.current_depth),
                chain.target_depth
            ),
        ),
        figure(
            "farming-chain-expected",
            "Expected ETA",
            format::duration(chain.expected_seconds()),
            "Stage expectations summed",
        ),
        figure(
            "farming-chain-median",
            "Median ETA",
            format::duration(chain.median_seconds()),
            "Stage medians summed",
        ),
        figure(
            "farming-chain-p90",
            "P90 ETA",
            format::duration(chain.p90_seconds()),
            "Stage estimates summed",
        ),
        figure(
            "farming-chain-crop",
            "Required crop",
            format!("#{}", chain.target_crop - 1),
            "Run on every eligible plot",
        ),
    ];
    let [a, b, c, d, e, f] = figures;
    Card::new()
        .header(header)
        .child(
            v_flex()
                .id("farming-chain")
                .role(gpui::Role::Group)
                .aria_label(format!(
                    "Evolution chain to crop #{target}: {} step(s)",
                    chain.step_count()
                ))
                .gap_3()
                .child(
                    v_flex()
                        .id("farming-chain-cards")
                        .gap(PANEL_GAP)
                        .child(KpiStrip::new().child(a).child(b).child(c))
                        .child(KpiStrip::new().child(d).child(e).child(f)),
                )
                .child(
                    div()
                        .text_size(TEXT_BODY)
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .child("Hardest remaining steps"),
                )
                .child(hardest_steps(chain, cx))
                .child(note(
                    "All eligible plots advance in parallel; percentile times are summed stage estimates.",
                    cx,
                )),
        )
        .into_any_element()
}

/// `Hardest Remaining Steps`: the slowest stages first, as the page sorts them.
fn hardest_steps(chain: &EvolutionChain, cx: &App) -> AnyElement {
    div()
        .id("farming-hardest-steps")
        .child(table(
            "farming-hardest-steps",
            &[
                Col::Text("Target crop"),
                Col::Number("Any-plot chance"),
                Col::Number("Per plot"),
                Col::Number("Cycle"),
                Col::Number("Expected"),
                Col::Number("Median"),
                Col::Number("P90"),
            ],
            chain.hardest_steps(HARDEST_STEPS).into_iter().map(step_row),
            cx,
        ))
        .into_any_element()
}

/// One stage of the chain.
fn step_row(step: &EvolutionStep) -> [AnyElement; 7] {
    let chance = format::chance(step.plan.chance);
    let any = format::duration(step.plan.expected_seconds);
    [
        div()
            .id(("farming-step", step.target_crop as usize))
            .role(gpui::Role::Label)
            .aria_label(format!(
                "Crop #{}: any-plot chance {chance} · expected {any}",
                step.target_crop
            ))
            .child(format!("Crop #{}", step.target_crop))
            .into_any_element(),
        text(chance),
        text(if step.plot_count == 0 {
            "—".to_string()
        } else if step.per_plot_min == step.per_plot_max {
            format::chance(step.per_plot_min)
        } else {
            format!(
                "{} – {}",
                format::chance(step.per_plot_min),
                format::chance(step.per_plot_max)
            )
        }),
        text(format::duration(step.plan.cycle_seconds)),
        text(any),
        text(format::duration(step.plan.median_seconds)),
        text(format::duration(step.plan.p90_seconds)),
    ]
}

/// `Overgrowth Stopping`: the selected plot's target, multiplier and wait.
fn stopping_card(selected: usize, curve: &OvergrowthCurve) -> AnyElement {
    let advice = match curve.advice() {
        OvergrowthAdvice::Harvest => "Harvest now".to_string(),
        OvergrowthAdvice::Wait(seconds) => format::duration(seconds),
    };
    let settled = matches!(curve.advice(), OvergrowthAdvice::Harvest);
    Card::new()
        .header(
            CardHeader::new("Overgrowth stopping")
                .description("When to stop rolling overgrowth and harvest the plot"),
        )
        .child(
            div()
                .id("farming-overgrowth-stopping")
                .role(gpui::Role::Group)
                .aria_label(format!(
                    "Overgrowth stopping: plot {} · target OG {} · {advice}",
                    selected + 1,
                    curve.best.target
                ))
                .child(
                    KpiStrip::new()
                        .child(figure(
                            "farming-og-plot",
                            "Selected plot",
                            format!("Plot {}", selected + 1),
                            format!("Current OG {}", curve.current_og),
                        ))
                        .child(figure(
                            "farming-og-target",
                            "Long-run target",
                            format!("OG {}", curve.best.target),
                            "Best reward per total regrowth time",
                        ))
                        .child(figure(
                            "farming-og-multiplier",
                            "Target multiplier",
                            format!("{}x", format::number(curve.best.reward)),
                            "OG multiplier caps at 1B",
                        ))
                        .child(figure(
                            "farming-og-eta",
                            "ETA from current",
                            advice,
                            if settled {
                                "At or beyond the optimum"
                            } else {
                                "Expected additional wait"
                            },
                        )),
                ),
        )
        .into_any_element()
}

/// The chain's starting depth: the field may carry no plot of the family.
fn depth_label(depth: i64) -> SharedString {
    if depth < 0 {
        SharedString::from("none")
    } else {
        SharedString::from(depth.to_string())
    }
}
