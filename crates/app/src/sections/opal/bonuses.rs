//! Every bonus term behind the rates, and the shared formula tree.

use crate::state::AppState;
use crate::ui::format;
use crate::ui::formula::FormulaTree;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, IntoElement, ParentElement, Styled, div, px};
use idleon_core::Opal;

/// The terms table: the chain, the outer multipliers and the additive pool.
pub fn terms(opal: &Opal, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let terms = &opal.ext.terms;
    let mut table = Table::new(["Group", "Term", "Value"]);
    for ((label, _), factor) in idleon_core::opal::CHAIN.iter().zip(terms.chain_factors()) {
        table = table.row([
            div().child("Chain").into_any_element(),
            div().child(*label).into_any_element(),
            div().child(format!("×{factor:.4}")).into_any_element(),
        ]);
    }
    for (label, factor) in terms.outer_factors() {
        table = table.row([
            div().child("Multiplier").into_any_element(),
            div().child(label).into_any_element(),
            div().child(format!("×{factor:.4}")).into_any_element(),
        ]);
    }
    for (label, value) in terms.additive_terms() {
        table = table.row([
            div().child("Additive").into_any_element(),
            div().child(label).into_any_element(),
            div()
                .child(format::percent(value / 100.0))
                .into_any_element(),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Bonus Terms").description(
                "Every term the page's buildExt reads, in its own order: the chain and the outer \
                 factors multiply, the additive pool sums",
            ),
        )
        .child(
            div()
                .id("opal-terms")
                .flex()
                .flex_col()
                .gap_2()
                .child(table)
                .child(
                    Caption::new(format!(
                        "Base {} × chain ×{:.4} × the outer multipliers = ×{:.4}; the additive pool \
                         sums to {} before a villager's own Opal Enthusiasm.",
                        format::value(terms.base_plus_upg),
                        terms.chain(),
                        terms.base_multiplier(),
                        format::percent(terms.additive() / 100.0)
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The shared formula tree, rooted at the selector's villager or the total.
pub fn formula(state: &AppState, tree: Entity<FormulaTree>) -> AnyElement {
    let title = match state.opal_selected {
        Some(villager) => format!(
            "{} EXP Rate Breakdown",
            idleon_core::opal::NAMES
                .get(villager)
                .copied()
                .unwrap_or("Villager")
        ),
        None => "Total EXP Rate Breakdown".to_owned(),
    };
    Card::new()
        .header(CardHeader::new(title).description("Search every factor and copy the full tree"))
        .child(
            div()
                .id("opal-formula-card-body")
                .h(px(320.0))
                .min_h_0()
                .child(tree),
        )
        .into_any_element()
}
