//! The estimate's outlook: what its mean numbers mean for the next
//! obstructions, the cell levels and the critical timeline.

use super::parts::{figure, stat, strips, text_kpi, text_table};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::ActiveTheme as _;
use idleon_jelly::{Estimate, Insights};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP};

/// The progression outlook, read once when the estimate was stored.
pub(crate) fn insights_card(insights: &Insights) -> AnyElement {
    let attempts = match insights.expected_attempts_to_clear {
        Some(value) => format!("{value:.2}"),
        None => "No clears observed".to_string(),
    };
    let days = match insights.expected_days_to_clear {
        Some(value) => format!("{value:.2}"),
        None => "Not estimable".to_string(),
    };
    let dash = || "—".to_string();
    let cell_exp = text_table(
        &[
            ("Cell", false),
            ("Level", true),
            ("EXP / attempt", true),
            ("Attempts to level", true),
            ("Days", true),
        ],
        insights.exp_rows.iter().map(|row| {
            vec![
                row.name.clone(),
                row.current_level.to_string(),
                format::value(row.mean_exp_per_attempt),
                row.attempts_to_next_level
                    .map_or_else(dash, |value| format!("{value:.1}")),
                row.days_to_next_level
                    .map_or_else(dash, |value| format!("{value:.2}")),
            ]
        }),
    );
    let outlook = text_table(
        &[
            ("Obstruction", true),
            ("HP", true),
            ("Clear estimate", true),
            ("Damage ratio", true),
            ("Expected attempts", true),
            ("Within daily tries", true),
        ],
        insights.obstruction_outlook.iter().map(|row| {
            vec![
                format!("{}", row.obstruction + 1),
                format::value(row.hp),
                row.estimated_clear_probability
                    .map_or_else(dash, format::chance),
                row.damage_ratio.map_or_else(dash, format::percent),
                row.expected_attempts
                    .map_or_else(dash, |value| format!("{value:.2}")),
                row.clear_within_daily_attempts
                    .map_or_else(dash, format::chance),
            ]
        }),
    );
    Card::new()
        .header(CardHeader::new("Progression outlook").description(format!(
            "{} daily attempts · {} passive Bloodcells a day",
            format::number(insights.daily_attempts),
            format::value(insights.saved_daily_passive_bloodcells)
        )))
        .child(strips(
            "jelly-insights",
            vec![
                text_kpi(
                    "jelly-insight-attempts",
                    "Attempts to clear",
                    attempts,
                    format!(
                        "{} chance within the daily attempts",
                        format::chance(insights.probability_within_daily_attempts)
                    ),
                ),
                stat(
                    "jelly-insight-bloodcells",
                    "Bloodcells / daily cycle",
                    insights.expected_bloodcells_per_daily_cycle,
                )
                .caption(format!(
                    "{} / attempt",
                    format::value(insights.expected_bloodcells_per_attempt)
                )),
                figure(
                    "jelly-insight-net-attempts",
                    "Net attempts / operation",
                    insights.expected_net_attempt_delta,
                    2,
                )
                .caption(format!(
                    "{} per daily cycle",
                    format::number(insights.expected_net_attempt_delta_per_daily_cycle)
                )),
                figure(
                    "jelly-insight-normal-share",
                    "Normal-phase damage",
                    100.0 * insights.normal_damage_share.unwrap_or(0.0),
                    1,
                )
                .suffix("%")
                .caption(format!("{} mean", format::value(insights.normal_damage))),
                figure(
                    "jelly-insight-critical-share",
                    "Critical-phase damage",
                    100.0 * insights.critical_damage_share.unwrap_or(0.0),
                    1,
                )
                .suffix("%")
                .caption(format!("{} mean", format::value(insights.critical_damage))),
                text_kpi(
                    "jelly-insight-days",
                    "Expected days to clear",
                    days,
                    format!("{} attempts a day", format::number(insights.daily_attempts)),
                ),
            ],
        ))
        .child(
            div()
                .id("jelly-cell-exp-outlook")
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Cell EXP outlook"))
                .child(cell_exp),
        )
        .child(
            div()
                .id("jelly-obstruction-outlook")
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new(format!(
                    "Obstruction outlook · {} (deterministic surrogate)",
                    insights.method
                )))
                .child(outlook),
        )
        .into_any_element()
}

/// The per-cell-type mean contribution columns, shared with the simulator.
pub(crate) const CONTRIBUTION: [(&str, bool); 6] = [
    ("Cell", false),
    ("Shots", true),
    ("Hits", true),
    ("Damage", true),
    ("Bloodcells", true),
    ("EXP", true),
];

pub(crate) fn contribution_card(result: &Estimate) -> AnyElement {
    let table = text_table(
        &CONTRIBUTION,
        result
            .mean_contribution_by_type
            .iter()
            .filter(|row| row.shots > 0.0 || row.hits > 0.0)
            .map(|row| {
                vec![
                    row.name.clone(),
                    format!("{:.1}", row.shots),
                    format!("{:.1}", row.hits),
                    format::value(row.damage),
                    format::value(row.bloodcells),
                    format::value(row.exp),
                ]
            }),
    );
    Card::new()
        .header(
            CardHeader::new("Cell contribution")
                .description("Mean across the trials, per cell type."),
        )
        .child(div().id("jelly-contribution").child(table))
        .into_any_element()
}

pub(crate) fn timeline_card(result: &Estimate, cx: &mut Context<AppState>) -> AnyElement {
    let representative = &result.representative;
    let deaths = &representative.death_log;
    let card = Card::new().header(CardHeader::new("Critical target timeline").description(
        format!(
            "The first trial ({}) lost {} slot(s) to {} boss attacks.",
            representative.ended_by,
            format::number(representative.deaths),
            format::number(representative.boss_attacks)
        ),
    ));
    if deaths.is_empty() {
        return card
            .child(Caption::new("The run never reached the critical phase."))
            .into_any_element();
    }
    let theme = cx.theme();
    let chips = deaths.iter().take(24).map(|death| {
        div()
            .id(format!("jelly-death-{}", death.slot))
            .px_2()
            .py_1()
            .rounded(theme.radius)
            .border_1()
            .border_color(theme.border)
            .child(Caption::new(format!(
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
            )))
    });
    card.child(
        div()
            .id("jelly-timeline")
            .flex()
            .flex_wrap()
            .gap(PANEL_GAP)
            .children(chips),
    )
    .into_any_element()
}
