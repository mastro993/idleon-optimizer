//! Shared section states: sections that are planned, and ids the registry lacks.
//!
//! Both render an empty state with a way back. Neither can panic: the router
//! resolves every id against the registry before the shell picks a renderer.

use crate::state::AppState;
use crate::ui::theme::WorldBadge;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

/// A section the registry declares as planned: its spec exists, the module does not.
pub fn planned(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(section) = state.current_section() else {
        return div().into_any_element();
    };
    let back = cx.entity();
    div()
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
                        .action(WorldBadge::new(section.group)),
                )
                .child(
                    EmptyState::new(
                        format!("section-planned-{}", section.id),
                        IconName::Map,
                        "On the roadmap",
                    )
                    .body(format!(
                        "{} is specified in {} and has not been built yet. The research optimizer is available today.",
                        section.title, section.spec
                    ))
                    .action(
                        Button::new("section-back", "Back to dashboard")
                            .icon(IconName::ArrowLeft)
                            .on_click(move |_event, window, cx| {
                                back.update(cx, |state, cx| state.back_to_dashboard(window, cx));
                            }),
                    ),
                ),
        )
        .into_any_element()
}

/// The route's id is not in the registry: say so, name the id, and offer the way back.
pub fn missing(id: &str, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let back = entity.clone();
    div()
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_4()
        .child(
            Card::new()
                .header(CardHeader::new("Unknown section").description(id.to_string()))
                .child(
                    EmptyState::new("section-missing", IconName::SearchX, "No such section")
                        .body(format!(
                            "The registry does not declare “{id}”. Check the id, or open a calculator from the dashboard."
                        ))
                        .action(
                            Button::new("section-missing-back", "Back to dashboard")
                                .icon(IconName::ArrowLeft)
                                .on_click(move |_event, window, cx| {
                                    back.update(cx, |state, cx| {
                                        state.back_to_dashboard(window, cx);
                                    });
                                }),
                        ),
                ),
        )
        .into_any_element()
}
