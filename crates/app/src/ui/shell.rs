//! Window chrome: the header, the sidebar and the routed place.
//!
//! The shell owns the window, the header, the sidebar and the history
//! shortcuts; a section owns its own tab strip and content behind the registry
//! factory.

use crate::session::SidebarFold;
use crate::state::{AppState, Route};
use crate::ui::layout::SidebarWidth;
use crate::ui::{dashboard, export, header, import, section, sidebar};
use ely_gpui_component::motion::LoadingOverlay;
use ely_gpui_component::theme::ActiveTheme as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, KeyBinding, ParentElement, Role, Styled, Window, div, px,
};

gpui::actions!(idleon_shell, [GoBack, GoForward]);

/// Key context of the window root while no dialog is open: the history
/// shortcuts work from every place, Home included, and wait while a dialog
/// owns the keyboard.
const SHELL_CONTEXT: &str = "Shell";

/// Register the history shortcuts.
pub(crate) fn bind_keys(cx: &mut App) {
    let context = Some(SHELL_CONTEXT);
    cx.bind_keys([
        KeyBinding::new("secondary-[", GoBack, context),
        KeyBinding::new("secondary-]", GoForward, context),
        KeyBinding::new("secondary-left", GoBack, context),
        KeyBinding::new("secondary-right", GoForward, context),
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
    let route = state.route().clone();
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
        .on_action(cx.listener(|state, _: &GoBack, window, cx| {
            state.go_back(window, cx);
            cx.stop_propagation();
        }))
        .on_action(cx.listener(|state, _: &GoForward, window, cx| {
            state.go_forward(window, cx);
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
    if !state.import.open && !state.export.open {
        root = root.key_context(SHELL_CONTEXT);
    }
    if state.import.open {
        root = root.child(import::dialog(state, cx));
    }
    if state.export.open {
        root = root.child(export::dialog(state, cx));
    }
    // The window opens before the session file is read; a veil holds the place
    // the restore is about to fill instead of showing the empty dashboard.
    if state.restoring {
        return LoadingOverlay::new("restoring", true)
            .label("Loading your save…")
            .size_full()
            .child(root)
            .into_any_element();
    }
    root.into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::headless;

    #[test]
    fn both_shortcut_pairs_step_through_places_and_escape_stays_on_the_section() {
        let dir = std::env::temp_dir().join(format!("idleon-history-keys-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut cx = gpui::TestAppContext::single();
        let window = headless(&mut cx, &dir);
        cx.update(bind_keys);
        let farming = Route::open("farming");
        let open = farming.clone();
        window
            .update(&mut cx, |state, window, cx| {
                state.open_place(open, window, cx)
            })
            .expect("the window is open");
        let route = |cx: &mut gpui::TestAppContext| {
            window
                .update(cx, |state, _, _| state.route().clone())
                .expect("the window is open")
        };

        cx.simulate_keystrokes(window.into(), "escape");
        assert_eq!(route(&mut cx), farming, "Escape no longer leaves a section");
        for (back, forward) in [
            ("secondary-[", "secondary-]"),
            ("secondary-left", "secondary-right"),
        ] {
            cx.simulate_keystrokes(window.into(), back);
            assert_eq!(route(&mut cx), Route::Dashboard, "{back} goes back");
            cx.simulate_keystrokes(window.into(), forward);
            assert_eq!(route(&mut cx), farming, "{forward} goes forward from Home");
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
