//! The formula trees behind the rates: the AFK panel rate and one claim's rolls.
//!
//! The trees' roots are set in [`super::State::sync`] when the rates or the
//! chosen character change; render only places them.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Entity, IntoElement, ParentElement, Styled, div};
use idleon_ladle::Stats;
use idleon_ui::formula::FormulaTree;
use idleon_ui::widgets::{Caption, Card, CardHeader};

/// The two rate trees, each in its own card around the shared tree component.
pub(crate) fn ladles(state: &AppState, stats: &[Stats]) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let Some(row) = stats.get(ladle_state.chosen()) else {
        return div().into_any_element();
    };
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
            &ladle_state.ladles_tree,
        ))
        .child(panel(
            "ladle-breakdown-claims",
            "Expected ladles from claims",
            "One claim every window, floored, then the Post Office 19c roll",
            &ladle_state.claims_tree,
        ))
        .into_any_element()
}

/// One breakdown card around a shared formula tree.
pub(crate) fn panel(
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
