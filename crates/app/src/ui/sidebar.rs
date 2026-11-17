//! The sidebar: Home, then one group of sections per world, then the Tools.
//!
//! Ely's `NavItem` takes a vector icon only, so the items are the app's own
//! [`SidebarItem`] with `NavItem`'s look (ADR 0003).

use crate::sections::{self, Group, Section};
use crate::session::SidebarFold;
use crate::state::{AppState, Route};
use ely_gpui_component::forms::Run;
use ely_gpui_component::layout::{Sidebar, bring_into_view};
use ely_gpui_component::navigation::NavGroup;
use ely_gpui_component::primitives::{FocusRing as _, Icon, IconName, Tooltip};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, IconSize, Radius, TextSize};
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Axis, Bounds, Context, FontWeight, Hsla, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString, Styled, Window, canvas,
    div, fill, linear_color_stop, linear_gradient, point, px, size,
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
    let ground = cx.theme().colors.bg;
    let entity = cx.entity();
    let scroll = &state.sidebar_scroll;

    let home = entity.clone();
    let mut list = div().flex().flex_col().gap_3().p_2().child(
        SidebarItem::new("nav-home", "Home", glyph(IconName::House))
            .active(state.route().is_dashboard())
            .folded(rail)
            .reveal(scroll)
            .on_click(move |window, cx| {
                home.update(cx, |state, cx| {
                    state.open_place(Route::Dashboard, window, cx)
                });
            }),
    );
    for (group, sections) in groups() {
        // Folded, NavGroup renders its items alone; the heading is drawn here
        // because NavGroup's title always carries a fold chevron, and these
        // headings do not fold.
        let mut nav =
            NavGroup::new(format!("nav-group-{}", group.id()), group.label(), true).folded(true);
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
                    open.update(cx, |state, cx| {
                        state.open_place(Route::Section(section), window, cx);
                    });
                }),
            );
        }
        list = list.child(
            div()
                .flex()
                .flex_col()
                .gap_0p5()
                .child(heading(group, rail, cx))
                .child(nav),
        );
    }

    // A fade at each edge says there is more to scroll; a scrollbar would sit
    // on the icons, the rail's narrow column most of all.
    Sidebar::new("app-sidebar", rail)
        .child(
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .child(
                    div()
                        .id("app-sidebar-scroll")
                        .size_full()
                        .overflow_y_scroll()
                        .track_scroll(scroll)
                        .child(list),
                )
                .child(edge_fades(scroll, ground)),
        )
        .into_any_element()
}

/// A fade into the sidebar's ground at each edge with more items beyond it.
///
/// Ely's `ScrollShadow` casts its shadows downward, so its bottom edge is
/// clipped away, and it reads the scroll offsets during render, before the
/// frame's layout. This paints after the list is laid out, so a fold or a
/// scroll always shows the current edges.
fn edge_fades(scroll: &ScrollHandle, ground: Hsla) -> impl IntoElement {
    let scroll = scroll.clone();
    canvas(
        |_, _, _| {},
        move |bounds, (), window, _| {
            let scrolled = -scroll.offset().y;
            let reach = scroll.max_offset().y;
            let height = px(36.0).min(bounds.size.height);
            let fade = |top: Pixels, angle: f32| {
                fill(
                    Bounds::new(point(bounds.origin.x, top), size(bounds.size.width, height)),
                    linear_gradient(
                        angle,
                        linear_color_stop(ground.alpha(0.0), 0.0),
                        linear_color_stop(ground, 1.0),
                    ),
                )
            };
            if scrolled > Pixels::ZERO {
                window.paint_quad(fade(bounds.top(), 0.0));
            }
            if scrolled < reach {
                window.paint_quad(fade(bounds.bottom() - height, 180.0));
            }
        },
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
}

/// A group's title in `NavGroup`'s type, without the fold chevron; the rail
/// shows its short form at the same left edge.
fn heading(group: Group, rail: bool, cx: &App) -> impl IntoElement {
    let theme = cx.theme();
    div()
        .id(SharedString::from(format!(
            "nav-group-title-{}",
            group.id()
        )))
        .role(Role::Heading)
        .aria_label(group.label())
        .flex()
        .items_center()
        .h(theme.control_height(ControlSize::Sm))
        .border_1()
        .border_color(gpui::transparent_black())
        .text_size(theme.text_size(TextSize::Xs))
        .font_weight(FontWeight::MEDIUM)
        .text_color(theme.colors.fg_subtle)
        .px_2()
        .whitespace_nowrap()
        .child(if rail {
            rail_label(group)
        } else {
            group.label()
        })
}

/// A group's heading in the rail, short enough for its 56 px.
fn rail_label(group: Group) -> &'static str {
    match group {
        Group::General => "Gen",
        Group::Tools => "Tools",
        world => world.badge(),
    }
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
        // One layout in both states, so only the sidebar's width animates: the
        // icon keeps its place and the rail hides the label instead of
        // dropping it, which would reflow the row mid-animation.
        item.px_2()
            .overflow_hidden()
            .when(self.folded, |item| {
                item.tooltip(Tooltip::text(label.clone()))
            })
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis()
                    .when(self.folded, |label| label.invisible())
                    .child(label),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rail_shortens_every_group_heading() {
        let labels: Vec<&str> = Group::ALL.into_iter().map(rail_label).collect();
        assert_eq!(labels, ["Gen", "W2", "W3", "W4", "W5", "W6", "W7", "Tools"]);
    }

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
