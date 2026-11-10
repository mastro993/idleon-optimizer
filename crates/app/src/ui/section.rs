//! The open section: research's tabbed panes, or a planned placeholder.
//!
//! The research section renders its own tab strip; every other dashboard card
//! lands on a placeholder until its milestone lands.

use crate::state::AppState;
use crate::ui::rates;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};

/// The research section: its three upstream tabs and the active pane.
pub fn research_section(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    match state.tab {
        crate::state::Tab::Grid => div()
            .id("pane-grid")
            .flex()
            .size_full()
            .gap_3()
            .min_h_0()
            .child(
                div()
                    .id("grid-column")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(crate::ui::grid_view::grid_pane(state, window, cx)),
            )
            .child(
                div()
                    .id("inspector-column")
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(360.0))
                    .min_h_0()
                    .child(crate::ui::inspector::inspector_pane(state, window, cx)),
            )
            .into_any_element(),
        crate::state::Tab::Rates => div()
            .id("pane-rates")
            .size_full()
            .min_h_0()
            .child(rates::rates_pane(state, window, cx))
            .into_any_element(),
        crate::state::Tab::Run => div()
            .id("pane-run")
            .size_full()
            .min_h_0()
            .child(crate::ui::optimizer::optimizer_pane(state, window, cx))
            .into_any_element(),
    }
}

/// A calm placeholder whose root id proves the card opened the right section.
pub fn placeholder_section(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let Some(section) = state.current_section() else {
        return div().id("section-unknown").into_any_element();
    };
    let entity = cx.entity();
    let back = entity.clone();
    div()
        .id(format!("section-{}", section.id))
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_4()
        .child(
            Card::new()
                .header(
                    CardHeader::new(section.title)
                        .description(section.description)
                        .action(Badge::new(section.badge())),
                )
                .child(
                    EmptyState::new(
                        format!("section-planned-{}", section.id),
                        IconName::Map,
                        "On the roadmap",
                    )
                    .body(format!(
                        "{} is specified in docs/sections/{}.md and has not been built yet. The research optimizer is available today.",
                        section.title, section.id
                    ))
                    .action(
                        Button::new("section-back", "Back to dashboard")
                            .icon(IconName::ArrowLeft)
                            .on_click(move |_event, _window, cx| {
                                back.update(cx, |state, cx| state.back_to_dashboard(cx));
                            }),
                    ),
                ),
        )
        .into_any_element()
}
