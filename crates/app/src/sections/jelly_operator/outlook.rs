//! The estimate's outlook: what its mean numbers mean for the next
//! obstructions, the cell levels and the critical timeline.

use super::RunInputs;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_core::jelly::{Estimate, Insights, Jelly};

use super::estimate::{stat, text_kpi};

/// The progression outlook, from the same estimate.
pub fn insights_card(
    jelly: &Jelly,
    inputs: &RunInputs,
    result: &Estimate,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let insights = Insights::of(jelly, &inputs.layout, result);
    let muted = cx.theme().colors.fg_muted;
    let attempts = match insights.expected_attempts_to_clear {
        Some(value) => format!("{value:.2}"),
        None => "No clears observed".to_string(),
    };
    let days = match insights.expected_days_to_clear {
        Some(value) => format!("{value:.2}"),
        None => "Not estimable".to_string(),
    };
    let mut cell_exp = Table::new([
        "Cell",
        "Level",
        "EXP / attempt",
        "Attempts to level",
        "Days",
    ]);
    for row in &insights.exp_rows {
        cell_exp = cell_exp.row([
            row.name.clone(),
            row.current_level.to_string(),
            format::value(row.mean_exp_per_attempt),
            row.attempts_to_next_level
                .map(|value| format!("{value:.1}"))
                .unwrap_or_else(|| "—".to_string()),
            row.days_to_next_level
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "—".to_string()),
        ]);
    }
    let mut outlook = Table::new([
        "Obstruction",
        "HP",
        "Clear estimate",
        "Damage ratio",
        "Expected attempts",
        "Within daily tries",
    ]);
    for row in &insights.obstruction_outlook {
        outlook = outlook.row([
            format!("{}", row.obstruction + 1),
            format::value(row.hp),
            row.estimated_clear_probability
                .map(format::chance)
                .unwrap_or_else(|| "—".to_string()),
            row.damage_ratio
                .map(format::percent)
                .unwrap_or_else(|| "—".to_string()),
            row.expected_attempts
                .map(|value| format!("{value:.2}"))
                .unwrap_or_else(|| "—".to_string()),
            row.clear_within_daily_attempts
                .map(format::chance)
                .unwrap_or_else(|| "—".to_string()),
        ]);
    }
    Card::new()
        .header(CardHeader::new("Progression outlook").description(format!(
            "{} daily attempts · {} passive Bloodcells a day",
            format::number(insights.daily_attempts),
            format::value(insights.saved_daily_passive_bloodcells)
        )))
        .child(
            div()
                .p_3()
                .flex()
                .flex_col()
                .gap_3()
                .child(
                    div()
                        .id("jelly-insights")
                        .flex()
                        .flex_wrap()
                        .gap_3()
                        .child(text_kpi(
                            "jelly-insight-attempts",
                            "Attempts to clear",
                            attempts.clone(),
                            format!(
                                "{} chance within the daily attempts",
                                format::chance(insights.probability_within_daily_attempts)
                            ),
                            cx,
                        ))
                        .child(
                            KpiCard::new(stat(
                                "jelly-insight-bloodcells",
                                "Bloodcells / daily cycle",
                                insights.expected_bloodcells_per_daily_cycle,
                            ))
                            .caption(format!(
                                "{} / attempt",
                                format::value(insights.expected_bloodcells_per_attempt)
                            )),
                        )
                        .child(
                            KpiCard::new(
                                Statistic::new(
                                    "jelly-insight-net-attempts",
                                    "Net attempts / operation",
                                    insights.expected_net_attempt_delta,
                                )
                                .decimals(2),
                            )
                            .caption(format!(
                                "{} per daily cycle",
                                format::number(insights.expected_net_attempt_delta_per_daily_cycle)
                            )),
                        )
                        .child(
                            KpiCard::new(
                                Statistic::new(
                                    "jelly-insight-normal-share",
                                    "Normal-phase damage",
                                    100.0 * insights.normal_damage_share.unwrap_or(0.0),
                                )
                                .decimals(1)
                                .suffix("%"),
                            )
                            .caption(format!("{} mean", format::value(insights.normal_damage))),
                        )
                        .child(
                            KpiCard::new(
                                Statistic::new(
                                    "jelly-insight-critical-share",
                                    "Critical-phase damage",
                                    100.0 * insights.critical_damage_share.unwrap_or(0.0),
                                )
                                .decimals(1)
                                .suffix("%"),
                            )
                            .caption(format!("{} mean", format::value(insights.critical_damage))),
                        )
                        .child(text_kpi(
                            "jelly-insight-days",
                            "Expected days to clear",
                            days,
                            format!("{} attempts a day", format::number(insights.daily_attempts)),
                            cx,
                        )),
                )
                .child(
                    div()
                        .id("jelly-cell-exp-outlook")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Cell EXP outlook").text_color(muted))
                        .child(cell_exp),
                )
                .child(
                    div()
                        .id("jelly-obstruction-outlook")
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            Caption::new(format!(
                                "Obstruction outlook · {} (deterministic surrogate)",
                                insights.method
                            ))
                            .text_color(muted),
                        )
                        .child(outlook),
                ),
        )
        .into_any_element()
}

pub fn contribution_card(result: &Estimate) -> AnyElement {
    let mut table = Table::new(["Cell", "Shots", "Hits", "Damage", "Bloodcells", "EXP"]);
    for row in result
        .mean_contribution_by_type
        .iter()
        .filter(|row| row.shots > 0.0 || row.hits > 0.0)
    {
        table = table.row([
            row.name.clone(),
            format!("{:.1}", row.shots),
            format!("{:.1}", row.hits),
            format::value(row.damage),
            format::value(row.bloodcells),
            format::value(row.exp),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Cell contribution")
                .description("Mean across the trials; the page's own table, per cell type."),
        )
        .child(div().id("jelly-contribution").p_3().child(table))
        .into_any_element()
}

pub fn timeline_card(result: &Estimate, cx: &mut Context<AppState>) -> AnyElement {
    let representative = &result.representative;
    let deaths = &representative.death_log;
    let muted = cx.theme().colors.fg_muted;
    let mut card = Card::new().header(CardHeader::new("Critical target timeline").description(
        format!(
            "The first trial ({}) lost {} slot(s) to {} boss attacks.",
            representative.ended_by,
            format::number(representative.deaths),
            format::number(representative.boss_attacks)
        ),
    ));
    if deaths.is_empty() {
        return card
            .child(
                div().p_3().child(
                    Caption::new("The run never reached the critical phase.").text_color(muted),
                ),
            )
            .into_any_element();
    }
    let chips = deaths.iter().take(24).map(|death| {
        div()
            .id(format!("jelly-death-{}", death.slot))
            .px_2()
            .py_1()
            .rounded(px(4.0))
            .border_1()
            .child(
                Caption::new(format!(
                    "{:.2}s: {}{} slot {}{}",
                    death.time,
                    if death.focused { "focused " } else { "" },
                    if death.disabled_anchor {
                        "anchor"
                    } else {
                        "footprint"
                    },
                    death.slot,
                    if death.revived { " — revived" } else { "" }
                ))
                .text_color(muted),
            )
            .into_any_element()
    });
    card = card.child(
        div()
            .id("jelly-timeline")
            .p_3()
            .flex()
            .flex_wrap()
            .gap_2()
            .children(chips),
    );
    card.into_any_element()
}
