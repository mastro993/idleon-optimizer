//! The formula trees behind the rates: the AFK panel rate and one claim's rolls.

use crate::state::AppState;
use crate::ui::formula::FormulaTree;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled, div};
use idleon_core::ladle::Stats;

/// The two rate trees, each in its own card around the shared tree component.
pub fn ladles(state: &mut AppState, stats: &[Stats], cx: &mut Context<AppState>) -> AnyElement {
    let chosen = state
        .ladle_state
        .breakdown_character
        .min(stats.len().saturating_sub(1));
    let Some(row) = stats.get(chosen).cloned() else {
        return div().into_any_element();
    };
    state
        .ladle_state
        .ladles_tree
        .update(cx, |tree, cx| tree.set_root(row.ladles_tree(), cx));
    state
        .ladle_state
        .claims_tree
        .update(cx, |tree, cx| tree.set_root(row.claims_tree(), cx));
    let ladles = state.ladle_state.ladles_tree.clone();
    let claims = state.ladle_state.claims_tree.clone();
    div()
        .id("ladle-breakdowns")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .id("ladle-breakdown-character")
                .role(gpui::Role::Label)
                .aria_label(format!("Breakdowns for {}", row.name))
                .child(Caption::new(format!(
                    "Breakdowns for {} · click a branch to expand, or search the nodes",
                    row.name
                ))),
        )
        .child(panel(
            "ladle-breakdown-ladles",
            "Ladles / day (AFK panel)",
            "15 × Ladle Multi × AFK gain rate",
            &ladles,
        ))
        .child(panel(
            "ladle-breakdown-claims",
            "Expected ladles from claims",
            "One claim every window, floored, then the Post Office 19c roll",
            &claims,
        ))
        .into_any_element()
}

/// One breakdown card around a shared formula tree.
pub fn panel(
    id: &'static str,
    title: &'static str,
    description: &'static str,
    tree: &Entity<FormulaTree>,
) -> AnyElement {
    Card::new()
        .header(CardHeader::new(title).description(description))
        .child(div().id(id).child(tree.clone()))
        .into_any_element()
}
