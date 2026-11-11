//! The Fountain Calculator: the upstream Overview tab.
//!
//! Ported from `fountain-calc.html` and `js/stats/systems/w5/fountain.js`. FN-01
//! builds the overview: the fountain summary, the fill and income rates, lucky
//! coins and ducks, the ground coin stacks and the cross-system bonuses. The
//! currency, upgrade, planner and marble tabs arrive with FN-02 to FN-04, so the
//! page renders its one built pane without a tab strip until then.

mod bonuses;
mod overview;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, Window, div};

/// The fountain section: the overview pane, or the import prompt when the loaded
/// save carries no cavern.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    if state.fountain.is_none() {
        return prompt(cx);
    }
    div()
        .id("fountain-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("fountain-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(overview::overview(state, cx)),
        )
        .into_any_element()
}

/// A save without `Holes` has no fountain state: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("fountain-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("fountain-empty", IconName::Map, "No fountain state")
                .body(
                    "The fountain calculator reads the cavern from a save. Import one to see the \
                     fountain level, income rates, lucky coins and the ground stacks.",
                )
                .action(
                    Button::new("fountain-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
