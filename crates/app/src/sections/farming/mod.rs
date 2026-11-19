//! The Farming Planner section: the Overview tab, the 9×4 field with the
//! selected plot's detail, the plot assignment, the Land Rank allocation, the
//! named presets and the preset comparison.
//!
//! FM-01 builds the field tab: every plot renders from `farmPlotData` and the
//! right rail reads the selected plot's saved row and formulas. FM-02 adds the
//! plot assignment and the Land Rank allocation: the objective decides the
//! crop each plot should grow, and the allocator spends the account's Land Rank
//! points off the UI thread through the shared task runner. FM-03 adds the
//! "Preset Comparison" table — every goal planned from the current baseline —
//! and the named presets the project document keeps. FM-04 adds the
//! Evolution & OG tab — the evolution chain to the target crop, its hardest
//! remaining steps, the overgrowth stopping advice, every plot's overgrowth
//! curve and the evolution assignment. FM-05 adds the Overview tab —
//! the priority signals, account multipliers, GMO thresholds, Crop Scientist
//! bonuses and growth breakpoints.

mod actions;

use crate::state::AppState;
use crate::ui::{self, layout};
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_farming::{self as farming, Assignment, Farm, Mode, Objective};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, PANEL_GAP, show_number};

mod allocation;
mod assignment;
mod comparison;
mod controls;
mod curves;
mod detail;
mod evolution;
mod field;
mod overview;
mod presets;
mod table;

pub(crate) use controls::Controls;
pub(crate) use presets::CustomPreset;

/// The farming page's tabs, in order.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
    /// The signals, multipliers, thresholds, Crop Scientist and curves.
    #[default]
    Overview,
    /// The full 9×4 field, the plot detail, the assignment and the allocator.
    Plots,
    /// The evolution chain, its hardest steps and the overgrowth stopping.
    Evolution,
}

impl Tab {
    pub(crate) const ALL: [Tab; 3] = [Tab::Overview, Tab::Plots, Tab::Evolution];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Overview => "overview",
            Tab::Plots => "plots",
            Tab::Evolution => "evolution",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Plots => "Full Plot Grid",
            Tab::Evolution => "Evolution & OG",
        }
    }
}

/// The field's own state: the tab, the selected plot, the crop the evolution
/// features aim at, the objective, the allocation mode and the named presets,
/// all kept alive across dashboard visits. The assignment is derived from the
/// loaded farm, so the project document does not carry it.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct State {
    pub(crate) tab: Tab,
    pub(crate) selected_plot: usize,
    pub(crate) target_crop: u32,
    /// The objective the assignment and the Land Rank allocation plan for.
    pub(crate) objective: Objective,
    /// Whether the allocator keeps the saved levels or reallocates every point.
    pub(crate) mode: Mode,
    /// The named presets the project document keeps.
    pub(crate) presets: Vec<CustomPreset>,
    /// The field's assignment for `objective`; derived from the loaded farm, so
    /// the project document does not carry it.
    #[serde(skip)]
    pub(crate) assignment: Assignment,
}

impl Default for State {
    fn default() -> Self {
        Self {
            tab: Tab::default(),
            selected_plot: 0,
            target_crop: 1,
            objective: Objective::default(),
            mode: Mode::default(),
            presets: Vec::new(),
            assignment: Assignment::default(),
        }
    }
}

impl State {
    /// Select a plot, clamped to the field.
    pub(crate) fn select(&mut self, plot: usize) {
        self.selected_plot = plot.min(farming::PLOTS - 1);
    }

    /// Switch tab.
    pub(crate) fn set_tab(&mut self, tab: Tab) {
        self.tab = tab;
    }

    /// Aim at a crop id, clamped to the last seed family.
    pub(crate) fn set_target_crop(&mut self, value: f64) {
        let max = farming::data()
            .seeds
            .last()
            .map(|seed| seed.end as u32)
            .unwrap_or(1);
        self.target_crop = value.round().clamp(1.0, f64::from(max)) as u32;
    }

    /// Plan for a new objective.
    pub(crate) fn set_objective(&mut self, objective: Objective) {
        self.objective = objective;
    }

    /// Keep the saved levels or reallocate every point.
    pub(crate) fn set_mode(&mut self, mode: Mode) {
        self.mode = mode;
    }

    /// Save the current goal, baseline and crop target under `name`.
    pub(crate) fn save_preset(&mut self, name: &str) -> Result<String, String> {
        CustomPreset::store(
            &mut self.presets,
            name,
            self.objective,
            self.mode,
            self.target_crop,
        )
    }

    /// Apply the preset at `index` to the section's inputs; returns its name.
    pub(crate) fn apply_preset(&mut self, index: usize) -> Option<String> {
        let preset = self.presets.get(index)?.clone();
        self.objective = preset.objective;
        self.mode = preset.mode;
        self.target_crop = preset.target_crop;
        Some(preset.name)
    }

    /// Forget the preset at `index`.
    pub(crate) fn delete_preset(&mut self, index: usize) -> Option<CustomPreset> {
        CustomPreset::delete(&mut self.presets, index)
    }

    /// Rebuild the assignment for the field, objective and target crop.
    pub(crate) fn adopt(&mut self, farm: &Farm) {
        self.assignment = farm.assignment(self.objective, Some(self.target_crop));
    }
}

/// The section: a summary line, the tab strip and the tab's pane.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let header = section_header(state, window, cx);
    let tabs = tabs(state, cx);
    let pane = match state.farming.tab {
        Tab::Overview => overview::overview(state, cx),
        Tab::Evolution => evolution::evolution(state, cx),
        Tab::Plots => plots(state, window, cx),
    };

    v_flex()
        .id("farming-section")
        .size_full()
        .min_h_0()
        .gap(PANEL_GAP)
        .child(header)
        .child(tabs)
        .child(pane)
        .into_any_element()
}

/// The Full Plot Grid tab: the field beside the plot detail, then the
/// assignment, the presets, the allocator and the comparison.
fn plots(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let breakpoint = layout::Breakpoint::of(window, cx);
    let field = state.farming_field();
    let selected = state.farming.selected_plot.min(farming::PLOTS - 1);
    let grid = field::field_card(
        &field.plots,
        selected,
        state.farm.has_plots(),
        &state.farming.assignment,
        cx,
    );
    let detail = detail::detail_pane(state, &field.plots[selected], &field.snapshot, cx);
    let assignment = assignment::assignment_card(&state.farming.assignment, cx);
    let presets = presets::presets_card(state, cx);
    let allocation = allocation::allocation_card(state, cx);
    let comparison = comparison::comparison_card(state, cx);
    let body = v_flex()
        .gap(PANEL_GAP)
        .child(layout::split_pane(
            "farming-body",
            breakpoint,
            grid,
            detail,
            px(360.0),
        ))
        .child(assignment)
        .child(presets)
        .child(allocation)
        .child(comparison);
    div()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("farming-scroll")
        .into_any_element()
}

/// The tab strip, driven by the state's own tab.
fn tabs(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    layout::tab_bar(
        "farming-tabs",
        Tab::ALL.map(|tab| (tab.id().into(), tab.label().into())),
        Tab::ALL
            .iter()
            .position(|tab| *tab == state.farming.tab)
            .unwrap_or(0),
        move |index, _window, cx| {
            entity.update(cx, |state, cx| state.set_farming_tab(Tab::ALL[index], cx));
        },
    )
    .into_any_element()
}

/// The inputs a comparison run captures, so a later edit marks its table stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompareInputs {
    pub(crate) save_revision: u64,
    pub(crate) mode: Mode,
    pub(crate) target_crop: u32,
}

/// The inputs a run captures, so a later edit marks its result stale.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RunInputs {
    pub(crate) save_revision: u64,
    pub(crate) objective: Objective,
    pub(crate) mode: Mode,
    pub(crate) target_crop: u32,
}

/// The crops, growth rate, points, objective and crop target above the tabs.
fn section_header(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let (accent, muted) = (cx.theme().primary, cx.theme().muted_foreground);
    let growth = state.farming_field().snapshot.growth_rate;
    let points = state.farm.land_rank_points_left();
    let summary = format!(
        "{} plots · {} crops · {} growth · {} rank point(s) unspent",
        farming::PLOTS,
        format::number(f64::from(state.farm.crop_count)),
        format::number(growth),
        points
    );
    let target = state.farming.target_crop;
    let controls = &state.farming_controls;
    // Values flow section state -> control; edits come back through the
    // subscriptions in `Controls::new`.
    controls.objective.show(
        controls::objective_choices(),
        Some(&state.farming.objective),
        window,
        cx,
    );
    show_number(&controls.target, f64::from(target), 0, window, cx);
    h_flex()
        .id("farming-header")
        .flex_none()
        .gap_3()
        .child(
            h_flex()
                .id("farming-summary")
                .role(gpui::Role::Label)
                .aria_label(summary)
                .gap_4()
                .child(Caption::new(format!("{} crops", state.farm.crop_count)).text_color(accent))
                .child(
                    Caption::new(format!(
                        "{} growth · {} unspent",
                        format::number(growth),
                        points
                    ))
                    .text_color(muted),
                ),
        )
        .child(div().flex_1())
        .child(
            h_flex()
                .id("farming-objective-field")
                .gap_2()
                .child(Caption::new("Objective").text_color(muted))
                .child(
                    div().flex_none().w(px(170.0)).child(
                        Select::new(controls.objective.state())
                            .id("farming-objective")
                            .accessibility_label("Objective")
                            .small(),
                    ),
                ),
        )
        .child(
            h_flex()
                .id("farming-crop-selection")
                .role(gpui::Role::Group)
                .aria_label(format!("Crop selection: target crop {target}"))
                .gap_2()
                .child(Caption::new("Target crop").text_color(muted))
                .child(ui::number_field(
                    "farming-target",
                    &controls.target,
                    "Target crop",
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_stays_inside_the_field() {
        let mut state = State::default();
        state.select(41);
        assert_eq!(state.selected_plot, farming::PLOTS - 1);
        state.select(3);
        assert_eq!(state.selected_plot, 3);
    }

    #[test]
    fn the_target_crop_stays_within_the_families() {
        let mut state = State::default();
        state.set_target_crop(0.0);
        assert_eq!(state.target_crop, 1);
        state.set_target_crop(400.0);
        assert_eq!(state.target_crop, 329);
        state.set_target_crop(230.6);
        assert_eq!(state.target_crop, 231);
    }

    #[test]
    fn the_tab_strip_opens_on_the_overview_and_keeps_its_ids() {
        let mut state = State::default();
        assert_eq!(state.tab, Tab::Overview);
        assert_eq!(
            Tab::ALL.map(Tab::label),
            ["Overview", "Full Plot Grid", "Evolution & OG"]
        );
        assert_eq!(Tab::ALL.map(Tab::id), ["overview", "plots", "evolution"]);
        state.set_tab(Tab::Plots);
        assert_eq!(state.tab, Tab::Plots);
    }

    #[test]
    fn adopting_a_farm_rebuilds_the_assignment_for_the_objective() {
        let mut farm = Farm::default();
        farm.crop_quantities.insert(0, 500.0);
        farm.crop_quantities.insert(230, 500.0);
        farm.plots[0].seed_family = 0;
        let mut state = State::default();
        state.adopt(&farm);
        assert_eq!(state.assignment.objective, Objective::RankXp);
        assert_eq!(state.assignment.rows.len(), farming::PLOTS);
        // The assignment ranks the same crops whatever the plot's history.
        assert_eq!(state.assignment.rows[0].current_crop, 0);
        state.set_objective(Objective::Evolution);
        state.adopt(&farm);
        assert_eq!(state.assignment.objective, Objective::Evolution);
        assert_eq!(state.assignment.target_crop, Some(state.target_crop - 1));
    }

    #[test]
    fn a_saved_preset_restores_the_section_inputs() {
        let mut state = State::default();
        state.set_objective(Objective::Beans);
        state.set_mode(Mode::Reset);
        state.set_target_crop(230.0);
        state.save_preset(" Beans run ").expect("a name saves");
        assert_eq!(state.presets.len(), 1);

        state.set_objective(Objective::RankXp);
        state.set_mode(Mode::Unspent);
        state.set_target_crop(1.0);
        assert_eq!(state.apply_preset(0).as_deref(), Some("Beans run"));
        assert_eq!(state.objective, Objective::Beans);
        assert_eq!(state.mode, Mode::Reset);
        assert_eq!(state.target_crop, 230);
        assert!(state.apply_preset(9).is_none());

        let removed = state.delete_preset(0).expect("the preset deletes");
        assert_eq!(removed.name, "Beans run");
        assert!(state.presets.is_empty());
    }

    #[test]
    fn the_objective_and_mode_survive_a_project_round_trip() {
        let mut state = State::default();
        state.set_objective(Objective::Beans);
        state.set_mode(Mode::Reset);
        state.set_tab(Tab::Plots);
        state.save_preset("Beans run").expect("a name saves");
        let text = serde_json::to_string(&state).expect("the farming state serializes");
        let restored: State = serde_json::from_str(&text).expect("the farming state parses");
        assert_eq!(restored.objective, Objective::Beans);
        assert_eq!(restored.mode, Mode::Reset);
        assert_eq!(restored.tab, Tab::Plots);
        assert_eq!(restored.presets.len(), 1);
        assert_eq!(restored.presets[0].name, "Beans run");
        assert_eq!(
            restored.assignment.rows.len(),
            farming::PLOTS,
            "a restored state starts with the empty assignment until the farm is adopted"
        );
    }
}
