//! The Royal Guardian Calculator: the upstream Overview tab.
//!
//! Ported from `royal-guardian-calc.html` and
//! `js/stats/systems/w7/royal-guardian.js`. RG-01 builds the overview: the
//! counters, the world progress cards, the raw clearing values every outpost
//! reports and the range target behind an outpost's Advanced Logistics plan.
//! The outpost map and the currencies arrive with RG-02, the upgrades with
//! RG-03 and the optimizer with RG-04, so the page renders its one built pane
//! without a tab strip until then.

mod overview;

#[cfg(test)]
mod tests;

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, Styled, Window, div};

/// The section's own state, kept across dashboard visits.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    /// The world the outpost tables list.
    pub world: usize,
    /// The built outpost whose next range target the panel explains.
    pub outpost: Option<usize>,
}

/// The Royal Guardian section: the overview pane, or the import prompt when the
/// loaded save carries no Royal Guardian rows.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    if state.royal.is_none() {
        return prompt(cx);
    }
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            ScrollArea::new("royal-pane")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(overview::overview(state, cx)),
        )
        .into_any_element()
}

/// A save without `RoyalG` has no outposts: say so and offer the import.
fn prompt(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .id("royal-guardian-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(
            EmptyState::new("royal-empty", IconName::Map, "No Royal Guardian state")
                .body(
                    "The Royal Guardian calculator reads the outposts from a save. Import one to \
                     see the counters, the world progress, the raw clearing values and the range \
                     targets.",
                )
                .action(
                    Button::new("royal-import", "Import save")
                        .icon(IconName::Upload)
                        .on_click(move |_event, window, cx| {
                            import.update(cx, |state, cx| state.open_import(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
