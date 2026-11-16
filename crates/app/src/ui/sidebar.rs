//! The sidebar: Home, then one group of sections per world, then the Tools.
//!
//! Ely's `NavItem` takes a vector icon only, so the items are the app's own
//! [`SidebarItem`] with `NavItem`'s look (ADR 0003).

use crate::sections::{self, Group, Section};
use crate::session::SidebarFold;
use crate::state::AppState;
use ely_gpui_component::forms::Run;
use ely_gpui_component::layout::{ScrollArea, Sidebar, bring_into_view};
use ely_gpui_component::navigation::NavGroup;
use ely_gpui_component::primitives::{FocusRing as _, Icon, IconName, Tooltip};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, IconSize, Radius, TextSize};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Axis, Context, FontWeight, IntoElement, MouseButton, ParentElement,
    RenderOnce, Role, ScrollHandle, SharedString, Styled, Window, canvas, div, px,
};
use std::rc::Rc;

/// The sidebar's groups in order, each with its sections. Tools lists every
/// section without a dashboard card, whatever world it belongs to.
pub(crate) fn groups() -> Vec<(Group, Vec<&'static Section>)> {
    Group::ALL
        .into_iter()
        .map(|group| {
            let sections = sections::SECTIONS
                .iter()
                .filter(|section| match group {
                    Group::Tools => !section.dashboard,
                    _ => section.dashboard && section.group == group,
                })
                .collect();
            (group, sections)
        })
        .collect()
}

/// The sidebar column: Home, then every group, scrolling on its own.
pub(crate) fn sidebar(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let rail = state.session.shell.sidebar == SidebarFold::Rail;
    let border = cx.theme().colors.border;
    let entity = cx.entity();
    let scroll = &state.sidebar_scroll;

    let home = entity.clone();
    let mut list = div()
        .flex()
        .flex_col()
        .gap_3()
        .p_2()
        .when(rail, |list| list.items_center())
        .child(
            SidebarItem::new("nav-home", "Home", glyph(IconName::House))
                .active(state.route.is_dashboard())
                .folded(rail)
                .reveal(scroll)
                .on_click(move |window, cx| {
                    home.update(cx, |state, cx| state.back_to_dashboard(window, cx));
                }),
        );
    for (group, sections) in groups() {
        if rail {
            list = list.child(div().self_stretch().mx_1().h(px(1.0)).bg(border));
        }
        let mut nav =
            NavGroup::new(format!("nav-group-{}", group.id()), group.label(), true).folded(rail);
        for section in sections {
            let open = entity.clone();
            nav = nav.child(
                SidebarItem::new(
                    format!("nav-{}", section.id),
                    section.title,
                    emoji(section.icon),
                )
                .active(state.current_section() == Some(section))
                .folded(rail)
                .reveal(scroll)
                .on_click(move |window, cx| {
                    open.update(cx, |state, cx| state.open_section(section.id, window, cx));
                }),
            );
        }
        list = list.child(nav);
    }

    Sidebar::new("app-sidebar", rail)
        .child(
            ScrollArea::new("app-sidebar-scroll")
                .vertical()
                .track_scroll(scroll)
                .flex_1()
                .min_h_0()
                .child(list),
        )
        .into_any_element()
}

/// The 20 px icon slot every item shares.
fn slot() -> gpui::Div {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(20.0))
}

/// A vector icon in the slot.
fn glyph(icon: IconName) -> AnyElement {
    slot()
        .child(Icon::new(icon).size(IconSize::Sm))
        .into_any_element()
}

/// A section's emoji in the slot.
fn emoji(icon: &'static str) -> AnyElement {
    slot().text_size(px(15.0)).child(icon).into_any_element()
}

/// One place in the sidebar: `NavItem`'s height, radius, hover, active fill
/// and focus ring, with any element as its icon.
#[derive(IntoElement)]
pub(crate) struct SidebarItem {
    id: SharedString,
    label: SharedString,
    icon: AnyElement,
    active: bool,
    folded: bool,
    reveal: Option<ScrollHandle>,
    on_click: Option<Run>,
}

impl SidebarItem {
    pub(crate) fn new(
        id: impl Into<SharedString>,
        label: impl Into<SharedString>,
        icon: AnyElement,
    ) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            icon,
            active: false,
            folded: false,
            reveal: None,
            on_click: None,
        }
    }

    /// Marks the open place; clicking it again does nothing.
    pub(crate) fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The icon alone with the title in a tooltip, for the rail.
    pub(crate) fn folded(mut self, folded: bool) -> Self {
        self.folded = folded;
        self
    }

    /// Scrolls `scroll` to the item once each time it becomes active.
    pub(crate) fn reveal(mut self, scroll: &ScrollHandle) -> Self {
        self.reveal = Some(scroll.clone());
        self
    }

    pub(crate) fn on_click(mut self, handler: impl Fn(&mut Window, &mut App) + 'static) -> Self {
        self.on_click = Some(Rc::new(handler));
        self
    }
}

impl RenderOnce for SidebarItem {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let reveal = match (self.active, self.reveal) {
            (true, Some(scroll)) => {
                let shown = window.use_keyed_state(
                    SharedString::from(format!("{}-revealed", self.id)),
                    cx,
                    |_, _| false,
                );
                Some(canvas(
                    move |bounds, window, cx| {
                        if !*shown.read(cx) {
                            shown.update(cx, |shown, _| *shown = true);
                            if bring_into_view(&scroll, bounds, Axis::Vertical) {
                                window.request_animation_frame();
                            }
                        }
                    },
                    |_, _, _, _| {},
                ))
            }
            _ => None,
        };
        let theme = cx.theme();
        let colors = &theme.colors;
        let fg = if self.active {
            colors.fg
        } else {
            colors.fg_muted
        };
        let label = self.label.clone();
        let item = div()
            .id(self.id)
            .role(Role::Link)
            .aria_label(label.clone())
            .aria_selected(self.active)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(theme.control_height(ControlSize::Md))
            .rounded(theme.radius(Radius::Md))
            .border_1()
            .border_color(gpui::transparent_black())
            .text_size(theme.text_size(TextSize::Sm))
            .text_color(fg)
            .cursor_pointer()
            .tab_index(0)
            .focus_ring(cx)
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .map(|item| {
                if self.active {
                    item.bg(colors.hover).font_weight(FontWeight::MEDIUM)
                } else {
                    item.hover(|style| style.bg(colors.hover).text_color(colors.fg))
                }
            })
            .when_some(self.on_click.filter(|_| !self.active), |item, click| {
                item.on_click(move |_, window, cx| click(window, cx))
            })
            .child(self.icon)
            .when_some(reveal, |item, reveal| {
                item.child(reveal.absolute().top_0().left_0().size_full())
            });
        if self.folded {
            return item
                .justify_center()
                .w(theme.control_height(ControlSize::Md))
                .tooltip(Tooltip::text(label));
        }
        item.px_2().child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_hidden()
                .whitespace_nowrap()
                .text_ellipsis()
                .child(label),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_section_sits_once_under_its_world_and_the_hidden_ones_under_tools() {
        let ids: Vec<(Group, Vec<&str>)> = groups()
            .into_iter()
            .map(|(group, sections)| (group, sections.iter().map(|section| section.id).collect()))
            .collect();
        assert_eq!(
            ids,
            [
                (Group::General, vec!["combat-sim", "statue"]),
                (Group::W2, vec!["fishing", "prisma", "killroy"]),
                (Group::W3, vec!["cog", "worship-td"]),
                (Group::W4, vec!["ladle", "cooking-mastery"]),
                (
                    Group::W5,
                    vec![
                        "fountain",
                        "jar",
                        "sailing",
                        "cglunko",
                        "coral-god-rank",
                        "monument",
                        "opal"
                    ]
                ),
                (Group::W6, vec!["sneaking", "farming"]),
                (
                    Group::W7,
                    vec![
                        "clamworks",
                        "royal-guardian",
                        "spelunking",
                        "jelly-operator",
                        "research"
                    ]
                ),
                (
                    Group::Tools,
                    vec!["stats-all", "damage", "drop-rate", "coin-multi", "pickle"]
                ),
            ]
        );
    }
}
