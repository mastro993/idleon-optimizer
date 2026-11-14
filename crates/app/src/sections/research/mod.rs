//! The Research Optimizer section: its upstream tab strip and its panes.

mod actions;
pub(crate) mod colors;

pub(crate) mod dashboard;
pub(crate) mod grid;
mod insight_roi;
mod inspector;
pub(crate) mod minehead;
mod optimizer;
mod rates;
mod result;
mod shape_opt;
mod shape_presets;
pub(crate) mod sushi;
mod unlocks;

pub(crate) use dashboard::Trees;
pub(crate) use shape_presets::CustomPreset;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::forms::TextInput;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, Entity, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::edit::{History, Snapshot};
use idleon_ui::format;

/// The research page's tabs; the dashboard opens first, as upstream.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// Summary, grid and observation previews and the breakdown trees.
    #[default]
    Dashboard,
    /// Grid canvas, shape list and node inspector.
    Grid,
    /// Rate breakdown and the observation board.
    Rates,
    /// Target, effort and the last result.
    Run,
    /// Per-observation insight levelling: time, EXP lost and break-even.
    InsightRoi,
    /// Undiscovered observations, ranked by the EXP they add per day of rolls.
    Unlocks,
    /// Shape priority tiers, presets and the optimized placement.
    Shapes,
    /// The Depth Charge dashboard, strategy search, ranking, path and game.
    Minehead,
    /// The Sushi Station overview, upgrades, currency, ranking and tiers.
    Sushi,
}

impl Tab {
    pub(crate) const ALL: [Tab; 9] = [
        Tab::Dashboard,
        Tab::Grid,
        Tab::Rates,
        Tab::Run,
        Tab::InsightRoi,
        Tab::Unlocks,
        Tab::Shapes,
        Tab::Minehead,
        Tab::Sushi,
    ];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Dashboard => "dashboard",
            Tab::Grid => "grid",
            Tab::Rates => "rates",
            Tab::Run => "run",
            Tab::InsightRoi => "insight",
            Tab::Unlocks => "unlocks",
            Tab::Shapes => "shapes",
            Tab::Minehead => "minehead",
            Tab::Sushi => "sushi",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Dashboard => "Dashboard",
            Tab::Grid => "Grid",
            Tab::Rates => "Rates",
            Tab::Run => "Optimize",
            Tab::InsightRoi => "Insight ROI",
            Tab::Unlocks => "Unlocks",
            Tab::Shapes => "Shape Optimization",
            Tab::Minehead => "Minehead",
            Tab::Sushi => "Sushi Station",
        }
    }

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// A shape drag in flight: where the pointer grabbed it, and where it was.
#[derive(Clone, Debug)]
pub(crate) struct ShapeDrag {
    pub(crate) shape: usize,
    pub(crate) start_pointer: (f32, f32),
    pub(crate) start_position: (f64, f64),
    /// The editable state the drag started from, recorded when it ends.
    pub(crate) before: Snapshot,
}

/// A rotation drag on the shape's canvas handle.
#[derive(Clone, Debug)]
pub(crate) struct ShapeRotate {
    pub(crate) shape: usize,
    /// The shape's rotation centre, in window pixels.
    pub(crate) centre: (f32, f32),
    /// The pointer's angle around the centre when the handle was grabbed.
    pub(crate) start_angle: f32,
    pub(crate) start_rot: f64,
    pub(crate) before: Snapshot,
}

/// The research section's live state, kept in `AppState` while the window is open.
pub(crate) struct State {
    pub(crate) tab: Tab,
    /// The selected square in the grid.
    pub(crate) selected_cell: Option<usize>,
    /// The shape the pointer is dragging, and where the drag started.
    pub(crate) shape_drag: Option<ShapeDrag>,
    /// The rotation drag the canvas handle is driving, if one is in flight.
    pub(crate) shape_rotate: Option<ShapeRotate>,
    /// The manual edits that can be undone and redone, newest last.
    pub(crate) history: History,
    /// The shape whose row or polygon was last picked.
    pub(crate) selected_shape: Option<usize>,
    /// The observation the dashboard preview last opened.
    pub(crate) selected_observation: Option<usize>,
    /// The action the last run's timeline marks, by index.
    pub(crate) run_phase: Option<usize>,
    /// The named shape tier lists the project keeps.
    pub(crate) shape_presets: Vec<CustomPreset>,
    /// The preset the shape tiers were last applied from: a built-in `_id` or a
    /// custom name. An edit clears it, so a built-in is never rewritten.
    pub(crate) active_shape_preset: Option<String>,
    /// The name typed into the shape preset bar.
    pub(crate) preset_name: Entity<TextInput>,
    /// Cached bare-grid cell values, with the simulation and context they came
    /// from, so typing in the preset rail does not rescore 240 cells per frame.
    pub(crate) shape_values: Option<(
        idleon_research::Simulation,
        idleon_research::SimContext,
        Vec<f64>,
    )>,
    /// The Minehead tab's own state.
    pub(crate) minehead: minehead::State,
    /// The Sushi Station tab's own state.
    pub(crate) sushi: sushi::State,
    /// The Insight ROI table's sort mode.
    pub(crate) insight_sort: idleon_research::insight::Sort,
    /// Whether the Insight ROI table hides rows that lose no EXP. Upstream
    /// starts hidden; a first run here would then show an empty table, so this
    /// build starts with every usable observation listed.
    pub(crate) insight_hide_free: bool,
    /// Which undiscovered observations the Unlocks table lists.
    pub(crate) unlock_filter: idleon_research::unlock::Filter,
    /// The Observation Unlocks name filter.
    pub(crate) unlock_search: Entity<TextInput>,
    _unlock_search_subscription: gpui::Subscription,
}

impl State {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let preset_name =
            crate::ui::text_input("Shape preset name", "Shape preset name", window, cx);
        let unlock_search =
            crate::ui::text_input("Observation name", "Find an observation", window, cx);
        let search_subscription = cx.subscribe(
            &unlock_search,
            |_state, _input, event: &ely_gpui_component::forms::InputEvent, cx| {
                if *event == ely_gpui_component::forms::InputEvent::Changed {
                    cx.notify();
                }
            },
        );
        Self {
            tab: Tab::default(),
            selected_cell: None,
            shape_drag: None,
            shape_rotate: None,
            history: History::new(),
            selected_shape: None,
            selected_observation: None,
            run_phase: None,
            shape_presets: Vec::new(),
            active_shape_preset: None,
            preset_name,
            shape_values: None,
            minehead: minehead::State::new(),
            sushi: sushi::State::new(),
            insight_sort: idleon_research::insight::Sort::default(),
            insight_hide_free: false,
            unlock_filter: idleon_research::unlock::Filter::default(),
            unlock_search,
            _unlock_search_subscription: search_subscription,
        }
    }
}

/// The tiers a built-in preset resolves to against the loaded save.
///
/// The Minehead preset's ranking reads the save's currency and damage terms,
/// which live in `idleon-minehead`; the research crate cannot depend on it, so
/// the app resolves that one preset here and the rest through the simulation.
pub(crate) fn preset_tiers(
    state: &AppState,
    preset: &idleon_research::data::Preset,
) -> Option<idleon_research::Tiers> {
    if preset.id == "_minehead"
        && let Some(inputs) = state.research.minehead.inputs.as_ref()
    {
        let shape_pct = idleon_research::game()
            .shapes
            .iter()
            .map(|shape| shape.bonus_pct)
            .fold(f64::MIN, f64::max);
        let context = inputs.shape_context(shape_pct, 5.0);
        return Some(idleon_research::Tiers::from_above(
            idleon_minehead::math::shape_priority(&context),
        ));
    }
    preset.tiers_for(&state.simulation)
}

/// Register the grid's shortcuts: arrows, `+`/`-` and Escape.
pub(crate) fn bind_keys(cx: &mut App) {
    grid::bind_keys(cx);
}

/// The research section: the tab strip, then the active pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
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
        Tab::InsightRoi => div()
            .id("pane-insight")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(insight_roi::pane(state, window, cx))
            .into_any_element(),
        Tab::Unlocks => div()
            .id("pane-unlocks")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(unlocks::pane(state, window, cx))
            .into_any_element(),
        Tab::Shapes => div()
            .id("pane-shapes")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(shape_opt::pane(state, window, cx))
            .into_any_element(),
        Tab::Minehead => div()
            .id("pane-minehead")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(minehead::pane(state, window, cx))
            .into_any_element(),
        Tab::Sushi => div()
            .id("pane-sushi")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .child(sushi::pane(state, window, cx))
            .into_any_element(),
    }
}
