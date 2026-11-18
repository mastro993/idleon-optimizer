//! The Breakdowns tab: the account-wide rules as the shared formula tree, one
//! category at a time.
//!
//! The category is a tab rail on the pane and the tree below it is the same
//! tree the page draws. [`AppState::sync_royal`] builds the breakdown when the
//! category or the save changes, so a frame only draws it.

use crate::state::AppState;
use crate::ui::layout::tab_bar;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, div, px};
use gpui_kit::component::v_flex;
use idleon_royal_guardian::Category;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP};

/// The Breakdowns pane.
pub(crate) fn breakdowns(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let category = state.royal_state.breakdown;
    let Some((_, breakdown)) = state.royal_controls.breakdown.as_ref() else {
        return div().id("royal-breakdowns").into_any_element();
    };
    let summary = format!(
        "{}: {} across {} nodes",
        category.label(),
        breakdown.root.format.format(breakdown.value),
        breakdown.root.node_count()
    );
    v_flex()
        .id("royal-breakdowns")
        .gap(PANEL_GAP)
        .child(category_rail(category, cx))
        .when(breakdown.partial, |pane| {
            pane.child(
                Card::new()
                    .header(
                        CardHeader::new("This breakdown is partial")
                            .description(breakdown.missing.join(", ")),
                    )
                    .child(div().id("royal-breakdown-warning").child(Caption::new(
                        "Some saved inputs are unavailable, so a factor reads at one instead of \
                         being guessed.",
                    ))),
            )
        })
        .child(
            div()
                .id("royal-breakdown-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .child(summary),
        )
        .child(
            Card::new()
                .header(
                    CardHeader::new(category.label())
                        .description("Every factor the rule multiplies, with its source"),
                )
                .child(
                    div()
                        .id("royal-breakdown-panel")
                        .h(px(520.0))
                        .min_h_0()
                        .child(state.royal_breakdown.clone()),
                ),
        )
        .into_any_element()
}

/// The seven categories, as the pane's rail.
fn category_rail(category: Category, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let selected = Category::ALL
        .iter()
        .position(|entry| *entry == category)
        .unwrap_or(0);
    let tabs = Category::ALL.map(|entry| {
        (
            SharedString::from(format!("royal-breakdown-{}", entry.id())),
            SharedString::from(entry.label()),
        )
    });
    div()
        .id("royal-breakdown-rail")
        .flex()
        .child(
            tab_bar(
                "royal-breakdown-tabs",
                tabs,
                selected,
                move |index, _window, cx| {
                    entity.update(cx, |state, cx| {
                        state.set_royal_breakdown(Category::ALL[index], cx);
                    });
                },
            )
            .flex_1(),
        )
        .into_any_element()
}
