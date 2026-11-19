//! Shared page parts and the responsive rules every section follows.
//!
//! Two-column panes use [`split_pane`], which stacks them when the workspace
//! beside the sidebar is narrower than [`COMPACT_WIDTH`], so the minimum
//! window never clips a fixed side rail.

use crate::sections::research::Tab;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, ElementId, Global, IntoElement, ParentElement, Pixels, Role,
    SharedString, Styled, Window, div, px,
};
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::tab::{Tab as KitTab, TabBar};
use idleon_ui::widgets::{Caption, Card, CardHeader, TEXT_KPI};
use std::rc::Rc;

/// The smallest window the layout is designed for.
pub(crate) const MIN_WIDTH: f32 = 1080.0;
/// The smallest window height the layout is designed for.
pub(crate) const MIN_HEIGHT: f32 = 680.0;
/// Below this workspace width, panes stack and fixed side rails drop under content.
pub(crate) const COMPACT_WIDTH: f32 = 1180.0;

/// The sidebar's width, which the shell publishes each frame so the
/// breakpoint measures the workspace instead of the whole window.
pub(crate) struct SidebarWidth(pub(crate) Pixels);

impl Global for SidebarWidth {}

/// The width beside the sidebar that a place renders into; content sized
/// from the window must measure this, not the viewport.
pub(crate) fn workspace_width(window: &Window, cx: &App) -> f32 {
    f32::from(window.viewport_size().width) - sidebar_width(cx)
}

/// The width the shell last published for the sidebar; none before the first frame.
fn sidebar_width(cx: &App) -> f32 {
    cx.try_global::<SidebarWidth>()
        .map_or(0.0, |width| f32::from(width.0))
}

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

    /// The room a `viewport` window leaves beside a `sidebar` wide sidebar.
    pub(crate) fn of_workspace(viewport: f32, sidebar: f32) -> Self {
        Self::from_width(viewport - sidebar)
    }

    /// The current window's room: the workspace beside the sidebar.
    pub(crate) fn of(window: &Window, cx: &App) -> Self {
        Self::of_workspace(f32::from(window.viewport_size().width), sidebar_width(cx))
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
            let stacked = div().flex().flex_col().flex_1().min_h_0().gap_3();
            let stacked = if side_first {
                stacked.child(side).child(primary)
            } else {
                stacked.child(primary).child(side)
            };
            stacked.overflow_y_scrollbar().id(id).into_any_element()
        }
    }
}

/// A headline figure: its label, its value and a one-line explanation, all in
/// one card.
pub(crate) fn figure(
    id: &'static str,
    label: &'static str,
    value: impl Into<SharedString>,
    caption: impl Into<SharedString>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
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
                            .text_size(TEXT_KPI)
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .child(value),
                    )
                    .child(Caption::new(caption.into()).text_color(muted)),
            ),
        )
        .into_any_element()
}

/// The height of an Ely tab strip (Ely's medium control height), which the
/// sections that still draw one pass to [`scrolling_tabs`].
const ELY_TAB_HEIGHT: Pixels = px(28.0);

/// A tab strip that scrolls sideways when its tabs need more room than the
/// workspace leaves them, instead of running past the window's edge. It takes
/// the full width in a column and shrinks beside siblings in a row; its height
/// is a tab's, so a column that squeezes its children never collapses it.
///
/// For the sections that still draw an Ely `TabBar`; a kit [`tab_bar`]
/// scrolls on its own.
pub(crate) fn scrolling_tabs(
    id: impl Into<ElementId>,
    tabs: impl IntoElement,
    _cx: &App,
) -> AnyElement {
    div()
        .flex()
        .w_full()
        .min_w_0()
        .h(ELY_TAB_HEIGHT)
        .min_h(ELY_TAB_HEIGHT)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .child(tabs)
                .overflow_x_scrollbar()
                .id(id),
        )
        .into_any_element()
}

/// A kit underline tab strip, each tab sized to its label, that scrolls
/// sideways when the tabs outgrow it. `tabs` are `(element id, label)` pairs;
/// each tab keeps its own element id, which the harnesses click, and
/// `on_select` gets the index of the tab chosen.
pub(crate) fn tab_bar(
    id: impl Into<ElementId>,
    tabs: impl IntoIterator<Item = (SharedString, SharedString)>,
    selected: usize,
    on_select: impl Fn(usize, &mut Window, &mut App) + 'static,
) -> TabBar {
    let on_select = Rc::new(on_select);
    let tabs = tabs.into_iter().enumerate().map(|(index, (id, label))| {
        let select = on_select.clone();
        // A kit tab's own id is its index, so the label carries the id; its
        // click is what the harnesses press, and the bar handles the rest.
        KitTab::new().aria_label(label.clone()).child(
            div()
                .id(id)
                .on_click(move |_, window, cx| select(index, window, cx))
                .child(label),
        )
    });
    TabBar::new(id)
        .underline()
        .min_w_0()
        .children(tabs)
        .selected_index(selected)
        .on_click(move |index, window, cx| on_select(*index, window, cx))
}

/// The Research tab strip; the active tab lives in the section's state.
pub(crate) fn app_tabs(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let selected = Tab::ALL
        .iter()
        .position(|tab| *tab == state.research.tab)
        .unwrap_or_default();
    let tabs = Tab::ALL.map(|tab| (tab.id().into(), tab.label().into()));
    tab_bar("app-tabs", tabs, selected, move |index, _window, cx| {
        entity.update(cx, |state, cx| state.set_tab(Tab::ALL[index], cx));
    })
    .flex_1()
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
    fn the_breakpoint_measures_the_workspace_beside_the_sidebar() {
        // (window width, sidebar width, expected room)
        let rows = [
            (1440.0, 228.0, Breakpoint::Regular),
            (1440.0, 56.0, Breakpoint::Regular),
            (1408.0, 228.0, Breakpoint::Regular),
            (1407.0, 228.0, Breakpoint::Compact),
            (1236.0, 56.0, Breakpoint::Regular),
            (1235.0, 56.0, Breakpoint::Compact),
            (1080.0, 56.0, Breakpoint::Compact),
            (1080.0, 228.0, Breakpoint::Compact),
        ];
        for (window, sidebar, expected) in rows {
            assert_eq!(
                Breakpoint::of_workspace(window, sidebar),
                expected,
                "{window} px window, {sidebar} px sidebar"
            );
        }
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
