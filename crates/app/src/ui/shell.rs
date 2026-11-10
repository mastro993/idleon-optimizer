//! Window chrome: the dashboard, the active section and the status bar.

use crate::sections::SECTIONS;
use crate::state::{AppState, Tab};
use crate::ui::{dashboard, format, section};
use ely_gpui_component::buttons::IconButton;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{StatusBar, StatusBarItem, TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Heading;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, px};

/// Window chrome and the active view.
pub fn shell(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let (bg, fg, border, accent, muted) = {
        let colors = &cx.theme().colors;
        (
            colors.bg,
            colors.fg,
            colors.border,
            colors.accent,
            colors.fg_muted,
        )
    };
    let section = state.current_section();
    let entity = cx.entity();
    let rate = state.rate();

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

    match section {
        None => {
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
                        .aria_label(format!("{} calculators", SECTIONS.len()))
                        .text_size(px(12.0))
                        .text_color(muted)
                        .child(format!("{} calculators", SECTIONS.len())),
                );
        }
        Some(section) => {
            let back = entity.clone();
            header = header
                .child(
                    IconButton::new("back-to-dashboard", IconName::ArrowLeft)
                        .tooltip("Back to dashboard")
                        .on_click(move |_event, _window, cx| {
                            back.update(cx, |state, cx| state.back_to_dashboard(cx));
                        }),
                )
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
            if section.id == "research" {
                header = header.child(tabs(state, cx));
            }
            header = header.child(
                div()
                    .id("header-rate")
                    .role(Role::Label)
                    .aria_label(format!("Research EXP/hr {}", format::rate(rate.total)))
                    .text_size(px(12.0))
                    .text_color(accent)
                    .child(format!("{} /hr", format::rate(rate.total))),
            );
        }
    }

    let body = match section.map(|section| section.id) {
        None => div()
            .id("pane-dashboard")
            .size_full()
            .p_6()
            .min_h_0()
            .child(dashboard::dashboard_pane(state, window, cx))
            .into_any_element(),
        Some("research") => div()
            .id("pane-section")
            .size_full()
            .p_3()
            .min_h_0()
            .child(section::research_section(state, window, cx))
            .into_any_element(),
        Some(_) => div()
            .id("pane-section")
            .size_full()
            .p_3()
            .min_h_0()
            .child(section::placeholder_section(state, window, cx))
            .into_any_element(),
    };

    let status = match section {
        None => StatusBar::new()
            .left(
                StatusBarItem::new("status-view").label(format!("{} calculators", SECTIONS.len())),
            )
            .left(
                StatusBarItem::new("status-detail")
                    .label("Search by title, description or world; Escape clears the search."),
            ),
        Some(section) if section.id == "research" => StatusBar::new()
            .left(StatusBarItem::new("status-rate").label(format!(
                "{} EXP/hr · ×{:.2}",
                format::rate(rate.total),
                rate.multi
            )))
            .left(StatusBarItem::new("status-points").label(format!(
                "{} / {} grid points",
                format::count(state.grid_points().spent as u64),
                format::count(state.grid_points().earned as u64)
            ))),
        Some(section) => StatusBar::new()
            .left(StatusBarItem::new("status-view").label(format!(
                "{} · {}",
                section.title,
                section.badge()
            )))
            .left(StatusBarItem::new("status-detail").label("Section planned")),
    };

    div()
        .id("app-root")
        .flex()
        .flex_col()
        .size_full()
        .bg(bg)
        .text_color(fg)
        .text_size(px(13.0))
        .child(header)
        .child(div().id("workspace").flex_1().min_h_0().child(body))
        .child(status.right(StatusBarItem::new("status-status").label(state.status.clone())))
}

/// The research section's upstream tab strip.
fn tabs(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("app-tabs").selected(SharedString::from(state.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    let picker = entity.clone();
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            picker.update(cx, |state, cx| state.set_tab(tab, cx));
        }
    })
}
