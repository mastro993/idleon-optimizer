//! Window chrome: the dashboard, the routed section and the status bar.
//!
//! The shell owns the window, the shared header and the routing shortcuts; a
//! section owns its own tab strip and content behind the registry factory.

use crate::sections;
use crate::state::{AppState, Route};
use crate::ui::{dashboard, section};
use ely_gpui_component::buttons::IconButton;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{StatusBar, StatusBarItem};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Heading;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, KeyBinding, ParentElement, Role, Styled, Window, div, px,
};

gpui::actions!(idleon_shell, [BackToDashboard]);

/// Key context of a section page: it makes Back work from every section,
/// including the ones that own Escape themselves (the research grid binds it
/// deeper, so Escape there clears the selection before it leaves the page).
pub const SECTION_CONTEXT: &str = "Section";

/// Register the routing shortcuts.
pub fn bind_keys(cx: &mut App) {
    let context = Some(SECTION_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", BackToDashboard, context),
        KeyBinding::new("secondary-left", BackToDashboard, context),
    ]);
}

/// The shared header's way back to the dashboard.
fn back_button(entity: &gpui::Entity<AppState>) -> IconButton {
    let back = entity.clone();
    IconButton::new("back-to-dashboard", IconName::ArrowLeft)
        .tooltip("Back to dashboard")
        .on_click(move |_event, window, cx| {
            back.update(cx, |state, cx| state.back_to_dashboard(window, cx));
        })
}

/// Window chrome and the active view.
pub fn shell(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let (bg, fg, border, muted) = {
        let colors = &cx.theme().colors;
        (colors.bg, colors.fg, colors.border, colors.fg_muted)
    };
    let route = state.route.clone();
    let entity = cx.entity();

    let mut header = div()
        .id("app-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        // Leaves room for the macOS traffic lights in the transparent title bar.
        .pl(px(84.0))
        .pr_4()
        .h(px(48.0))
        .border_b_1()
        .border_color(border);

    match &route {
        Route::Dashboard => {
            header = header
                .child(Heading::new(3, "Idleon Optimizer"))
                .child(
                    div()
                        .id("header-subtitle")
                        .role(Role::Label)
                        .aria_label("Dashboard")
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child("Dashboard"),
                )
                .child(div().flex_1())
                .child(
                    div()
                        .id("header-cards")
                        .role(Role::Label)
                        .aria_label(format!("{} calculators", sections::card_count()))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(format!("{} calculators", sections::card_count())),
                );
        }
        Route::Section(section) => {
            header = header
                .child(back_button(&entity))
                .child(div().text_size(px(18.0)).child(section.icon))
                .child(
                    div()
                        .id("section-title")
                        .role(Role::Heading)
                        .aria_label(section.title)
                        .child(Heading::new(3, section.title)),
                )
                .child(Badge::new(section.badge()))
                .child(div().flex_1());
        }
        Route::Missing(_) => {
            header = header
                .child(back_button(&entity))
                .child(
                    div()
                        .id("section-title")
                        .role(Role::Heading)
                        .aria_label("Unknown section")
                        .child(Heading::new(3, "Unknown section")),
                )
                .child(div().flex_1());
        }
    }

    let body = match &route {
        Route::Dashboard => div()
            .id("pane-dashboard")
            .size_full()
            .p_6()
            .min_h_0()
            .child(dashboard::dashboard_pane(state, window, cx))
            .into_any_element(),
        Route::Section(section) => {
            let view = (section.view)(state, window, cx);
            div()
                .id(format!("section-{}", section.id))
                .size_full()
                .p_3()
                .min_h_0()
                .child(view)
                .into_any_element()
        }
        Route::Missing(id) => div()
            .id("section-unknown")
            .size_full()
            .p_3()
            .min_h_0()
            .child(section::missing(id, window, cx))
            .into_any_element(),
    };

    let status = match &route {
        Route::Dashboard => StatusBar::new()
            .left(
                StatusBarItem::new("status-view")
                    .label(format!("{} calculators", sections::card_count())),
            )
            .left(
                StatusBarItem::new("status-detail")
                    .label("Search by title, description or world; Escape clears the search."),
            ),
        Route::Section(section) => StatusBar::new()
            .left(StatusBarItem::new("status-view").label(format!(
                "{} · {}",
                section.title,
                section.badge()
            )))
            .left(StatusBarItem::new("status-detail").label(format!(
                "{} · {}",
                section.status.label(),
                section.spec
            ))),
        Route::Missing(id) => StatusBar::new()
            .left(StatusBarItem::new("status-view").label("Unknown section"))
            .left(
                StatusBarItem::new("status-detail").label(format!("{id} is not in the registry")),
            ),
    };

    let mut root = div()
        .id("app-root")
        .flex()
        .flex_col()
        .size_full()
        .bg(bg)
        .text_color(fg)
        .text_size(px(13.0))
        .track_focus(&state.focus)
        .on_action(cx.listener(|state, _: &BackToDashboard, window, cx| {
            state.back_to_dashboard(window, cx);
            cx.stop_propagation();
        }))
        .child(header)
        .child(div().id("workspace").flex_1().min_h_0().child(body))
        .child(status.right(StatusBarItem::new("status-status").label(state.status.clone())));
    if !route.is_dashboard() {
        root = root.key_context(SECTION_CONTEXT);
    }
    root.into_any_element()
}
