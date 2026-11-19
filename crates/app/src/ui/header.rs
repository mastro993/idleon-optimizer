//! The header across the window: the sidebar toggle, the history arrows, the
//! save status and the shared Export and Import actions.

use crate::session::SidebarFold;
use crate::state::{AppState, History, Route, SaveStatus};
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Pixels, Role, Styled, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_ui::widgets::TEXT_CAPTION;

/// The header's height, which the shell lays its cached view out at.
pub(crate) const HEIGHT: Pixels = px(44.0);

/// The header for every place. Its left padding leaves room for the macOS
/// traffic lights in the transparent title bar.
pub(crate) fn header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let (border, muted) = (cx.theme().border, cx.theme().muted_foreground);
    let entity = cx.entity();
    let toggle = entity.clone();
    let toggle_label = if state.session.shell.sidebar == SidebarFold::Rail {
        "Expand sidebar"
    } else {
        "Collapse sidebar"
    };
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
            Button::new("sidebar-toggle")
                .icon(IconName::PanelLeft)
                .ghost()
                .small()
                .accessibility_label(toggle_label)
                .tooltip(toggle_label)
                .on_click(move |_event, _window, cx| {
                    toggle.update(cx, |state, cx| state.toggle_sidebar(cx));
                }),
        )
        .child(history(&state.history, entity.clone()))
        .child(div().flex_1())
        .child(save_actions(state.save_status(), muted, entity))
        .into_any_element()
}

/// What the history names a place, as the sidebar names it.
fn place_label(place: &Route) -> String {
    match place {
        Route::Dashboard => "Home".to_string(),
        Route::Section(section) => section.title.to_string(),
        Route::Missing(id) => id.clone(),
    }
}

/// Back, forward and the list of every place, newest first. An arrow is
/// disabled at its end; the list button reports the current place as its value.
fn history(history: &History, state: Entity<AppState>) -> impl IntoElement {
    let (current, last) = (history.position(), history.places().len() - 1);
    let labels: Vec<String> = history.places().iter().map(place_label).collect();
    let (back, forward) = (state.clone(), state.clone());
    let arrow = |id: &'static str, icon: IconName, label: &'static str| {
        Button::new(id)
            .icon(icon)
            .ghost()
            .small()
            .accessibility_label(label)
            .tooltip(label)
    };
    div()
        .flex()
        .items_center()
        .gap_0p5()
        .child(
            arrow("history-back", IconName::ArrowLeft, "Back")
                .disabled(current == 0)
                .on_click(move |_event, window, cx| {
                    back.update(cx, |state, cx| state.go_to(current - 1, window, cx));
                }),
        )
        .child(
            arrow("history-forward", IconName::ArrowRight, "Forward")
                .disabled(current >= last)
                .on_click(move |_event, window, cx| {
                    forward.update(cx, |state, cx| state.go_to(current + 1, window, cx));
                }),
        )
        .child(
            // The kit button cannot carry a value, so a wrapper reports the
            // current place under the list's id.
            div()
                .id("history-history-button")
                .role(Role::Group)
                .aria_label("History")
                .aria_value(labels[current].clone())
                .child(
                    arrow("history-list", IconName::ListClock, "History").dropdown_menu(
                        move |mut menu, _window, _cx| {
                            for (index, label) in labels.iter().enumerate().rev() {
                                let go = state.clone();
                                menu = menu.item(
                                    PopupMenuItem::new(label.clone())
                                        .checked(index == current)
                                        .on_click(move |_event, window, cx| {
                                            go.update(cx, |state, cx| {
                                                state.go_to(index, window, cx)
                                            });
                                        }),
                                );
                            }
                            menu
                        },
                    ),
                ),
        )
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
                .text_size(TEXT_CAPTION)
                .text_color(muted)
                .child(save.label()),
        )
        .child(
            Button::new("header-export")
                .label("Export")
                .icon(IconName::Download)
                .small()
                .on_click(move |_event, window, cx| {
                    export.update(cx, |state, cx| state.open_export(window, cx));
                }),
        )
        .child(
            Button::new("header-import")
                .label("Import")
                .icon(IconName::Upload)
                .small()
                .on_click(move |_event, window, cx| {
                    state.update(cx, |state, cx| state.open_import(window, cx));
                }),
        )
}
