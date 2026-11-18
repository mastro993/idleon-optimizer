//! The Upgrades tab: the `Research[44]` purchase list, what the next level
//! costs and what the save's own levels buy.
//!
//! One row per purchase slot with its game description, level, current effect,
//! gain per level and next price. A slot the Research level has not revealed
//! yet says so instead of printing a price the game hides, and the doubler card
//! names every upgrade Menagerie's Upgrade Doubler covers.
//!
//! The game lists a fixed 40 slots, read once per import; each row wraps its
//! description and carries tags, so it stays a plain kit `Table`.

use super::parts::{INDEX_WIDTH, cell, number_cell, stat, strips};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::table::{Table, TableBody, TableHead, TableHeader, TableRow};
use gpui_kit::component::tag::Tag;
use idleon_jelly::{Jelly, UpgradeRow};
use idleon_ui::format;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, PANEL_GAP, Tone};

/// The Upgrades pane.
pub(crate) fn pane(state: &AppState, jelly: &Jelly) -> AnyElement {
    let rows = &state.jelly_state.save.upgrades;
    let affordable = rows.iter().filter(|row| row.status.can_buy).count();
    let gated = rows.iter().filter(|row| !row.status.level_visible).count();
    let doubled: Vec<&str> = rows
        .iter()
        .filter(|row| row.doubled)
        .map(|row| row.name.as_str())
        .collect();
    let next = rows.iter().find(|row| row.status.below_max);
    let gate = rows
        .iter()
        .filter(|row| !row.status.level_visible)
        .map(|row| row.status.required_research_level)
        .fold(f64::INFINITY, f64::min);

    let mut body = div()
        .id("jelly-upgrades")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(strips(
            "jelly-upgrades-kpis",
            vec![
                stat(
                    "jelly-upgrade-affordable",
                    "Affordable now",
                    affordable as f64,
                )
                .caption(format!("of {} purchase slots", rows.len())),
                stat("jelly-upgrade-gated", "Hidden slots", gated as f64).caption(
                    if gate.is_finite() {
                        format!("Research Lv {} reveals the rest", format::number(gate))
                    } else {
                        "Every slot is revealed".to_string()
                    },
                ),
                stat(
                    "jelly-upgrade-doubled",
                    "Doubled upgrades",
                    doubled.len() as f64,
                )
                .caption("Menagerie Upgrade Doubler"),
                stat(
                    "jelly-upgrade-divisor",
                    "Cost divisor",
                    1.0 + jelly.upgrade_quantity(34) / 100.0,
                )
                .caption("Lower Cholesterol (upgrade 34)"),
            ],
        ));
    if let Some(next) = next {
        body = body.child(
            Card::new()
                .header(
                    CardHeader::new(format!("Next slot: #{} {}", next.order + 1, next.name))
                        .description(next.description.clone()),
                )
                .child(Caption::new(format!(
                    "{} · current {} · {} · {} saved",
                    next.per_level,
                    next.effect,
                    cost_text(next.cost),
                    format::value(jelly.progress.bloodcells)
                ))),
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
                        .child(Caption::new(doubled.join(" · "))),
                ),
        );
    }
    if gated > 0 {
        body = body.child(div().id("jelly-upgrade-gate").child(
            Callout::new(Tone::Info, format!("{gated} slots are not revealed yet")).child(
                Caption::new(format!(
                    "The purchase order reveals the next one at Research Lv {}. Their prices \
                         and effects read from the save anyway, so a plan can see what is coming.",
                    format::number(gate)
                )),
            ),
        ));
    }
    body.child(
        Card::new()
            .header(CardHeader::new("Upgrade reference").description(
                "Every purchase slot in the game's own order; the price curve is the slot's, \
                 not the upgrade's",
            ))
            .child(div().id("jelly-upgrades-table").child(table(rows))),
    )
    .into_any_element()
}

fn table(rows: &[UpgradeRow]) -> Table {
    let tag = |label: &'static str, tone: Tone| {
        Tag::new()
            .with_variant(tone.into())
            .outline()
            .small()
            .child(label)
    };
    Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(TableHead::new().w(INDEX_WIDTH).text_right().child("#"))
                    .child(TableHead::new().child("Upgrade and game description"))
                    .child(TableHead::new().text_right().child("Level"))
                    .child(TableHead::new().child("Current effect"))
                    .child(TableHead::new().child("Gain / level"))
                    .child(TableHead::new().text_right().child("Next cost")),
            ),
        )
        .child(TableBody::new().children(rows.iter().map(|row| {
            let name = div()
                .id(format!("jelly-upgrade-row-{}", row.order))
                .flex()
                .flex_col()
                .gap_0p5()
                .child(row.name.clone())
                .when(row.doubled, |name| {
                    name.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(tag("2x Doubled", Tone::Info))
                            .child(Caption::new("Upgrade Doubler")),
                    )
                })
                .when(!row.status.level_visible, |name| {
                    name.child(Caption::new(format!(
                        "Hidden until Research Lv {}",
                        format::number(row.status.required_research_level)
                    )))
                })
                .child(Caption::new(row.description.clone()))
                .when(!row.note.is_empty(), |name| {
                    name.child(Caption::new(row.note.clone()))
                });
            let level = if row.max_level > 998.0 {
                format::number(row.level)
            } else {
                format!(
                    "{} / {}",
                    format::number(row.level),
                    format::number(row.max_level)
                )
            };
            let status = if row.status.can_buy {
                tag("Affordable", Tone::Success)
            } else if row.status.below_max {
                tag("Saving", Tone::Neutral)
            } else {
                tag("Maxed", Tone::Neutral)
            };
            TableRow::new()
                .child(number_cell(format!("{}", row.order + 1)).w(INDEX_WIDTH))
                .child(cell(name))
                .child(number_cell(level))
                .child(cell(row.effect.clone()))
                .child(cell(row.per_level.clone()))
                .child(number_cell(
                    div()
                        .flex()
                        .flex_col()
                        .items_end()
                        .gap_0p5()
                        .child(cost_text(row.cost))
                        .child(status),
                ))
        })))
}

/// A price, or a dash when the order names a slot the game has not filled in.
fn cost_text(cost: f64) -> String {
    if cost.is_finite() {
        format!("{} Bloodcells", format::value(cost))
    } else {
        "—".to_string()
    }
}
