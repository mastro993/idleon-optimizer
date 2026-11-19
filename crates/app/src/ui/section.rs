//! The section page frame and the shared section states: no save imported yet,
//! and ids the registry lacks.
//!
//! The missing state renders an empty state with a way back. It cannot panic:
//! the router resolves every id against the registry before the shell picks a
//! renderer.

use crate::state::{AppState, Route};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, ElementId, IntoElement, ParentElement, Pixels, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::Button;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::{ActiveTheme as _, Icon};
use idleon_ui::widgets::{Card, CardHeader, Tone};

/// The Console page gutter (`docs/ui-kit/design.md` § Density and shape):
/// the page frame owns it, so a section never sets its own.
pub(crate) const GUTTER_X: Pixels = px(20.0);
const GUTTER_Y: Pixels = px(16.0);

/// The page every place in the workspace renders into, inside the gutter.
pub(crate) fn page(id: impl Into<ElementId>, content: impl IntoElement) -> AnyElement {
    div()
        .id(id)
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .px(GUTTER_X)
        .py(GUTTER_Y)
        .child(content)
        .into_any_element()
}

/// The no-save state every section shows until a decoded save is imported.
///
/// It names what the fallback means and offers the shared import action, so a
/// section that needs account data is never a blank page with no way forward.
pub(crate) fn no_save(cx: &mut Context<AppState>) -> AnyElement {
    let import = cx.entity();
    div()
        .flex()
        .flex_none()
        .items_center()
        .gap_2()
        .px(GUTTER_X)
        .pt(GUTTER_Y)
        .child(
            div().flex_1().min_w_0().child(
                Alert::new(
                    "no-save-state",
                    "No save imported — this section uses defaults and supplements until you import a decoded save.",
                )
                .with_variant(Tone::Info.into())
                .banner(),
            ),
        )
        .child(
            Button::new("no-save-import")
                .label("Import save")
                .icon(IconName::Upload)
                .small()
                .on_click(move |_event, window, cx| {
                    import.update(cx, |state, cx| state.open_import(window, cx));
                }),
        )
        .into_any_element()
}

/// The route's id is not in the registry: say so, name the id, and offer the way back.
pub(crate) fn missing(id: &str, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let back = cx.entity();
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(CardHeader::new("Unknown section").description(id.to_string()))
        .child(
            div().id("section-missing").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new()
                                    .child(Icon::new(IconName::SearchX).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("No such section"))
                            .description(EmptyDescription::new().child(format!(
                                "The registry does not declare “{id}”. Check the id, or open a calculator from the dashboard."
                            ))),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("section-missing-back")
                                .label("Back to dashboard")
                                .icon(IconName::ArrowLeft)
                                .on_click(move |_event, window, cx| {
                                    back.update(cx, |state, cx| {
                                        state.open_place(Route::Dashboard, window, cx);
                                    });
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}
