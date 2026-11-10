//! Window chrome: the dashboard, the routed section and the status bar.
//!
//! The shell owns the window, the shared header and the routing shortcuts; a
//! section owns its own tab strip and content behind the registry factory.

use crate::sections;
use crate::state::{AppState, Route};
use crate::ui::{dashboard, import, layout, section};
use ely_gpui_component::shell::{StatusBar, StatusBarItem};
use ely_gpui_component::theme::ActiveTheme as _;
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

/// Window chrome and the active view.
pub fn shell(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let (bg, fg) = {
        let colors = &cx.theme().colors;
        (colors.bg, colors.fg)
    };
    let route = state.route.clone();
    let header = match &route {
        Route::Dashboard => layout::dashboard_header(state, cx),
        Route::Section(_) => layout::page_header(state, cx),
        Route::Missing(id) => layout::missing_header(id, state, cx),
    };
    // Every section carries the same no-save state under its header; importing
    // replaces it with the save status the header already reflects.
    let no_save = match &route {
        Route::Section(_) if !state.save_loaded => Some(section::no_save(cx)),
        _ => None,
    };

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
    let status =
        status.right(StatusBarItem::new("status-assets").label(state.assets.status_label()));

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
        .children(no_save)
        .child(div().id("workspace").flex_1().min_h_0().child(body))
        // The status line is a labelled node of its own: a status bar item has no
        // aria label, and an agent or a screen reader reads the run's state here.
        .child(
            status.right(
                div()
                    .id("status-status")
                    .role(Role::Label)
                    .aria_label(state.status.clone())
                    .child(StatusBarItem::new("status-text").label(state.status.clone())),
            ),
        );
    // The import dialog owns Escape while it is open, so the section's back
    // shortcut waits until it closes.
    if !route.is_dashboard() && !state.import.open {
        root = root.key_context(SECTION_CONTEXT);
    }
    if state.import.open {
        root = root.child(import::dialog(state, cx));
    }
    root.into_any_element()
}
