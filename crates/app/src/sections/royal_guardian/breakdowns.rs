//! The Breakdowns tab: the account-wide rules as the shared formula tree, one
//! category at a time.
//!
//! The upstream page opens this tree from any factor that names a global rule
//! (`data-breakdown-link`); here the category is a selector on the pane and the
//! tree below it is the same tree the page draws.

use crate::state::AppState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, SharedString, Styled, div, px};
use idleon_royal_guardian::Category;

/// The Breakdowns pane.
pub(crate) fn breakdowns(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let royal = state
        .royal
        .as_ref()
        .expect("the section renders its import prompt without a save");
    let category = state.royal_state.breakdown;
    let breakdown = royal.global_breakdown(category);
    let tree = state.royal_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(breakdown.root.clone(), cx));
    let muted = cx.theme().colors.fg_muted;
    let mut pane = div()
        .id("royal-breakdowns")
        .flex()
        .flex_col()
        .gap_3()
        .child(category_rail(category, cx));
    if breakdown.partial {
        pane = pane.child(
            Card::new()
                .header(
                    CardHeader::new("This breakdown is partial")
                        .description(breakdown.missing.join(", ")),
                )
                .child(
                    div()
                        .id("royal-breakdown-warning")
                        .child(Caption::new(
                            "Some saved inputs are unavailable, so a factor reads at one instead \
                             of being guessed.",
                        ))
                        .text_color(muted),
                ),
        );
    }
    let summary = format!(
        "{}: {} across {} nodes",
        category.label(),
        breakdown.root.format.format(breakdown.value),
        breakdown.root.node_count()
    );
    pane.child(
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
                    .child(tree),
            ),
    )
    .into_any_element()
}

/// The seven categories, as the pane's rail.
fn category_rail(category: Category, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("royal-breakdown-tabs").selected(SharedString::from(format!(
        "royal-breakdown-{}",
        category.id()
    )));
    for entry in Category::ALL {
        tabs = tabs.tab(WindowTab::new(
            format!("royal-breakdown-{}", entry.id()),
            entry.label(),
        ));
    }
    let tabs = tabs.on_select(move |id, _window, cx| {
        let category = id
            .as_ref()
            .strip_prefix("royal-breakdown-")
            .and_then(Category::from_id);
        if let Some(category) = category {
            entity.update(cx, |state, cx| state.set_royal_breakdown(category, cx));
        }
    });
    div()
        .id("royal-breakdown-rail")
        .flex()
        .items_center()
        .child(tabs)
        .into_any_element()
}
