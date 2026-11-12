//! Shared page chrome and the responsive rules every section follows.
//!
//! A section renders the same header (back, title, world badge, save status,
//! import) and owns its upstream tab strip. Two-column panes use [`split_pane`],
//! which stacks them below [`COMPACT_WIDTH`] so the minimum window never clips
//! a fixed side rail.

use crate::sections::{self, research::Tab};
use crate::state::{AppState, SaveStatus};
use crate::ui::theme::WorldBadge;
use ely_gpui_component::buttons::{Button, IconButton};
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Heading;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Div, ElementId, Entity, Hsla, IntoElement, ParentElement, Pixels, Role,
    SharedString, Stateful, Styled, Window, div, px,
};

/// The smallest window the layout is designed for.
pub(crate) const MIN_WIDTH: f32 = 1080.0;
/// The smallest window height the layout is designed for.
pub(crate) const MIN_HEIGHT: f32 = 680.0;
/// Below this window width, panes stack and fixed side rails drop under content.
pub(crate) const COMPACT_WIDTH: f32 = 1360.0;

/// The room a window leaves for the layout.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Breakpoint {
    /// Stacked panes and full-width cards.
    Compact,
    /// Side-by-side panes with fixed rails.
    Regular,
}

impl Breakpoint {
    /// The room a viewport `width` leaves.
    pub(crate) fn from_width(width: f32) -> Self {
        if width < COMPACT_WIDTH {
            Self::Compact
        } else {
            Self::Regular
        }
    }

    /// The current window's room.
    pub(crate) fn of(window: &Window) -> Self {
        Self::from_width(f32::from(window.viewport_size().width))
    }

    pub(crate) fn is_compact(self) -> bool {
        self == Self::Compact
    }

    /// The narrowest a dashboard card may be at this width.
    pub(crate) fn card_min(self) -> Pixels {
        match self {
            Self::Compact => px(280.0),
            Self::Regular => px(300.0),
        }
    }
}

/// The height a scrolling data table may take in a window `viewport_height` tall.
///
/// Leaves room for the header, tab strip, KPI cards and the table's own header
/// and footnote, so a table scrolls instead of pushing the page off-screen.
pub(crate) fn table_height(viewport_height: f32) -> Pixels {
    px((viewport_height - 400.0).clamp(160.0, 520.0))
}

/// Two panes side by side, or stacked in a scrolling page when compact.
///
/// `side_width` is the fixed rail width; it only applies while regular.
pub(crate) fn split_pane(
    id: impl Into<ElementId>,
    breakpoint: Breakpoint,
    primary: impl IntoElement,
    side: impl IntoElement,
    side_width: Pixels,
) -> AnyElement {
    split_pane_inner(id, breakpoint, primary, side, side_width, false)
}

/// Like [`split_pane`], with the side rail first once the panes stack.
pub(crate) fn split_pane_reversed(
    id: impl Into<ElementId>,
    breakpoint: Breakpoint,
    primary: impl IntoElement,
    side: impl IntoElement,
    side_width: Pixels,
) -> AnyElement {
    split_pane_inner(id, breakpoint, primary, side, side_width, true)
}

fn split_pane_inner(
    id: impl Into<ElementId>,
    breakpoint: Breakpoint,
    primary: impl IntoElement,
    side: impl IntoElement,
    side_width: Pixels,
    side_first: bool,
) -> AnyElement {
    let primary = primary.into_any_element();
    let side = side.into_any_element();
    let id = id.into();
    match breakpoint {
        Breakpoint::Regular => div()
            .id(id)
            .flex()
            .flex_1()
            .min_h_0()
            .gap_3()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .min_h_0()
                    .child(primary),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_none()
                    .w(side_width)
                    .min_h_0()
                    .child(side),
            )
            .into_any_element(),
        Breakpoint::Compact => {
            let stacked = div().flex().flex_col().gap_3();
            let stacked = if side_first {
                stacked.child(side).child(primary)
            } else {
                stacked.child(primary).child(side)
            };
            ScrollArea::new(id)
                .vertical()
                .flex_1()
                .min_h_0()
                .child(stacked)
                .into_any_element()
        }
    }
}

/// A headline figure as the upstream `_card` draws it: its label, its value and
/// a one-line explanation, all in one card.
pub(crate) fn figure(
    id: &'static str,
    label: &'static str,
    value: impl Into<SharedString>,
    caption: impl Into<SharedString>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    use ely_gpui_component::layout::{Card, CardHeader};
    use ely_gpui_component::typography::Caption;

    let muted = cx.theme().colors.fg_muted;
    let value = value.into();
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{label}: {value}"))
        .flex()
        .flex_col()
        .flex_1()
        .min_w(px(190.0))
        .child(
            Card::new().header(CardHeader::new(label)).child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_size(px(20.0))
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    )
                    .child(Caption::new(caption.into()).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// The shared section header: back, icon, title, world badge, save status, import.
pub(crate) fn page_header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(section) = state.current_section() else {
        return div().into_any_element();
    };
    let (border, muted) = {
        let colors = &cx.theme().colors;
        (colors.border, colors.fg_muted)
    };
    let entity = cx.entity();
    let back = entity.clone();
    let import = entity.clone();
    let export = entity.clone();

    header_bar(border)
        .child(back_button(back))
        .child(div().text_size(px(18.0)).child(section.icon))
        .child(
            div()
                .id("section-title")
                .role(Role::Heading)
                .aria_label(section.title)
                .min_w_0()
                .child(Heading::new(3, section.title)),
        )
        .child(WorldBadge::new(section.group))
        .child(div().flex_1())
        .child(save_actions(state.save_status(), muted, import, export))
        .into_any_element()
}

/// The bar every header sits in. Its left padding leaves room for the macOS
/// traffic lights in the transparent title bar.
fn header_bar(border: Hsla) -> Stateful<Div> {
    div()
        .id("app-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .pl(px(84.0))
        .pr_4()
        .h(px(48.0))
        .border_b_1()
        .border_color(border)
}

/// The header's way back to the dashboard.
fn back_button(state: Entity<AppState>) -> IconButton {
    IconButton::new("back-to-dashboard", IconName::ArrowLeft)
        .tooltip("Back to dashboard")
        .on_click(move |_event, window, cx| {
            state.update(cx, |state, cx| state.back_to_dashboard(window, cx));
        })
}

/// The landing page's header: product name, save status and the shared import action.
pub(crate) fn dashboard_header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let colors = &cx.theme().colors;
    let import = cx.entity();
    let export = cx.entity();
    header_bar(colors.border)
        .child(Heading::new(3, "Idleon Optimizer"))
        .child(
            div()
                .id("header-subtitle")
                .role(Role::Label)
                .aria_label("Dashboard")
                .text_size(px(12.0))
                .text_color(colors.fg_muted)
                .child("Dashboard"),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("header-cards")
                .role(Role::Label)
                .aria_label(format!("{} calculators", sections::card_count()))
                .text_size(px(12.0))
                .text_color(colors.fg_muted)
                .child(format!("{} calculators", sections::card_count())),
        )
        .child(save_actions(
            state.save_status(),
            colors.fg_muted,
            import,
            export,
        ))
        .into_any_element()
}

/// The save status and the import action shared by the dashboard and each section.
fn save_actions(
    save: SaveStatus,
    muted: gpui::Hsla,
    import: gpui::Entity<AppState>,
    export: gpui::Entity<AppState>,
) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap_3()
        .child(
            div()
                .id("header-save")
                .role(Role::Label)
                .aria_label(save.label())
                .text_size(px(12.0))
                .text_color(muted)
                .child(save.label()),
        )
        .child(
            Button::new("header-export", "Export")
                .size(ControlSize::Sm)
                .icon(IconName::Download)
                .on_click(move |_event, window, cx| {
                    export.update(cx, |state, cx| state.open_export(window, cx));
                }),
        )
        .child(
            Button::new("header-import", "Import")
                .size(ControlSize::Sm)
                .icon(IconName::Upload)
                .on_click(move |_event, window, cx| {
                    import.update(cx, |state, cx| state.open_import(window, cx));
                }),
        )
}

/// The shared tab strip; the section that owns it passes its active tab in state.
pub(crate) fn app_tabs(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("app-tabs").selected(SharedString::from(state.research.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.set_tab(tab, cx));
        }
    })
}

/// The header for an id the registry does not declare.
pub(crate) fn missing_header(id: &str, state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let (border, muted) = {
        let colors = &cx.theme().colors;
        (colors.border, colors.fg_muted)
    };
    let entity = cx.entity();
    let back = entity.clone();
    let import = entity.clone();
    let export = entity.clone();
    header_bar(border)
        .child(back_button(back))
        .child(
            div()
                .id("section-title")
                .role(Role::Heading)
                .aria_label(format!("Unknown section {id}"))
                .min_w_0()
                .child(Heading::new(3, "Unknown section")),
        )
        .child(div().flex_1())
        .child(save_actions(state.save_status(), muted, import, export))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_minimum_window_is_compact_and_keeps_cards_readable() {
        const { assert!(MIN_WIDTH < COMPACT_WIDTH) };
        assert_eq!(
            Breakpoint::from_width(MIN_WIDTH),
            Breakpoint::Compact,
            "the minimum window stacks panes instead of clipping side rails"
        );
        assert!(Breakpoint::Compact.card_min() <= px(280.0));
        assert!(Breakpoint::Regular.card_min() < px(320.0));
    }

    #[test]
    fn a_table_shrinks_with_the_window_but_keeps_scroll_room() {
        assert_eq!(table_height(900.0), px(500.0));
        assert_eq!(table_height(680.0), px(280.0));
        assert_eq!(table_height(500.0), px(160.0));
        assert_eq!(table_height(2_000.0), px(520.0));
    }

    #[test]
    fn the_breakpoint_is_inclusive_at_regular() {
        assert_eq!(
            Breakpoint::from_width(COMPACT_WIDTH - 1.0),
            Breakpoint::Compact
        );
        assert_eq!(Breakpoint::from_width(COMPACT_WIDTH), Breakpoint::Regular);
    }
}
