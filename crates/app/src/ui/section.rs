//! Shared section states: ids the registry lacks.
//!
//! The state renders an empty state with a way back. It cannot panic: the router
//! resolves every id against the registry before the shell picks a renderer.

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::{Banner, EmptyState};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};

/// The no-save state every section shows until a decoded save is imported.
///
/// It names what the fallback means and offers the shared import action, so a
/// section that needs account data is never a blank page with no way forward.
pub(crate) fn no_save(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    Banner::new(
        "no-save-state",
        Severity::Info,
        "No save imported — this section uses defaults and supplements until you import a decoded save.",
    )
    .action("Import save", move |window, cx| {
        import.update(cx, |state, cx| state.open_import(window, cx));
    })
    .into_any_element()
}

/// The route's id is not in the registry: say so, name the id, and offer the way back.
pub(crate) fn missing(id: &str, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
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
