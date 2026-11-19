//! The Coral God Rank section: the planner inputs, the current summary and
//! the beam-search plan.
//!
//! CR-01 landed the controls and the summary, CR-02 the shared task runner, the
//! projected coral state and the ordered action list, and CR-03 the offering
//! fit and the current-levels table with their missing-save warnings.

mod controls;
mod fit;
mod plan;
mod run;
mod summary;

pub(crate) use controls::Controls;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::v_flex;
use idleon_coral::{Goal, Inputs, SaveInputs};
use idleon_ui::widgets::PANEL_GAP;

/// One numeric planner field, so the controls can share one editor row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Field {
    Budget,
    MaxLevels,
    BeamSize,
    GodRank,
    Y4,
    DivCostAfter3,
    ReefUpgradeCount,
    MineFloor,
    Vault76Bonus,
    Tier,
    PlanningConfidence,
}

/// The section's state, kept alive in `AppState` while the window is open.
#[derive(Default)]
pub(crate) struct State {
    pub(crate) inputs: Inputs,
    /// The last imported save's values, so Reset can restore them.
    pub(crate) save: SaveInputs,
}

impl State {
    /// Take the imported save's values as the new defaults.
    pub(crate) fn adopt(&mut self, save: &SaveInputs) {
        self.save = save.clone();
        self.inputs = Inputs::from_save(save);
    }

    /// Put every input back to what the save carried.
    pub(crate) fn reset(&mut self) {
        self.inputs = Inputs::from_save(&self.save);
    }

    pub(crate) fn set_goal(&mut self, id: &str) {
        self.inputs.goal = Goal::from_id(id);
    }

    pub(crate) fn set_assume_unlocked(&mut self, on: bool) {
        self.inputs.assume_unlocked = on;
    }

    pub(crate) fn set(&mut self, field: Field, value: f64) {
        let value = if value.is_finite() {
            value.max(0.0)
        } else {
            0.0
        };
        match field {
            Field::Budget => self.inputs.budget = value,
            Field::MaxLevels => self.inputs.max_levels = value.round() as u32,
            Field::BeamSize => self.inputs.beam_size = value.round() as u32,
            Field::GodRank => self.inputs.god_rank = value.round() as u32,
            Field::Y4 => self.inputs.y4 = value,
            Field::DivCostAfter3 => self.inputs.div_cost_after3 = value,
            Field::ReefUpgradeCount => self.inputs.reef_upgrade_count = value.round(),
            Field::MineFloor => self.inputs.mine_floor = value.round(),
            Field::Vault76Bonus => self.inputs.vault76_bonus = value,
            Field::Tier => self.inputs.tier = value.round() as u32,
            Field::PlanningConfidence => self.inputs.planning_confidence = value,
        }
    }
}

/// The value `field` holds, as its number field shows it.
fn value(inputs: &Inputs, field: Field) -> f64 {
    match field {
        Field::Budget => inputs.budget,
        Field::MaxLevels => f64::from(inputs.max_levels),
        Field::BeamSize => f64::from(inputs.beam_size),
        Field::GodRank => f64::from(inputs.god_rank),
        Field::Y4 => inputs.y4,
        Field::DivCostAfter3 => inputs.div_cost_after3,
        Field::ReefUpgradeCount => inputs.reef_upgrade_count,
        Field::MineFloor => inputs.mine_floor,
        Field::Vault76Bonus => inputs.vault76_bonus,
        Field::Tier => f64::from(inputs.tier),
        Field::PlanningConfidence => inputs.planning_confidence,
    }
}

/// The section: the controls rail and the summary, plan and projection column.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let controls = div()
        .flex_1()
        .min_h_0()
        .child(controls::controls_pane(state, window, cx))
        .overflow_y_scrollbar()
        .id("coral-controls-scroll")
        .into_any_element();
    let summary = div()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(
            v_flex()
                .id("coral-summary-column")
                .gap(PANEL_GAP)
                .child(summary::kpis(state))
                .child(plan::plan_pane(state, cx))
                .child(fit::offering_pane(state, cx))
                .child(summary::current_pane(state, cx)),
        )
        .overflow_y_scrollbar()
        .id("coral-summary-scroll")
        .into_any_element();
    v_flex()
        .id("coral-god-rank")
        .size_full()
        .min_h_0()
        .child(layout::split_pane_reversed(
            "coral-body",
            Breakpoint::of(window, cx),
            summary,
            controls,
            px(420.0),
        ))
        .into_any_element()
}
