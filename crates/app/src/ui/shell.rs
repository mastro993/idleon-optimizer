//! Window chrome: the header, the sidebar and the routed place.
//!
//! The shell owns the window, the header, the sidebar and the routing
//! shortcuts; a section owns its own tab strip and content behind the registry
//! factory.

use crate::session::SidebarFold;
use crate::state::{AppState, Route};
use crate::ui::layout::SidebarWidth;
use crate::ui::{dashboard, export, header, import, section, sidebar};
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, KeyBinding, ParentElement, Role, Styled, Window, div, px,
};

gpui::actions!(idleon_shell, [BackToDashboard]);

/// Key context of a section page: it makes Back work from every section,
/// including the ones that own Escape themselves (the research grid binds it
/// deeper, so Escape there clears the selection before it leaves the page).
pub(crate) const SECTION_CONTEXT: &str = "Section";

/// Register the routing shortcuts.
pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(SECTION_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("escape", BackToDashboard, context),
        KeyBinding::new("secondary-left", BackToDashboard, context),
    ]);
}

/// Window chrome and the active view.
pub(crate) fn shell(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let (bg, fg) = {
        let colors = &cx.theme().colors;
        (colors.bg, colors.fg)
    };
    let route = state.route.clone();
    // Published before the place renders, so its breakpoint measures the
    // workspace the sidebar leaves.
    let rail = state.session.shell.sidebar == SidebarFold::Rail;
    let sidebar_width = cx.theme().sidebar_width(rail).to_pixels(window.rem_size());
    // Only on a change: setting a global notifies its observers every time.
    if cx.try_global::<SidebarWidth>().map(|width| width.0) != Some(sidebar_width) {
        cx.set_global(SidebarWidth(sidebar_width));
    }
    // Every section carries the same no-save state above its content; importing
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
        .child(header::header(state, cx))
        .child(
            div()
                .flex()
                .flex_1()
                .min_h_0()
                .child(sidebar::sidebar(state, cx))
                .child(
                    div()
                        .id("workspace")
                        .flex()
                        .flex_col()
                        .flex_1()
                        .min_w_0()
                        .min_h_0()
                        .children(no_save)
                        .child(div().flex_1().min_h_0().child(body)),
                ),
        )
        // No status bar is drawn; the last run or import message stays a
        // labelled node for a screen reader and the harnesses.
        .child(
            div()
                .id("status-status")
                .role(Role::Label)
                .aria_label(state.status.clone())
                .absolute()
                .size_0()
                .overflow_hidden(),
        );
    // The import dialog owns Escape while it is open, so the section's back
    // shortcut waits until it closes.
    if !route.is_dashboard() && !state.import.open && !state.export.open {
        root = root.key_context(SECTION_CONTEXT);
    }
    if state.import.open {
        root = root.child(import::dialog(state, cx));
    }
    if state.export.open {
        root = root.child(export::dialog(state, cx));
    }
    root.into_any_element()
}
