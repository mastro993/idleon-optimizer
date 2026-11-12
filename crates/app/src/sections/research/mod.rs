//! The Research Optimizer section: its upstream tab strip and its panes.

pub mod dashboard;
pub mod grid;
mod inspector;
mod optimizer;
mod rates;
mod shape_opt;
mod shape_presets;

pub use dashboard::Trees;
pub use shape_presets::CustomPreset;

use crate::state::AppState;
use crate::ui::format;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::forms::TextInput;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};

/// The research page's tabs; the dashboard opens first, as upstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// Summary, grid and observation previews and the breakdown trees.
    #[default]
    Dashboard,
    /// Grid canvas, shape list and node inspector.
    Grid,
    /// Rate breakdown and the observation board.
    Rates,
    /// Target, effort and the last result.
    Run,
    /// Shape priority tiers, presets and the optimized placement.
    Shapes,
}

impl Tab {
    pub const ALL: [Tab; 5] = [Tab::Dashboard, Tab::Grid, Tab::Rates, Tab::Run, Tab::Shapes];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Dashboard => "dashboard",
            Tab::Grid => "grid",
            Tab::Rates => "rates",
            Tab::Run => "run",
            Tab::Shapes => "shapes",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Grid => "Grid",
            Tab::Rates => "Rates",
            Tab::Run => "Optimize",
            Tab::Shapes => "Shape Optimization",
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
pub struct State {
    pub tab: Tab,
    /// The selected square in the grid.
    pub selected_cell: Option<usize>,
    /// The shape the pointer is dragging, and where the drag started.
    pub shape_drag: Option<ShapeDrag>,
    /// The shape whose row or polygon was last picked.
    pub selected_shape: Option<usize>,
    /// The observation the dashboard preview last opened.
    pub selected_observation: Option<usize>,
    /// The named shape tier lists the project keeps.
    pub shape_presets: Vec<CustomPreset>,
    /// The preset the shape tiers were last applied from: a built-in `_id` or a
    /// custom name. An edit clears it, so a built-in is never rewritten.
    pub active_shape_preset: Option<String>,
    /// The name typed into the shape preset bar.
    pub preset_name: Entity<TextInput>,
    /// Cached bare-grid cell values, with the simulation and context they came
    /// from, so typing in the preset rail does not rescore 240 cells per frame.
    pub shape_values: Option<(idleon_core::Simulation, idleon_core::SimContext, Vec<f64>)>,
}

impl State {
    pub fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let preset_name = cx.new(|cx| {
            TextInput::new(window, cx)
                .placeholder("Shape preset name")
                .label("Shape preset name")
        });
        Self {
            tab: Tab::default(),
            selected_cell: None,
            shape_drag: None,
            selected_shape: None,
            selected_observation: None,
            shape_presets: Vec::new(),
            active_shape_preset: None,
            preset_name,
            shape_values: None,
        }
    }
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
        Tab::Dashboard => div()
            .id("pane-dashboard")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(dashboard::dashboard_pane(state, window, cx))
            .into_any_element(),
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
        Tab::Shapes => div()
            .id("pane-shapes")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(shape_opt::pane(state, window, cx))
            .into_any_element(),
    }
}
