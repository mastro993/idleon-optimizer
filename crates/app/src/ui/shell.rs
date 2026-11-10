//! Window chrome: title, tab strip, the active pane and the status bar.

use crate::state::{AppState, Tab};
use crate::ui::{dashboard, format, grid_view, inspector, optimizer};
use ely_gpui_component::shell::{StatusBar, StatusBarItem, TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::{Heading, Label};
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px};

/// Window chrome and the active pane.
pub fn shell(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let entity = cx.entity();

    let mut tabs = TabBar::new("app-tabs").selected(SharedString::from(state.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    let picker = entity.clone();
    let tabs = tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            picker.update(cx, |state, cx| state.set_tab(tab, cx));
        }
    });

    let rate = state.rate();
    let points = state.grid_points();

    div()
        .id("app-root")
        .flex()
        .flex_col()
        .size_full()
        .bg(colors.bg)
        .text_color(colors.fg)
        .text_size(px(13.0))
        .child(
            div()
                .id("app-header")
                .flex()
                .flex_none()
                .items_center()
                .gap_4()
                // Leaves room for the macOS traffic lights in the transparent title bar.
                .pl(px(84.0))
                .pr_4()
                .h(px(48.0))
                .border_b_1()
                .border_color(colors.border)
                .child(Heading::new(3, "Research Optimizer"))
                .child(tabs)
                .child(div().flex_1())
                .child(
                    Label::new(format!("{} /hr", format::rate(rate.total)))
                        .text_color(colors.accent),
                ),
        )
        .child(div().id("workspace").flex_1().min_h_0().child(pane(state, window, cx)))
        .child(
            StatusBar::new()
                .left(StatusBarItem::new("status-rate").label(format!(
                    "{} EXP/hr · ×{:.2}",
                    format::rate(rate.total),
                    rate.multi
                )))
                .left(StatusBarItem::new("status-points").label(format!(
                    "{} / {} grid points",
                    format::count(points.spent as u64),
                    format::count(points.earned as u64)
                )))
                .right(StatusBarItem::new("status-status").label(state.status.clone())),
        )
}

fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    match state.tab {
        Tab::Grid => div()
            .id("pane-grid")
            .flex()
            .size_full()
            .gap_3()
            .p_3()
            .min_h_0()
            .child(
                div()
                    .id("grid-column")
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(grid_view::grid_pane(state, window, cx)),
            )
            .child(
                div()
                    .id("inspector-column")
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(px(360.0))
                    .min_h_0()
                    .child(inspector::inspector_pane(state, window, cx)),
            )
            .into_any_element(),
        Tab::Rates => div()
            .id("pane-rates")
            .size_full()
            .p_3()
            .min_h_0()
            .child(dashboard::dashboard_pane(state, window, cx))
            .into_any_element(),
        Tab::Run => div()
            .id("pane-run")
            .size_full()
            .p_3()
            .min_h_0()
            .child(optimizer::optimizer_pane(state, window, cx))
            .into_any_element(),
    }
}
