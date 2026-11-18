//! The Breakdowns tab: the four multipliers the board reads and the layout's two
//! passives, as the shared formula tree, plus the page's per-cell output table.
//!
//! The six trees show one at a time behind a rail, and every tree is the same
//! [`FormulaTree`](idleon_ui::formula::FormulaTree) the other calculators use.
//! The section reads the tree and the per-cell output again only when the board
//! or the picked tree changes (`derived::BoardFigures`).
//!
//! A breakdown the save cannot compute (Cell EXP without Cell Biology,
//! Bloodcells without Bloodcell Coagulation, a board the rules refuse) prints a
//! warning naming what is missing instead of a guessed factor.

use super::parts::{figure, stat, strips, text_kpi, text_table};
use crate::state::AppState;
use crate::ui::layout::tab_bar;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_jelly::{BreakdownCategory, CellRow, LayoutBreakdown};
use idleon_ui::format;
use idleon_ui::widgets::{Callout, Caption, Card, CardHeader, PANEL_GAP, Tone};

/// The breakdowns pane.
pub(crate) fn pane(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let category = state.jelly_state.breakdown;
    let figures = &state.jelly_state.board;
    let body = div()
        .id("jelly-breakdowns")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(rail(category, cx));
    let Some(breakdown) = figures.breakdown.as_ref() else {
        return body.into_any_element();
    };
    if !breakdown.available {
        // The locked text prints instead of a tree, so a factor the save
        // cannot compute never reads as a number.
        return body
            .child(
                div().id("jelly-breakdown-warning").child(
                    Callout::new(Tone::Warning, "This breakdown is not available")
                        .child(Caption::new(format!(
                            "{}.",
                            breakdown.reason.trim_end_matches('.')
                        )))
                        .child(Caption::new(description(category))),
                ),
            )
            .into_any_element();
    }
    let summary = format!(
        "{}: {} × across {} nodes",
        category.label(),
        format::value(breakdown.root.value),
        breakdown.root.node_count()
    );
    body.child(
        div()
            .id("jelly-breakdown-head")
            .flex()
            .flex_wrap()
            .items_center()
            .gap(PANEL_GAP)
            .child(strips(
                "jelly-breakdown-figure",
                vec![
                    stat(
                        "jelly-breakdown-value",
                        category.label(),
                        breakdown.root.value,
                    )
                    .caption(format!(
                        "{} · {} nodes",
                        category.id(),
                        breakdown.root.node_count()
                    )),
                ],
            ))
            .child(
                div()
                    .id("jelly-breakdown-summary")
                    .role(gpui::Role::Label)
                    .aria_label(summary.clone())
                    .child(summary),
            )
            .child(Caption::new(description(category))),
    )
    .child(
        Card::new()
            .header(
                CardHeader::new(category.label())
                    .description("Every factor the multiplier reads, with its own inputs"),
            )
            .child(
                div()
                    .id("jelly-breakdown-panel")
                    .h(px(460.0))
                    .min_h_0()
                    .child(state.jelly_breakdown.clone()),
            ),
    )
    .when(figures.layout.available, |body| {
        body.child(per_cell(&figures.layout, &figures.cells))
    })
    .into_any_element()
}

/// The six trees, as the pane's rail.
fn rail(category: BreakdownCategory, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let selected = BreakdownCategory::ALL
        .iter()
        .position(|entry| *entry == category)
        .unwrap_or(0);
    let tabs = tab_bar(
        "jelly-breakdown-tabs",
        BreakdownCategory::ALL.map(|entry| {
            (
                format!("jelly-breakdown-{}", entry.id()).into(),
                entry.label().into(),
            )
        }),
        selected,
        move |index, _window, cx| {
            let category = BreakdownCategory::ALL[index];
            entity.update(cx, |state, cx| state.set_jelly_breakdown(category, cx));
        },
    );
    div()
        .id("jelly-breakdown-rail")
        .flex()
        .items_center()
        .child(div().flex_1().min_w_0().child(tabs))
        .into_any_element()
}

/// What one tree means, as the page's section notes read.
fn description(category: BreakdownCategory) -> &'static str {
    match category {
        BreakdownCategory::Damage => {
            "Every cell's hit before its own level bonus, the board passives and the virus reach."
        }
        BreakdownCategory::Speed => {
            "The attack-cadence multiplier the selected Fever adds to every cell."
        }
        BreakdownCategory::Exp => {
            "Cell EXP per hit, once Cell Biology unlocks it; the levels scale it further."
        }
        BreakdownCategory::Bloodcells => {
            "Bloodcells per point of damage, from the additive gain and every account factor."
        }
        BreakdownCategory::LayoutDamage => {
            "The board's own damage passives: Amoebas, Ribosomes, Gigacysts and virus reach."
        }
        BreakdownCategory::LayoutSpeed => {
            "The board's own speed passives: Plasmids, Organelles and Mitochondria."
        }
    }
}

/// The page's per-cell output table and the board's steady-state headline.
fn per_cell(layout: &LayoutBreakdown, rows: &[CellRow]) -> AnyElement {
    let metrics = &layout.metrics;
    let table = text_table(
        &[
            ("Cell type", false),
            ("Placed", true),
            ("Average hit", true),
            ("Attack rate", true),
            ("DPS", true),
        ],
        rows.iter().map(|row| {
            vec![
                row.name.clone(),
                format::number(row.placed as f64),
                format::value(row.average_hit),
                format!("{}/sec", format::value(row.attacks_per_second)),
                format::value(row.dps),
            ]
        }),
    );
    div()
        .id("jelly-breakdown-cells")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(strips(
            "jelly-breakdown-kpis",
            vec![
                stat("jelly-breakdown-dps", "Total steady DPS", metrics.dps).caption(format!(
                    "{}/sec attack rate",
                    format::value(metrics.attacks_per_second)
                )),
                text_kpi(
                    "jelly-damage-passive",
                    "Damage passive",
                    format!("{:.3}x", layout.damage_multiplier()),
                    "Amoeba, Ribosome, Gigacyst and virus reach".to_string(),
                ),
                text_kpi(
                    "jelly-speed-passive",
                    "Speed passive",
                    format!("{:.3}x", layout.speed_multiplier()),
                    "Plasmid, Organelle and Mitochondria".to_string(),
                ),
                figure(
                    "jelly-virus-multiplier",
                    "Virus multiplier",
                    metrics.virus_multiplier,
                    0,
                )
                .caption(format!("{} infected slots", metrics.infected_slots)),
            ],
        ))
        .child(
            Card::new()
                .header(
                    CardHeader::new("Per-cell output")
                        .description("One row per shipped cell type over the board on screen"),
                )
                .child(table),
        )
        .into_any_element()
}
