//! The plan card, its projection table and the action list.
//!
//! Every figure comes from [`idleon_core::coral::Plan`]; the shared task runner
//! owns the run, its progress and the stale marker.

use crate::state::AppState;
use crate::task::TaskState;
use crate::ui::format;
use crate::ui::task;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_core::coral::{Action, Plan, Projection};

/// The plan card, its projection and its actions, stacked.
pub fn plan_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.coral_task(cx);
    let runnable = state.coral_plan_inputs().is_ok();
    div()
        .id("coral-plan")
        .flex()
        .flex_col()
        .gap_3()
        .child(plan_card(state, &task, runnable, cx))
        .children(task.result.as_ref().map(|plan| {
            div()
                .flex()
                .flex_col()
                .gap_3()
                .child(projection_card(plan, &task))
                .child(actions_card(plan, &task, cx))
        }))
        .into_any_element()
}

/// The run button, its budget summary and the plan's assumptions.
fn plan_card(
    state: &mut AppState,
    task: &TaskState<Plan>,
    runnable: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let run_entity = entity.clone();
    let running = task.running;
    let run = Button::new("coral-run", "Run plan")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .disabled(running || !runnable)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_coral(cx));
        });
    let inputs = &state.coral.inputs;
    let muted = cx.theme().colors.fg_muted;
    let (budget, budget_unit) = format::scaled(inputs.budget);
    let scope = format!(
        "Max {} levels · beam {} · budget {}{}",
        inputs.max_levels,
        inputs.beam_size,
        format::value(budget),
        budget_unit
    );

    let mut body = div()
        .id("coral-plan-body")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_center()
                .gap_3()
                .child(run)
                .child(Caption::new(scope).text_color(muted)),
        );
    if let Some(plan) = &task.result {
        body = body.child(summary_row(plan));
    }
    if let Some(progress) = &task.progress {
        let cancel_entity = entity.clone();
        body = body.child(task::progress_card(
            "coral-run",
            "Planning upgrades",
            "The search keeps the best beam and reports each step.",
            progress.clone(),
            move |_window, cx| {
                cancel_entity.update(cx, |state, cx| state.cancel_coral(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(task::cancelled_banner(
            "coral-cancelled",
            task.result.is_some(),
        ));
    }
    if task.result.is_none() && !task.running {
        body = body.child(Caption::new(
            "Run the planner to project the current coral state and list the actions.",
        ));
    }
    if task.stale {
        body = body.child(task::stale_banner("coral-stale"));
    }
    if let Some(plan) = &task.result
        && !plan.partial.is_empty()
    {
        body = body.child(Banner::new(
            "coral-partial",
            Severity::Warning,
            format!("Assumed inputs: {}.", plan.partial.join(", ")),
        ));
    }

    Card::new()
        .header(CardHeader::new("Beam-search plan").description(
            "A bounded deterministic search over Coral Kid levels and God Rank offerings.",
        ))
        .child(body)
        .into_any_element()
}

/// What the plan commits: budget, remaining and the whole-plan confidence.
fn summary_row(plan: &Plan) -> AnyElement {
    let (spent, spent_unit) = format::scaled(plan.spent);
    let (remaining, remaining_unit) = format::scaled(plan.remaining());
    let summary = format!(
        "Spent {}{} · remaining {}{} · {} actions · whole-plan confidence {:.1}%",
        format::value(spent),
        spent_unit,
        format::value(remaining),
        remaining_unit,
        plan.actions.len(),
        plan.whole_plan_confidence * 100.0
    );
    div()
        .id("coral-plan-summary")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("kpi-coral-spent", "Spent", spent)
                    .decimals(2)
                    .suffix(spent_unit),
            )
            .caption("Worst case committed"),
        )
        .child(
            KpiCard::new(
                Statistic::new("kpi-coral-remaining", "Remaining", remaining)
                    .decimals(2)
                    .suffix(remaining_unit),
            )
            .caption("Budget left"),
        )
        .child(
            KpiCard::new(Statistic::new(
                "kpi-coral-actions",
                "Actions",
                plan.actions.len() as f64,
            ))
            .caption(format!("Goal: {}", plan.goal.label())),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "kpi-coral-confidence",
                    "Confidence",
                    plan.whole_plan_confidence * 100.0,
                )
                .decimals(1)
                .suffix("%"),
            )
            .caption("Whole-plan"),
        )
        .into_any_element()
}

/// The current against projected coral state.
fn projection_card(plan: &Plan, task: &TaskState<Plan>) -> AnyElement {
    let mut table = Table::new(["Metric", "Current", "Projected", "Δ"]);
    for row in plan.projection() {
        table = table.row([
            row.key.label().to_string(),
            metric(row, row.current),
            metric(row, row.projected),
            delta(row),
        ]);
    }
    Card::new()
        .header(
            CardHeader::new("Projected coral state").description(format!(
                "The save's current state against the plan's best state ({} levels, rank {}).",
                plan.best.levels.iter().sum::<u32>(),
                plan.best.god_rank
            )),
        )
        .when(task.stale, |card| {
            card.child(
                div()
                    .px_3()
                    .pt_3()
                    .child(task::stale_badge("coral-projection")),
            )
        })
        .child(div().id("coral-projection").p_3().child(table))
        .into_any_element()
}

/// The ordered Coral Kid levels and God Rank offerings the plan buys.
fn actions_card(plan: &Plan, task: &TaskState<Plan>, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let mut table = Table::new(["Type", "Target", "Qty", "Cost"]);
    for action in &plan.actions {
        table = table.row(match action {
            Action::CoralKid { index, level, cost } => [
                "Coral Kid".to_string(),
                format!("Upgrade {index} → level {level}"),
                "1".to_string(),
                format::value(*cost),
            ],
            Action::GodOffering {
                from_rank,
                to_rank,
                attempts,
                max_paid_spend,
                ..
            } => [
                "God offering".to_string(),
                format!("Rank {from_rank} → {to_rank}"),
                attempts.to_string(),
                format::value(*max_paid_spend),
            ],
        });
    }
    let mut body = div().flex().flex_col().gap_2().p_3();
    if plan.actions.is_empty() {
        body = body.child(Caption::new(
            "No action fits the budget and level cap; the plan holds the current state.",
        ));
    } else {
        body = body.child(table).child(
            Caption::new(
                "Offering costs are the worst case: every attempt paid. Level costs are Divinity points.",
            )
            .text_color(muted),
        );
    }
    Card::new()
        .header(CardHeader::new("Plan actions").description("In the order the plan buys them."))
        .when(task.stale, |card| {
            card.child(
                div()
                    .px_3()
                    .pt_3()
                    .child(task::stale_badge("coral-actions")),
            )
        })
        .child(body.id("coral-actions"))
        .into_any_element()
}

/// A multiplier with its `×` suffix, a count as itself.
fn metric(row: Projection, value: f64) -> String {
    if row.key.is_multiplier() {
        format!("{}×", format::value(value))
    } else {
        format::value(value)
    }
}

/// The change the plan makes: percent for multipliers, absolute for a count.
fn delta(row: Projection) -> String {
    if row.key.is_multiplier() {
        format!("{:+.1}%", row.ratio() * 100.0)
    } else {
        let change = row.delta();
        let sign = if change >= 0.0 { "+" } else { "-" };
        format!("{sign}{}", format::value(change.abs()))
    }
}
