//! The Farming Planner section: the 9×4 field and the selected plot's detail.
//!
//! FM-01 ports the upstream field tab: every plot renders from
//! `farmPlotData`, the right rail reads the selected plot's saved row and
//! formulas, and the selection and crop target stay in `AppState` across
//! dashboard visits. Allocation, presets, evolution and the signal cards are
//! later features in the same milestone.

use crate::state::AppState;
use crate::ui::format;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::farming::{self, PlotMetrics, Snapshot};

mod detail;
mod field;

/// The field's own state: the selected plot and the crop the later evolution
/// features aim at, both kept alive across dashboard visits.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct State {
    pub selected_plot: usize,
    pub target_crop: u32,
}

impl Default for State {
    fn default() -> Self {
        Self {
            selected_plot: 0,
            target_crop: 1,
        }
    }
}

impl State {
    /// Select a plot, clamped to the field.
    pub fn select(&mut self, plot: usize) {
        self.selected_plot = plot.min(farming::PLOTS - 1);
    }

    /// Aim at a crop id, clamped to the last seed family.
    pub fn set_target_crop(&mut self, value: f64) {
        let max = farming::data()
            .seeds
            .last()
            .map(|seed| seed.end as u32)
            .unwrap_or(1);
        self.target_crop = value.round().clamp(1.0, f64::from(max)) as u32;
    }
}

/// The section: a summary line, the field, and the selected plot's detail.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let breakpoint = Breakpoint::of(window);
    let snapshot = state.farm.snapshot();
    let plots: Vec<PlotMetrics> = (0..farming::PLOTS)
        .map(|index| state.farm.plot_metrics_with(index, &snapshot))
        .collect();
    let selected = state.farming.selected_plot.min(farming::PLOTS - 1);
    let header = section_header(state, &snapshot, cx);
    let field = field::field_card(&plots, selected, state.farm.has_plots(), cx);
    let detail = detail::detail_pane(state, &plots[selected], &snapshot, cx);

    div()
        .id("farming-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header)
        .child(layout::split_pane(
            "farming-body",
            breakpoint,
            field,
            detail,
            px(360.0),
        ))
        .into_any_element()
}

/// The crops, growth rate, points and crop-selection stepper above the field.
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
    div()
        .id("farming-header")
        .flex()
        .flex_none()
        .items_center()
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
}
