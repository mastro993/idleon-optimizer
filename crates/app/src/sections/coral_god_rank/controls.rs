//! Planner controls: every upstream input, its validation and the save's data gaps.

use super::Field;
use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Checkbox, Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, px};
use idleon_core::coral::Goal;

pub fn controls_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let entity = cx.entity();
    let inputs = &state.coral.inputs;
    let muted = cx.theme().colors.fg_muted;

    let goal_entity = entity.clone();
    let goal = Select::new(
        "coral-goal",
        Goal::ALL
            .into_iter()
            .map(|goal| Choice::new(goal.id(), goal.label())),
    )
    .label("Goal")
    .selected(inputs.goal.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        let goal_entity = goal_entity.clone();
        let value = value.to_string();
        goal_entity.update(cx, |state, cx| {
            state.coral.set_goal(&value);
            cx.notify();
        });
    });

    let budget_entity = entity.clone();
    let budget = NumberInput::new("coral-budget", inputs.budget)
        .label("Budget")
        .range(0.0, 1e24)
        .step(1000.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let budget_entity = budget_entity.clone();
            budget_entity.update(cx, |state, cx| {
                state.coral.set(Field::Budget, value);
                cx.notify();
            });
        });

    let assume_entity = entity.clone();
    let assume = Checkbox::new("coral-assume-unlocked", inputs.assume_unlocked)
        .label("Assume unlocked")
        .on_change(move |on, _window, cx| {
            let assume_entity = assume_entity.clone();
            assume_entity.update(cx, |state, cx| {
                state.coral.set_assume_unlocked(on);
                cx.notify();
            });
        });

    let reset_entity = entity.clone();
    let reset = Button::new("coral-reset", "Reset")
        .size(ControlSize::Sm)
        .on_click(move |_event, _window, cx| {
            let reset_entity = reset_entity.clone();
            reset_entity.update(cx, |state, cx| {
                state.coral.reset();
                cx.notify();
            });
        });

    let validation = inputs.validation();
    let (severity, message) = if !validation.errors.is_empty() {
        (Severity::Danger, validation.errors.join(" "))
    } else if !validation.warnings.is_empty() {
        (Severity::Warning, validation.warnings.join(" "))
    } else {
        (Severity::Success, "Inputs valid.".into())
    };
    let unlock = unlock_state(state);

    let mut card = Card::new()
        .header(
            CardHeader::new("Planner controls")
                .description("The upstream inputs for the Coral Kid and offering plan.")
                .action(reset),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(note("Goal", goal.into_any_element()))
                .child(note("Budget (Divinity points)", budget.into_any_element()))
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .w_full()
                        .gap_3()
                        .child(field(
                            entity.clone(),
                            "Max levels",
                            Field::MaxLevels,
                            inputs.max_levels as f64,
                            (0.0, 10_000.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Beam size",
                            Field::BeamSize,
                            inputs.beam_size as f64,
                            (1.0, 10_000.0),
                            10.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "God rank",
                            Field::GodRank,
                            inputs.god_rank as f64,
                            (0.0, 100.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Y4 %",
                            Field::Y4,
                            inputs.y4,
                            (0.0, 100.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "DivCostAfter3 %",
                            Field::DivCostAfter3,
                            inputs.div_cost_after3,
                            (0.0, 1_000.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Reef upgrades",
                            Field::ReefUpgradeCount,
                            inputs.reef_upgrade_count,
                            (0.0, 100_000.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Minehead floor",
                            Field::MineFloor,
                            inputs.mine_floor,
                            (0.0, 1_000.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Vault 76 bonus %",
                            Field::Vault76Bonus,
                            inputs.vault76_bonus,
                            (0.0, 100_000.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Offer tier",
                            Field::Tier,
                            inputs.tier as f64,
                            (0.0, 5.0),
                            1.0,
                            0,
                        ))
                        .child(field(
                            entity.clone(),
                            "Confidence",
                            Field::PlanningConfidence,
                            inputs.planning_confidence,
                            (0.1, 0.99),
                            0.01,
                            2,
                        )),
                )
                .child(
                    div().flex().flex_col().gap_2().child(assume).child(
                        div()
                            .id("coral-unlock-state")
                            .role(gpui::Role::Label)
                            .aria_label(unlock.clone())
                            .child(Caption::new(unlock).text_color(muted)),
                    ),
                )
                .child(Banner::new("coral-validation", severity, message)),
        );

    if !state.coral.save.missing.is_empty() {
        card = card.child(Banner::new(
            "coral-missing",
            Severity::Info,
            format!(
                "Missing from the save: {}.",
                state.coral.save.missing.join(", ")
            ),
        ));
    }

    div()
        .id("coral-controls")
        .flex()
        .flex_col()
        .gap_3()
        .child(card)
        .into_any_element()
}

/// A label drawn above its control.
fn note(label: &'static str, control: impl IntoElement) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label))
        .child(control)
}

/// One numeric planner field, wired to the section's state.
fn field(
    entity: Entity<AppState>,
    label: &'static str,
    field: Field,
    value: f64,
    (min, max): (f64, f64),
    step: f64,
    precision: usize,
) -> impl IntoElement {
    let control_entity = entity.clone();
    let control = NumberInput::new(format!("coral-{}", field_id(field)), value)
        .label(SharedString::from(label))
        .range(min, max)
        .step(step)
        .precision(precision)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let control_entity = control_entity.clone();
            control_entity.update(cx, |state, cx| {
                state.coral.set(field, value);
                cx.notify();
            });
        });
    div()
        .flex()
        .flex_col()
        .gap_1()
        .w(px(165.0))
        .flex_none()
        .child(Caption::new(label))
        .child(control)
}

fn field_id(field: Field) -> &'static str {
    match field {
        Field::Budget => "budget",
        Field::MaxLevels => "max-levels",
        Field::BeamSize => "beam-size",
        Field::GodRank => "god-rank",
        Field::Y4 => "y4",
        Field::DivCostAfter3 => "div-cost-after-3",
        Field::ReefUpgradeCount => "reef-upgrades",
        Field::MineFloor => "mine-floor",
        Field::Vault76Bonus => "vault-76",
        Field::Tier => "tier",
        Field::PlanningConfidence => "confidence",
    }
}

/// What the unlock inputs currently say, matching `_upgradeStatus`.
fn unlock_state(state: &AppState) -> String {
    let inputs = &state.coral.inputs;
    if inputs.assume_unlocked {
        return "Unlock state: all 6 upgrades assumed unlocked.".into();
    }
    match inputs.total_divinity_levels {
        Some(total) => format!(
            "Unlock state: {} of 6 from {} total Divinity levels.",
            inputs.unlocked_count(),
            format::count(u64::from(total))
        ),
        None => "Unlock state: unknown; tick Assume unlocked.".into(),
    }
}
