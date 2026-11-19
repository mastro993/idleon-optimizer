//! Every bonus term behind the rates, and the shared formula tree.

use super::table::{Col, note, table, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, IntoElement, ParentElement, Styled, div, px};
use gpui_kit::component::v_flex;
use idleon_opal::Opal;
use idleon_ui::format;
use idleon_ui::widgets::{Card, CardHeader};

/// The terms table: the chain, the outer multipliers and the additive pool.
pub(crate) fn terms(opal: &Opal, cx: &App) -> AnyElement {
    let terms = &opal.ext.terms;
    let row =
        |group: &'static str, label: String, value: String| [text(group), text(label), text(value)];
    let rows =
        idleon_opal::CHAIN
            .iter()
            .zip(terms.chain_factors())
            .map(|((label, _), factor)| row("Chain", (*label).to_string(), format!("×{factor:.4}")))
            .chain(terms.outer_factors().into_iter().map(|(label, factor)| {
                row("Multiplier", label.to_string(), format!("×{factor:.4}"))
            }))
            .chain(terms.additive_terms().into_iter().map(|(label, value)| {
                row(
                    "Additive",
                    label.to_string(),
                    format::percent(value / 100.0),
                )
            }));
    let table = table(
        "opal-terms",
        &[Col::Text("Group"), Col::Text("Term"), Col::Number("Value")],
        rows,
        cx,
    );
    Card::new()
        .header(CardHeader::new("Bonus Terms").description(
            "Every term the villagers' EXP rates read, in their own order: the chain and the outer \
                 factors multiply, the additive pool sums",
        ))
        .child(v_flex().id("opal-terms").gap_2().child(table).child(note(
            format!(
                "Base {} × chain ×{:.4} × the outer multipliers = ×{:.4}; the additive pool \
                         sums to {} before a villager's own Opal Enthusiasm.",
                format::value(terms.base_plus_upg),
                terms.chain(),
                terms.base_multiplier(),
                format::percent(terms.additive() / 100.0)
            ),
            cx,
        )))
        .into_any_element()
}

/// The shared formula tree, rooted at the selector's villager or the total.
pub(crate) fn formula(state: &AppState) -> AnyElement {
    let title = match state.opal.selected {
        Some(villager) => format!(
            "{} EXP Rate Breakdown",
            idleon_opal::NAMES
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
                .child(state.opal.breakdown.clone()),
        )
        .into_any_element()
}
