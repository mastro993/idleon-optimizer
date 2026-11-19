//! The sidebar: Home, then one group of sections per world, then the Tools.
//!
//! The kit's `SidebarMenuItem` takes a vector icon only, hides its suffix in
//! the rail and draws no keyboard focus, so the column and its items are the
//! app's own, drawn from the kit theme: each section's game sprite and its
//! activity mark (ADR 0005).

use crate::sections::{self, Group, Section};
use crate::session::SidebarFold;
use crate::state::{AppState, Route, Shown};
use crate::ui::assets::section_icon;
use crate::ui::mark::ActivityMark;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Axis, Bounds, Context, FontWeight, Hsla, IntoElement, MouseButton,
    ParentElement, Pixels, RenderOnce, Role, ScrollHandle, SharedString, Styled, Window, canvas,
    div, fill, linear_color_stop, linear_gradient, point, px, size,
};
use gpui_kit::assets::IconName;
use gpui_kit::base::{Transition, transition};
use gpui_kit::component::animation::ease_in_out_cubic;
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{ActiveTheme as _, Icon, ThemeStyled as _};
use idleon_ui::widgets::{TEXT_BODY, TEXT_CAPTION, bring_into_view};
use std::rc::Rc;
use std::time::Duration;

/// The sidebar's width expanded, with icon and title.
const WIDE: Pixels = px(228.0);
/// The sidebar's width folded to the rail of icons.
const RAIL: Pixels = px(56.0);
/// How long the sidebar animates its width when it folds or unfolds.
pub(crate) const FOLD_TIME: Duration = Duration::from_millis(200);
/// An item's height.
const ROW: Pixels = px(26.0);
/// A group heading's height.
const HEADING: Pixels = px(24.0);

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

/// The sidebar's width at `fold`, which the shell lays its cached view out at.
pub(crate) fn width(fold: SidebarFold) -> Pixels {
    match fold {
        SidebarFold::Expanded => WIDE,
        SidebarFold::Rail => RAIL,
    }
}

/// The sidebar column: Home, then every group, scrolling on its own.
pub(crate) fn sidebar(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let fold = state.session.shell.sidebar;
    let rail = fold == SidebarFold::Rail;
    let entity = cx.entity();
    let scroll = &state.sidebar_scroll;

    let home = entity.clone();
    let mut list = div().flex().flex_col().gap_3().p_2().child(
        SidebarItem::new(
            "nav-home",
            "Home",
            glyph(Icon::new(IconName::House).size_3p5()),
        )
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
        let mut items = div()
            .id(SharedString::from(format!("nav-group-{}", group.id())))
            .flex()
            .flex_col()
            .gap_0p5();
        for section in sections {
            let open = entity.clone();
            items = items.child(
                SidebarItem::new(
                    format!("nav-{}", section.id),
                    section.title,
                    section_icon(section, &state.assets, ICON),
                )
                .active(state.current_section() == Some(section))
                .activity(state.activity.shown(section.id))
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
                .child(items),
        );
    }

    // The width eases between the two folds; everything inside keeps one
    // layout, so only the column's edge moves.
    let ground = cx.theme().sidebar;
    let shown = transition(
        "app-sidebar-width",
        width(fold),
        Transition::new(FOLD_TIME).ease(ease_in_out_cubic),
        window,
        cx,
    );
    // A fade at each edge says there is more to scroll; a scrollbar would sit
    // on the icons, the rail's narrow column most of all.
    div()
        .id("app-sidebar")
        .flex()
        .flex_col()
        .flex_none()
        .h_full()
        .w(shown)
        .overflow_hidden()
        .border_r_1()
        .border_color(cx.theme().sidebar_border)
        .bg(ground)
        .text_color(cx.theme().sidebar_foreground)
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
/// It paints after the list is laid out, so a fold or a scroll always shows
/// the current edges.
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

/// A group's title in the subtle uppercase of a table head, which does not
/// fold; the rail shows its short form at the same left edge.
fn heading(group: Group, rail: bool, cx: &App) -> impl IntoElement {
    div()
        .id(SharedString::from(format!(
            "nav-group-title-{}",
            group.id()
        )))
        .role(Role::Heading)
        .aria_label(group.label())
        .flex()
        .items_center()
        .h(HEADING)
        .px_2()
        .text_size(TEXT_CAPTION)
        .font_weight(FontWeight::MEDIUM)
        .text_color(cx.theme().table_head_foreground)
        .whitespace_nowrap()
        .child(
            if rail {
                rail_label(group)
            } else {
                group.label()
            }
            .to_uppercase(),
        )
}

/// A group's heading in the rail, short enough for its 56 px.
fn rail_label(group: Group) -> &'static str {
    match group {
        Group::General => "Gen",
        Group::Tools => "Tools",
        world => world.badge(),
    }
}

/// The icon slot every item shares.
const ICON: Pixels = px(16.0);

/// A vector icon in the slot.
fn glyph(icon: Icon) -> AnyElement {
    div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(ICON)
        .child(icon)
        .into_any_element()
}

/// How far a rail mark's disc sits past the icon's bottom-right corner.
const RAIL_MARK_OFFSET: f32 = 5.0;

/// What a click on an item runs.
type Run = Rc<dyn Fn(&mut Window, &mut App)>;

/// One place in the sidebar: the kit's sidebar fill on hover and while
/// active, its focus ring, any element as its icon and an optional activity
/// mark: at the row's end when expanded, on the icon's corner in the rail.
#[derive(IntoElement)]
pub(crate) struct SidebarItem {
    id: SharedString,
    label: SharedString,
    icon: AnyElement,
    active: bool,
    folded: bool,
    activity: Option<Shown>,
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
            activity: None,
            reveal: None,
            on_click: None,
        }
    }

    /// Marks the open place; clicking it again does nothing.
    pub(crate) fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    /// The section's runs: a ring while one is in flight, or a dot.
    pub(crate) fn activity(mut self, activity: Option<Shown>) -> Self {
        self.activity = activity;
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
        let focus = window
            .use_keyed_state(
                SharedString::from(format!("{}-focus", self.id)),
                cx,
                |_, cx| cx.focus_handle(),
            )
            .read(cx)
            .clone();
        let focused = focus.is_focused(window);
        let theme = cx.theme();
        let (ground, fill) = (theme.sidebar, theme.sidebar_accent);
        let fg = if self.active {
            theme.sidebar_accent_foreground
        } else {
            theme.sidebar_foreground
        };
        let hover_fg = theme.sidebar_accent_foreground;
        let label = self.label.clone();
        let mark_id = SharedString::from(format!("{}-activity", self.id));
        let (end_mark, corner_mark) = match (self.activity, self.folded) {
            (None, _) => (None, None),
            (Some(shown), false) => (Some(ActivityMark::new(mark_id, shown, false)), None),
            (Some(shown), true) => (None, Some(ActivityMark::new(mark_id, shown, true))),
        };
        // The rail mark sits on a disc in the row's current background, so it
        // reads over the icon.
        let icon = match corner_mark {
            None => self.icon,
            Some(mark) => {
                let row = self.id.clone();
                let disc = div()
                    .absolute()
                    .right(px(-RAIL_MARK_OFFSET))
                    .bottom(px(-RAIL_MARK_OFFSET))
                    .p(px(1.5))
                    .rounded_full_style(cx)
                    .map(|disc| {
                        if self.active {
                            disc.bg(fill)
                        } else {
                            disc.bg(ground).group_hover(row, |style| style.bg(fill))
                        }
                    })
                    .child(mark);
                div()
                    .relative()
                    .flex_none()
                    .child(self.icon)
                    .child(disc)
                    .into_any_element()
            }
        };
        let tooltip = match self.activity {
            Some(shown) => SharedString::from(format!("{label} · {}", shown.label())),
            None => label.clone(),
        };
        let item = div()
            .id(self.id.clone())
            .group(self.id)
            .role(Role::Link)
            .aria_label(label.clone())
            .aria_selected(self.active)
            .relative()
            .flex()
            .flex_none()
            .items_center()
            .gap_2()
            .h(ROW)
            .rounded(theme.radius)
            .border_1()
            .border_color(gpui::transparent_black())
            .text_size(TEXT_BODY)
            .text_color(fg)
            .cursor_pointer()
            .track_focus(&focus)
            .tab_index(0)
            .when(focused, |item| item.focus_ring_style(window, cx))
            .on_mouse_down(MouseButton::Left, |_, window, _| window.prevent_default())
            .map(|item| {
                if self.active {
                    item.bg(fill).font_weight(FontWeight::MEDIUM)
                } else {
                    item.hover(|style| style.bg(fill).text_color(hover_fg))
                }
            })
            .when_some(self.on_click.filter(|_| !self.active), |item, click| {
                item.on_click(move |_, window, cx| click(window, cx))
            })
            .child(icon)
            .when_some(reveal, |item, reveal| {
                item.child(reveal.absolute().top_0().left_0().size_full())
            });
        // One layout in both states, so only the sidebar's width animates: the
        // icon keeps its place and the rail hides the label instead of
        // dropping it, which would reflow the row mid-animation.
        item.px_2()
            .overflow_hidden()
            .when(self.folded, |item| {
                item.tooltip(move |window, cx| Tooltip::new(tooltip.clone()).build(window, cx))
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
            .children(end_mark)
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
