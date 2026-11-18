//! The header across the window: the sidebar toggle, the history arrows, the
//! save status and the shared Export and Import actions.

use crate::session::SidebarFold;
use crate::state::{AppState, History, Route, SaveStatus};
use ely_gpui_component::buttons::{Button, ButtonVariant, IconButton};
use ely_gpui_component::forms::Choice;
use ely_gpui_component::navigation::BackForwardNavigation;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Pixels, Role, Styled, div, px,
};

/// The header's height, which the shell lays its cached view out at.
pub(crate) const HEIGHT: Pixels = px(48.0);

/// The header for every place. Its left padding leaves room for the macOS
/// traffic lights in the transparent title bar.
pub(crate) fn header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let (border, muted) = {
        let colors = &cx.theme().colors;
        (colors.border, colors.fg_muted)
    };
    let entity = cx.entity();
    let toggle = entity.clone();
    let go = entity.clone();
    let rail = state.session.shell.sidebar == SidebarFold::Rail;
    div()
        .id("app-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .pl(px(84.0))
        .pr_4()
        .h(HEIGHT)
        .border_b_1()
        .border_color(border)
        .child(
            IconButton::new("sidebar-toggle", IconName::PanelLeft)
                .variant(ButtonVariant::Ghost)
                .size(ControlSize::Sm)
                .tooltip(if rail {
                    "Expand sidebar"
                } else {
                    "Collapse sidebar"
                })
                .on_click(move |_event, _window, cx| {
                    toggle.update(cx, |state, cx| state.toggle_sidebar(cx));
                }),
        )
        .child(history(&state.history).on_go(move |index, window, cx| {
            go.update(cx, |state, cx| state.go_to(index, window, cx));
        }))
        .child(div().flex_1())
        .child(save_actions(state.save_status(), muted, entity))
        .into_any_element()
}

/// Back, forward and the list of every place, named as the sidebar names them.
fn history(history: &History) -> BackForwardNavigation {
    let places = history.places().iter().enumerate().map(|(index, place)| {
        let label = match place {
            Route::Dashboard => "Home".to_string(),
            Route::Section(section) => section.title.to_string(),
            Route::Missing(id) => id.clone(),
        };
        Choice::new(index.to_string(), label)
    });
    BackForwardNavigation::new("history", places, history.position())
}

/// The save status and the Export and Import actions.
fn save_actions(save: SaveStatus, muted: Hsla, state: Entity<AppState>) -> impl IntoElement {
    let export = state.clone();
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .id("header-save")
                .role(Role::Label)
                .aria_label(save.label())
                .text_size(px(12.0))
                .text_color(muted)
                .child(save.label()),
        )
        .child(
            Button::new("header-export", "Export")
                .size(ControlSize::Sm)
                .icon(IconName::Download)
                .on_click(move |_event, window, cx| {
                    export.update(cx, |state, cx| state.open_export(window, cx));
                }),
        )
        .child(
            Button::new("header-import", "Import")
                .size(ControlSize::Sm)
                .icon(IconName::Upload)
                .on_click(move |_event, window, cx| {
                    state.update(cx, |state, cx| state.open_import(window, cx));
                }),
        )
}
