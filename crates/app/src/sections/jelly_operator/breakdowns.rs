//! The Breakdowns tab: the four multipliers the board reads and the layout's two
//! passives, as the shared formula tree, plus the page's per-cell output table.
//!
//! Ported from `breakdownsHtml` and `initializeBreakdownTrees` in
//! `jelly-operator-calc.html`. The page draws six trees side by side; the port
//! shows one at a time behind a rail, and every tree is the same
//! [`FormulaTree`](idleon_ui::formula::FormulaTree) the other calculators use.
//!
//! A breakdown the save cannot compute (Cell EXP without Cell Biology,
//! Bloodcells without Bloodcell Coagulation, a board the rules refuse) prints a
//! warning naming what is missing instead of a guessed factor.

use super::estimate::{stat, text_kpi};
use crate::state::AppState;
use ely_gpui_component::data_display::KpiCard;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_jelly::{BreakdownCategory, Jelly, MetricOptions};
use idleon_ui::format;

/// The breakdowns pane.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let category = state.jelly_state.breakdown;
    let options = MetricOptions {
        fever: Some(jelly.progress.fever),
        ..MetricOptions::default()
    };
    let breakdown = jelly.breakdown(category, &state.jelly_state.layout, options);
    let tree = state.jelly_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(breakdown.root.clone(), cx));
    let muted = cx.theme().colors.fg_muted;
    let layout = jelly.layout_breakdown(&state.jelly_state.layout, options.fever);
    let summary = format!(
        "{}: {} × across {} nodes",
        category.label(),
        format::value(breakdown.root.value),
        breakdown.root.node_count()
    );

    let mut body = div()
        .id("jelly-breakdowns")
        .flex()
        .flex_col()
        .gap_3()
        .child(rail(category, cx))
        .child(
            div()
                .id("jelly-breakdown-head")
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(
                    KpiCard::new(stat(
                        "jelly-breakdown-value",
                        category.label(),
                        breakdown.root.value,
                    ))
                    .caption(format!(
                        "{} · {} nodes",
                        category.id(),
                        breakdown.root.node_count()
                    )),
                )
                .child(
                    div()
                        .id("jelly-breakdown-summary")
                        .role(gpui::Role::Label)
                        .aria_label(summary.clone())
                        .child(summary),
                )
                .child(Caption::new(description(category)).text_color(muted)),
        );
    if !breakdown.available {
        body = body.child(
            div().id("jelly-breakdown-warning").child(
                Callout::new(Severity::Warning)
                    .title("This breakdown is not available")
                    .child(Caption::new(breakdown.reason.clone())),
            ),
        );
    }
    body = body.child(
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
                    .child(tree),
            ),
    );
    if layout.available {
        body = body.child(per_cell(&layout, cx));
    }
    body.into_any_element()
}

/// The six trees, as the pane's rail.
fn rail(category: BreakdownCategory, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("jelly-breakdown-tabs").selected(SharedString::from(format!(
        "jelly-breakdown-{}",
        category.id()
    )));
    for entry in BreakdownCategory::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("jelly-breakdown-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let category = id
            .as_ref()
            .strip_prefix("jelly-breakdown-")
            .and_then(BreakdownCategory::from_id);
        if let Some(category) = category {
            entity.update(cx, |state, cx| state.set_jelly_breakdown(category, cx));
        }
    });
    div()
        .id("jelly-breakdown-rail")
        .flex()
        .items_center()
        .child(tabs)
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
fn per_cell(layout: &idleon_jelly::LayoutBreakdown, cx: &mut Context<AppState>) -> AnyElement {
    let metrics = &layout.metrics;
    let rows = layout.cell_rows();
    let mut table = Table::new(["Cell type", "Placed", "Average hit", "Attack rate", "DPS"]);
    for row in &rows {
        table = table.row([
            div()
                .id(format!("jelly-cell-row-{}", row.cell_type))
                .child(row.name.clone())
                .into_any_element(),
            div()
                .child(format::number(row.placed as f64))
                .into_any_element(),
            div()
                .child(format::value(row.average_hit))
                .into_any_element(),
            div()
                .child(format!("{}/sec", format::value(row.attacks_per_second)))
                .into_any_element(),
            div().child(format::value(row.dps)).into_any_element(),
        ]);
    }
    div()
        .id("jelly-breakdown-cells")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(
                    KpiCard::new(stat("jelly-breakdown-dps", "Total steady DPS", metrics.dps))
                        .caption(format!(
                            "{}/sec attack rate",
                            format::value(metrics.attacks_per_second)
                        )),
                )
                .child(text_kpi(
                    "jelly-damage-passive",
                    "Damage passive",
                    format!("{:.3}x", layout.damage_multiplier()),
                    "Amoeba, Ribosome, Gigacyst and virus reach".to_string(),
                    cx,
                ))
                .child(text_kpi(
                    "jelly-speed-passive",
                    "Speed passive",
                    format!("{:.3}x", layout.speed_multiplier()),
                    "Plasmid, Organelle and Mitochondria".to_string(),
                    cx,
                ))
                .child(
                    KpiCard::new(stat(
                        "jelly-virus-multiplier",
                        "Virus multiplier",
                        metrics.virus_multiplier,
                    ))
                    .caption(format!("{} infected slots", metrics.infected_slots)),
                ),
        )
        .child(
            Card::new()
                .header(
                    CardHeader::new("Per-cell output")
                        .description("One row per shipped cell type over the board on screen"),
                )
                .child(div().p_3().child(table)),
        )
        .into_any_element()
}
