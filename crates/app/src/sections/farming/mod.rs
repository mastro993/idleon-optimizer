//! The Farming Planner section: the upstream Overview tab, the 9×4 field with
//! the selected plot's detail, the plot assignment, the Land Rank allocation,
//! the named presets and the preset comparison.
//!
//! FM-01 ports the upstream field tab: every plot renders from `farmPlotData`
//! and the right rail reads the selected plot's saved row and formulas. FM-02
//! adds `plotAssignmentPlan` and `optimizeLandRank`: the objective decides the
//! crop each plot should grow, and the allocator spends the account's Land Rank
//! points off the UI thread through the shared task runner. FM-03 adds the
//! upstream "Preset Comparison" table — every goal planned from the current
//! baseline — and the named presets the project document keeps. FM-04 adds the
//! Evolution & OG tab — the evolution chain to the target crop, its hardest
//! remaining steps, the overgrowth stopping advice, every plot's overgrowth
//! curve and the evolution assignment. FM-05 adds the upstream Overview tab —
//! the priority signals, account multipliers, GMO thresholds, Crop Scientist
//! bonuses and growth breakpoints.

mod actions;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::ScrollArea;
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_farming::{self as farming, Assignment, Farm, Mode, Objective, PlotMetrics, Snapshot};
use idleon_ui::format;

mod allocation;
mod assignment;
mod comparison;
mod curves;
mod detail;
mod evolution;
mod field;
mod overview;
mod presets;

pub(crate) use presets::CustomPreset;

/// The farming page's tabs, in the upstream order.
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

    pub(crate) fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
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
    let snapshot = state.farm.snapshot();
    let header = section_header(state, &snapshot, cx);
    let tabs = tabs(state, cx);
    let pane = match state.farming.tab {
        Tab::Overview => overview::overview(state, cx),
        Tab::Evolution => evolution::evolution(state, cx),
        Tab::Plots => {
            let breakpoint = Breakpoint::of(window);
            let plots: Vec<PlotMetrics> = (0..farming::PLOTS)
                .map(|index| state.farm.plot_metrics_with(index, &snapshot))
                .collect();
            let selected = state.farming.selected_plot.min(farming::PLOTS - 1);
            let field = field::field_card(
                &plots,
                selected,
                state.farm.has_plots(),
                &state.farming.assignment,
                cx,
            );
            let detail = detail::detail_pane(state, &plots[selected], &snapshot, cx);
            let assignment = assignment::assignment_card(&state.farming.assignment, cx);
            let presets = presets::presets_card(state, cx);
            let allocation = allocation::allocation_card(state, cx);
            let comparison = comparison::comparison_card(state, cx);
            let body = div()
                .flex()
                .flex_col()
                .gap_3()
                .child(layout::split_pane(
                    "farming-body",
                    breakpoint,
                    field,
                    detail,
                    px(360.0),
                ))
                .child(assignment)
                .child(presets)
                .child(allocation)
                .child(comparison);
            ScrollArea::new("farming-scroll")
                .vertical()
                .flex_1()
                .min_h_0()
                .child(body)
                .into_any_element()
        }
    };

    div()
        .id("farming-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(tabs)
        .child(pane)
        .into_any_element()
}

/// The upstream tab strip, driven by the state's own tab.
fn tabs(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let mut strip =
        TabBar::new("farming-tabs").selected(SharedString::from(state.farming.tab.id()));
    for tab in Tab::ALL {
        strip = strip.tab(WindowTab::new(tab.id(), tab.label()));
    }
    strip
        .on_select(move |id, _window, cx| {
            if let Some(tab) = Tab::from_id(id.as_ref()) {
                entity.update(cx, |state, cx| state.set_farming_tab(tab, cx));
            }
        })
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
    snapshot: &Snapshot,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = &cx.theme().colors;
    let (accent, muted) = (colors.accent, colors.fg_muted);
    let points = state.farm.land_rank_points_left();
    let summary = format!(
        "{} plots · {} crops · {} growth · {} rank point(s) unspent",
        farming::PLOTS,
        format::number(f64::from(state.farm.crop_count)),
        format::number(snapshot.growth_rate),
        points
    );
    let entity = cx.entity();
    let target = f64::from(state.farming.target_crop);
    let objectives: Vec<Choice> = Objective::ALL
        .into_iter()
        .map(|objective| Choice::new(objective.id(), objective.label()))
        .collect();
    let objective_entity = entity.clone();
    let objective = Select::new("farming-objective", objectives)
        .label("Objective")
        .selected(state.farming.objective.id())
        .on_change(move |value, _window, cx| {
            if let Some(objective) = Objective::from_id(value.as_ref()) {
                objective_entity.update(cx, |state, cx| state.set_farming_objective(objective, cx));
            }
        });
    div()
        .id("farming-header")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(
            div()
                .id("farming-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .flex()
                .items_center()
                .gap_4()
                .child(Caption::new(format!("{} crops", state.farm.crop_count)).text_color(accent))
                .child(
                    Caption::new(format!(
                        "{} growth · {} unspent",
                        format::number(snapshot.growth_rate),
                        points
                    ))
                    .text_color(muted),
                ),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("farming-objective-field")
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new("Objective").text_color(muted))
                .child(div().w(px(170.0)).child(objective)),
        )
        .child(
            div()
                .id("farming-crop-selection")
                .role(gpui::Role::Group)
                .aria_label(format!("Crop selection: target crop {target:.0}"))
                .flex()
                .items_center()
                .gap_2()
                .child(Caption::new("Target crop").text_color(muted))
                .child(
                    Stepper::new("farming-target", target)
                        .range(1.0, 329.0)
                        .step(1.0)
                        .precision(0)
                        .on_change(move |value, _window, cx| {
                            entity.update(cx, |state, cx| state.set_target_crop(value, cx));
                        }),
                ),
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
    fn the_tab_strip_opens_on_the_overview_and_round_trips_its_ids() {
        let mut state = State::default();
        assert_eq!(state.tab, Tab::Overview);
        assert_eq!(
            Tab::ALL.map(Tab::label),
            ["Overview", "Full Plot Grid", "Evolution & OG"]
        );
        for tab in Tab::ALL {
            assert_eq!(Tab::from_id(tab.id()), Some(tab));
        }
        state.set_tab(Tab::Plots);
        assert_eq!(Tab::from_id("plots"), Some(state.tab));
        assert_eq!(Tab::from_id("rank"), None);
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
