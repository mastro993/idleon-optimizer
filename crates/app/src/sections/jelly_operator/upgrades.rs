//! The Upgrades tab: the `Research[44]` purchase list, what the next level
//! costs and what the save's own levels buy.
//!
//! Ported from `upgradesHtml` in `jelly-operator-calc.html`: one row per
//! purchase slot with its game description, level, current effect, gain per
//! level and next price. A slot the Research level has not revealed yet says so
//! instead of printing a price the game hides, and the doubler card names every
//! upgrade Menagerie's Upgrade Doubler covers.

use super::estimate::stat;
use crate::state::AppState;
use ely_gpui_component::data_display::{Badge, KpiCard, Tone};
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_jelly::Jelly;
use idleon_ui::format;

/// The Upgrades pane.
pub(crate) fn pane(jelly: &Jelly, cx: &mut Context<AppState>) -> AnyElement {
    let rows = jelly.upgrade_rows();
    let affordable = rows.iter().filter(|row| row.status.can_buy).count();
    let gated = rows.iter().filter(|row| !row.status.level_visible).count();
    let doubled: Vec<String> = rows
        .iter()
        .filter(|row| row.doubled)
        .map(|row| row.name.clone())
        .collect();
    let next = rows.iter().find(|row| row.status.below_max);
    let muted = cx.theme().colors.fg_muted;

    let mut body = div().id("jelly-upgrades").flex().flex_col().gap_3().child(
        div()
            .id("jelly-upgrades-kpis")
            .flex()
            .flex_wrap()
            .gap_3()
            .child(
                KpiCard::new(stat(
                    "jelly-upgrade-affordable",
                    "Affordable now",
                    affordable as f64,
                ))
                .caption(format!("of {} purchase slots", rows.len())),
            )
            .child(
                KpiCard::new(stat("jelly-upgrade-gated", "Hidden slots", gated as f64)).caption(
                    format!(
                        "Research Lv {} reveals the rest",
                        format::number(jelly.account.research_level)
                    ),
                ),
            )
            .child(
                KpiCard::new(stat(
                    "jelly-upgrade-doubled",
                    "Doubled upgrades",
                    doubled.len() as f64,
                ))
                .caption("Menagerie Upgrade Doubler"),
            )
            .child(
                KpiCard::new(stat(
                    "jelly-upgrade-divisor",
                    "Cost divisor",
                    1.0 + jelly.upgrade_quantity(34) / 100.0,
                ))
                .caption("Lower Cholesterol (upgrade 34)"),
            ),
    );
    if let Some(next) = next {
        body = body.child(
            Card::new()
                .header(
                    CardHeader::new(format!("Next slot: #{} {}", next.order + 1, next.name))
                        .description(next.description.clone()),
                )
                .child(div().p_3().child(Caption::new(format!(
                    "{} × {} · {} · {} Bloodcells saved",
                    next.per_level,
                    next.effect,
                    cost_text(next.cost),
                    format::value(jelly.progress.bloodcells)
                )))),
        );
    }
    if !doubled.is_empty() {
        body = body.child(
            Card::new()
                .header(
                    CardHeader::new("Upgrade Doubler")
                        .description("Spelunk[53] doubles these upgrades' per-level quantity"),
                )
                .child(
                    div()
                        .id("jelly-upgrade-doublers")
                        .p_3()
                        .child(Caption::new(doubled.join(" · ")).text_color(muted)),
                ),
        );
    }
    if gated > 0 {
        body = body.child(
            div().id("jelly-upgrade-gate").child(
                Callout::new(Severity::Info)
                    .title(format!("{gated} slots are not revealed yet"))
                    .child(Caption::new(format!(
                        "The purchase order reveals a slot at Research Lv {}. Their prices and \
                         effects read from the save anyway, so a plan can see what is coming.",
                        format::number(
                            rows.iter()
                                .filter(|row| !row.status.level_visible)
                                .map(|row| row.status.required_research_level)
                                .fold(f64::INFINITY, f64::min)
                        )
                    ))),
            ),
        );
    }

    let mut table = Table::new([
        "#",
        "Upgrade and game description",
        "Level",
        "Current effect",
        "Gain / level",
        "Next cost",
    ]);
    for row in &rows {
        let mut name = div()
            .id(format!("jelly-upgrade-row-{}", row.order))
            .flex()
            .flex_col()
            .gap_0p5()
            .child(row.name.clone());
        if row.doubled {
            name = name.child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Badge::new("2x Doubled").tone(Tone::Info))
                    .child(Caption::new("Upgrade Doubler").text_color(muted)),
            );
        }
        if !row.status.level_visible {
            name = name.child(
                Badge::new(format!(
                    "Research Lv {}",
                    format::number(row.status.required_research_level)
                ))
                .tone(Tone::Neutral),
            );
        }
        name = name.child(Caption::new(row.description.clone()).text_color(muted));
        if !row.note.is_empty() {
            name = name.child(Caption::new(row.note.clone()).text_color(muted));
        }
        table = table.row([
            div().child(format!("{}", row.order + 1)).into_any_element(),
            name.into_any_element(),
            div()
                .child(if row.max_level > 998.0 {
                    format::number(row.level)
                } else {
                    format!(
                        "{} / {}",
                        format::number(row.level),
                        format::number(row.max_level)
                    )
                })
                .into_any_element(),
            div().child(row.effect.clone()).into_any_element(),
            div().child(row.per_level.clone()).into_any_element(),
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(cost_text(row.cost))
                .child(if row.status.can_buy {
                    Badge::new("Affordable").tone(Tone::Success)
                } else if row.status.below_max {
                    Badge::new("Saving").tone(Tone::Neutral)
                } else {
                    Badge::new("Maxed").tone(Tone::Neutral)
                })
                .into_any_element(),
        ]);
    }
    body.child(
        Card::new()
            .header(CardHeader::new("Upgrade reference").description(
                "Every purchase slot in the game's own order; the price curve is the slot's, \
                 not the upgrade's",
            ))
            .child(div().id("jelly-upgrades-table").p_3().child(table)),
    )
    .into_any_element()
}

/// A price, or a dash when the order names a slot the game has not filled in.
fn cost_text(cost: f64) -> String {
    if cost.is_finite() {
        format!("{} Bloodcells", format::value(cost))
    } else {
        "—".to_string()
    }
}
