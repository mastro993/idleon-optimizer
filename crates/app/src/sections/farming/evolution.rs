//! The Evolution & OG tab: the chain to the target crop, its hardest remaining
//! steps, the overgrowth stopping advice and the evolution assignment.
//!
//! Ported from `renderEvolution` in `farming-calc.html`: the chain cards and the
//! "Hardest Remaining Steps" table come from `evolutionChainPlan`, the stopping
//! cards from `optimalOvergrowthTarget`, and the plot grid from the evolution
//! objective's `plotAssignmentPlan`. The per-plot action table and the selected
//! plot's curve are the shared overgrowth card below them.

use crate::sections::farming::assignment::assignment_card;
use crate::sections::farming::curves::curves_card;
use crate::sections::farming::field::family_name;
use crate::sections::farming::overview::stat_card;
use crate::state::AppState;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_farming::{
    EvolutionChain, EvolutionStep, Objective, OvergrowthAdvice, OvergrowthCurve, PLOTS,
};
use idleon_ui::format;

/// How many of the slowest stages the table lists, as the upstream page slices
/// its own list.
const HARDEST_STEPS: usize = 20;

/// The chain, the stopping advice and the evolution assignment.
pub(super) fn evolution(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let snapshot = state.farm.snapshot();
    let target = i64::from(state.farming.target_crop);
    let chain = state.farm.evolution_chain(target, &snapshot);
    let selected = state.farming.selected_plot.min(PLOTS - 1);
    let curve = state.farm.overgrowth_curve(selected, &snapshot);
    let assignment = state
        .farm
        .assignment(Objective::Evolution, Some(state.farming.target_crop));
    ScrollArea::new("farming-evolution-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("farming-evolution")
                .role(gpui::Role::Group)
                .aria_label("Farming evolution and overgrowth")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(chain_card(target, chain.as_ref(), cx))
                .child(stopping_card(selected, &curve, cx))
                .child(curves_card(state, &snapshot, cx))
                .child(assignment_card(&assignment, cx)),
        )
        .into_any_element()
}

/// The six chain cards and the hardest remaining steps.
fn chain_card(
    target: i64,
    chain: Option<&EvolutionChain>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let header = CardHeader::new(format!("Evolution chain to crop #{target}"))
        .description("Every evolution roll left, priced at the plots that can roll it");
    let Some(chain) = chain else {
        return Card::new()
            .header(header)
            .child(
                div().id("farming-chain-missing").px_4().pb_4().child(
                    Callout::new(Severity::Warning)
                        .title("Target crop is outside the known seed families")
                        .child(Caption::new(
                            "The families end at crop 329; aim the crop target inside them.",
                        )),
                ),
            )
            .into_any_element();
    };
    let muted = cx.theme().colors.fg_muted;
    let remaining = if chain.step_count() == 0 {
        "No steps left".to_string()
    } else {
        format::number(chain.step_count() as f64)
    };
    Card::new()
        .header(header)
        .child(
            div()
                .id("farming-chain")
                .role(gpui::Role::Group)
                .aria_label(format!(
                    "Evolution chain to crop #{target}: {} step(s)",
                    chain.step_count()
                ))
                .flex()
                .flex_col()
                .gap_3()
                .px_4()
                .pb_4()
                .child(
                    div()
                        .id("farming-chain-cards")
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(stat_card(
                            "farming-chain-plots",
                            "Parallel plots",
                            format::number(chain.plot_count as f64),
                            family_name(chain.family as i64),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-chain-steps",
                            "Evolution steps",
                            remaining,
                            format!(
                                "Current depth {} to {}",
                                depth_label(chain.current_depth),
                                chain.target_depth
                            ),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-chain-expected",
                            "Expected ETA",
                            format::duration(chain.expected_seconds()),
                            "Stage expectations summed".into(),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-chain-median",
                            "Median ETA",
                            format::duration(chain.median_seconds()),
                            "Stage medians summed".into(),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-chain-p90",
                            "P90 ETA",
                            format::duration(chain.p90_seconds()),
                            "Stage estimates summed".into(),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-chain-crop",
                            "Required crop",
                            format!("#{}", chain.target_crop - 1),
                            "Run on every eligible plot".into(),
                            cx,
                        )),
                )
                .child(
                    div()
                        .text_size(px(13.0))
                        .font_weight(gpui::FontWeight::BOLD)
                        .child("Hardest remaining steps"),
                )
                .child(hardest_steps(chain))
                .child(
                    Caption::new(
                        "All eligible plots advance in parallel; percentile times are summed stage estimates.",
                    )
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// `Hardest Remaining Steps`: the slowest stages first, as the page sorts them.
fn hardest_steps(chain: &EvolutionChain) -> AnyElement {
    let mut table = Table::new([
        "Target crop",
        "Any-plot chance",
        "Per plot",
        "Cycle",
        "Expected",
        "Median",
        "P90",
    ]);
    for step in chain.hardest_steps(HARDEST_STEPS) {
        table = table.row(step_row(step));
    }
    div()
        .id("farming-hardest-steps")
        .child(table)
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
        div().child(chance).into_any_element(),
        div()
            .child(if step.plot_count == 0 {
                "—".to_string()
            } else if step.per_plot_min == step.per_plot_max {
                format::chance(step.per_plot_min)
            } else {
                format!(
                    "{} – {}",
                    format::chance(step.per_plot_min),
                    format::chance(step.per_plot_max)
                )
            })
            .into_any_element(),
        div()
            .child(format::duration(step.plan.cycle_seconds))
            .into_any_element(),
        div()
            .child(format::duration(step.plan.expected_seconds))
            .into_any_element(),
        div()
            .child(format::duration(step.plan.median_seconds))
            .into_any_element(),
        div()
            .child(format::duration(step.plan.p90_seconds))
            .into_any_element(),
    ]
}

/// `Overgrowth Stopping`: the selected plot's target, multiplier and wait.
fn stopping_card(
    selected: usize,
    curve: &OvergrowthCurve,
    cx: &mut Context<AppState>,
) -> AnyElement {
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
                .p_4()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(stat_card(
                            "farming-og-plot",
                            "Selected plot",
                            format!("Plot {}", selected + 1),
                            format!("Current OG {}", curve.current_og),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-og-target",
                            "Long-run target",
                            format!("OG {}", curve.best.target),
                            "Best reward per total regrowth time".into(),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-og-multiplier",
                            "Target multiplier",
                            format!("{}x", format::number(curve.best.reward)),
                            "OG multiplier caps at 1B".into(),
                            cx,
                        ))
                        .child(stat_card(
                            "farming-og-eta",
                            "ETA from current",
                            advice,
                            if settled {
                                "At or beyond the optimum".into()
                            } else {
                                "Expected additional wait".into()
                            },
                            cx,
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
