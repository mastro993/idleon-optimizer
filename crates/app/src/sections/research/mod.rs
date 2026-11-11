//! The Research Optimizer section: its upstream tab strip and the three panes.

pub mod grid;
mod inspector;
mod optimizer;
mod rates;

use crate::state::AppState;
use crate::ui::format;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, Window, div, px};

/// The research page's three upstream tabs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// Grid canvas, shape list and node inspector.
    #[default]
    Grid,
    /// Rate breakdown and the observation board.
    Rates,
    /// Target, effort and the last result.
    Run,
}

impl Tab {
    pub const ALL: [Tab; 3] = [Tab::Grid, Tab::Rates, Tab::Run];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Grid => "grid",
            Tab::Rates => "rates",
            Tab::Run => "run",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Grid => "Grid",
            Tab::Rates => "Rates",
            Tab::Run => "Optimize",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// A shape drag in flight: where the pointer grabbed it, and where it was.
#[derive(Clone, Debug)]
pub struct ShapeDrag {
    pub shape: usize,
    pub start_pointer: (f32, f32),
    pub start_position: (f64, f64),
}

/// The research section's live state, kept in `AppState` while the window is open.
#[derive(Default)]
pub struct State {
    pub tab: Tab,
    /// The selected square in the grid.
    pub selected_cell: Option<usize>,
    /// The shape the pointer is dragging, and where the drag started.
    pub shape_drag: Option<ShapeDrag>,
    /// The shape whose row or polygon was last picked.
    pub selected_shape: Option<usize>,
}

/// Register the grid's shortcuts: arrows, `+`/`-` and Escape.
pub fn bind_keys(cx: &mut App) {
    grid::bind_keys(cx);
}

/// The research section: the tab strip, then the active pane.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let rate = state.rate();
    let points = state.grid_points();
    let tab = state.research.tab;
    let colors = {
        let palette = &cx.theme().colors;
        (palette.accent, palette.fg_muted)
    };
    let summary = format!(
        "{} /hr · {} / {} grid points",
        format::rate(rate.total),
        format::count(points.spent as u64),
        format::count(points.earned as u64)
    );
    let header = div()
        .id("research-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(layout::app_tabs(state, cx))
        .child(div().flex_1())
        .child(
            div()
                .id("research-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Caption::new(format!("{} /hr", format::rate(rate.total))).text_color(colors.0),
                )
                .child(
                    Caption::new(format!(
                        "{} / {} points",
                        format::count(points.spent as u64),
                        format::count(points.earned as u64)
                    ))
                    .text_color(colors.1),
                ),
        );

    div()
        .id("research-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(pane(state, window, cx, tab))
        .into_any_element()
}

/// The active tab's pane.
fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    tab: Tab,
) -> AnyElement {
    match tab {
        Tab::Grid => {
            let grid = grid::grid_pane(state, window, cx).into_any_element();
            let inspector = inspector::inspector_pane(state, window, cx).into_any_element();
            layout::split_pane(
                "pane-grid",
                Breakpoint::of(window),
                grid,
                inspector,
                px(360.0),
            )
        }
        Tab::Rates => div()
            .id("pane-rates")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(rates::rates_pane(state, window, cx))
            .into_any_element(),
        Tab::Run => div()
            .id("pane-run")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(optimizer::optimizer_pane(state, window, cx))
            .into_any_element(),
    }
}
