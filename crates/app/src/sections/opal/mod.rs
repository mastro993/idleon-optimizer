//! The Opal Optimizer (W5): the current villagers, their EXP rates and the
//! account bonuses behind them.
//!
//! OP-01 builds the upstream `Current State` tab: the villager overview and
//! table, the current opal state and the shared formula tree, with a villager
//! picker that switches the tree between the total and one villager. The
//! optimizer (OP-02) and the manual simulator (OP-03) arrive with later
//! features, so the page renders its one built pane without a tab strip.
//!
//! Every figure comes from the import, so the section keeps only its selection.

mod bonuses;
mod overview;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

/// The section: the villager selector and the two panes under it.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let Some(opal) = state.opal.clone() else {
        return prompt(cx);
    };
    let tree = state.opal_breakdown.clone();
    tree.update(cx, |tree, cx| {
        tree.set_root(opal.breakdown(state.opal_selected), cx)
    });
    div()
        .id("opal-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("opal-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("opal-overview")
                        .flex()
                        .flex_col()
                        .gap_3()
                        .p_3()
                        .child(overview::current_state(&opal, cx))
                        .child(overview::villagers(state, &opal, cx))
                        .child(bonuses::terms(&opal, cx))
                        .child(bonuses::formula(state, tree)),
                ),
        )
        .into_any_element()
}

/// A save without `Holes` has no villagers: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("opal-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("opal-empty", IconName::Map, "No villager state")
                .body(
                    "The opal optimizer reads the five villagers, their levels and their invested \
                     opals from a save. Import one to see the EXP rates and every bonus term.",
                )
                .action(
                    Button::new("opal-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
