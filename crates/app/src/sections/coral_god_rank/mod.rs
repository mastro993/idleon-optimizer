//! The Coral God Rank section: upstream planner inputs, the current summary and
//! the beam-search plan.
//!
//! CR-01 landed the controls and the summary; this issue adds the shared task
//! runner, the projected coral state and the ordered action list. The offering
//! fit and its explanation are the next feature issue on the same plan.

mod controls;
mod plan;
mod run;
mod summary;

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::layout::ScrollArea;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::coral::{Goal, Inputs, SaveInputs};

/// One numeric planner field, so the controls can share one editor row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field {
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
pub struct State {
    pub inputs: Inputs,
    /// The last imported save's values, so Reset can restore them.
    pub save: SaveInputs,
}

impl State {
    /// Take the imported save's values as the new defaults.
    pub fn adopt(&mut self, save: &SaveInputs) {
        self.save = save.clone();
        self.inputs = Inputs::from_save(save);
    }

    /// Put every input back to what the save carried.
    pub fn reset(&mut self) {
        self.inputs = Inputs::from_save(&self.save);
    }

    pub fn set_goal(&mut self, id: &str) {
        self.inputs.goal = Goal::from_id(id);
    }

    pub fn set_assume_unlocked(&mut self, on: bool) {
        self.inputs.assume_unlocked = on;
    }

    pub fn set(&mut self, field: Field, value: f64) {
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

/// The section: the controls rail and the summary, plan and projection column.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let controls = ScrollArea::new("coral-controls-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(controls::controls_pane(state, window, cx))
        .into_any_element();
    let summary = ScrollArea::new("coral-summary-scroll")
        .vertical()
        .flex_1()
        .min_w_0()
        .min_h_0()
        .child(
            div()
                .id("coral-summary-column")
                .flex()
                .flex_col()
                .gap_3()
                .child(summary::kpis(state))
                .child(plan::plan_pane(state, cx))
                .child(summary::current_pane(state, window, cx)),
        )
        .into_any_element();
    div()
        .id("coral-god-rank")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .child(layout::split_pane_reversed(
            "coral-body",
            Breakpoint::of(window),
            summary,
            controls,
            px(420.0),
        ))
        .into_any_element()
}
