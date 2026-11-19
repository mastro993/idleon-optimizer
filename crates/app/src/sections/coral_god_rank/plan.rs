//! The plan card, its projection table and the action list.
//!
//! Every figure comes from [`idleon_coral::Plan`]; the shared task runner
//! owns the run, its progress and the stale marker. The plan's actions can run
//! to thousands with a large level cap, so they sit in a virtualized table
//! whose rows are built once per stored plan (`docs/ui-kit/data.md`).

use super::summary::{number_cell, table};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::table::{DataTable, TableCell, TableRow};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_coral::{Action, Plan, Projection};
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, PANEL_GAP, RowColumn, RowsDelegate, Statistic, Tone,
};

/// The height of one action row and the most rows the table shows before it
/// scrolls.
const ROW_HEIGHT: f32 = 33.0;
const VISIBLE_ACTIONS: usize = 12;

/// One plan action, formatted once per stored plan.
pub(crate) struct ActionRow {
    kind: SharedString,
    target: SharedString,
    quantity: SharedString,
    cost: SharedString,
}

/// The action table's columns.
pub(crate) fn action_table() -> RowsDelegate<ActionRow> {
    RowsDelegate::new(vec![
        RowColumn::text("type", "Type", |row: &ActionRow| row.kind.clone())
            .column(|column| column.width(px(160.0))),
        RowColumn::text("target", "Target", |row: &ActionRow| row.target.clone())
            .column(|column| column.width(px(280.0))),
        RowColumn::number("qty", "Qty", |row: &ActionRow| row.quantity.clone())
            .column(|column| column.width(px(80.0))),
        RowColumn::number("cost", "Cost", |row: &ActionRow| row.cost.clone())
            .column(|column| column.width(px(150.0))),
    ])
}

/// The plan's actions as table rows, in the order the plan buys them.
pub(crate) fn action_rows(plan: &Plan) -> Vec<ActionRow> {
    plan.actions
        .iter()
        .map(|action| match action {
            Action::CoralKid { index, level, cost } => ActionRow {
                kind: "Coral Kid".into(),
                target: format!("Upgrade {index} → level {level}").into(),
                quantity: "1".into(),
                cost: format::value(*cost).into(),
            },
            Action::GodOffering {
                from_rank,
                to_rank,
                attempts,
                max_paid_spend,
                ..
            } => ActionRow {
                kind: "God offering".into(),
                target: format!("Rank {from_rank} → {to_rank}").into(),
                quantity: attempts.to_string().into(),
                cost: format::value(*max_paid_spend).into(),
            },
        })
        .collect()
}

/// The plan card, its projection and its actions, stacked.
pub(crate) fn plan_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let runner = state.coral_run.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = runner.is_stale(&state.coral.inputs);
    // The stored plan drawn is the one the runner still holds.
    let plan = runner.result().and(state.coral_controls.plan.clone());
    let runnable = state.coral_plan_inputs().is_ok();
    let flags = Flags {
        running,
        cancelled,
        stale,
    };
    v_flex()
        .id("coral-plan")
        .gap(PANEL_GAP)
        .child(plan_card(
            state,
            plan.as_deref(),
            progress,
            flags,
            runnable,
            cx,
        ))
        .children(plan.map(|plan| {
            v_flex()
                .gap(PANEL_GAP)
                .child(projection_card(&plan, stale))
                .child(actions_card(state, &plan, stale, cx))
        }))
        .into_any_element()
}

/// What the runner says about the run besides its result.
#[derive(Clone, Copy)]
struct Flags {
    running: bool,
    cancelled: bool,
    stale: bool,
}

/// The run button, its budget summary and the plan's assumptions.
fn plan_card(
    state: &AppState,
    plan: Option<&Plan>,
    progress: Option<idleon_common::Progress>,
    flags: Flags,
    runnable: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let entity = cx.entity();
    let run_entity = entity.clone();
    let run = Button::new("coral-run")
        .label("Run plan")
        .primary()
        .small()
        .disabled(flags.running || !runnable)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_coral(cx));
        });
    let inputs = &state.coral.inputs;
    let muted = cx.theme().muted_foreground;
    let (budget, budget_unit) = format::scaled(inputs.budget);
    let scope = format!(
        "Max {} levels · beam {} · budget {}{}",
        inputs.max_levels,
        inputs.beam_size,
        format::value(budget),
        budget_unit
    );

    let mut body = v_flex().id("coral-plan-body").gap_3().child(
        h_flex()
            .flex_wrap()
            .gap_3()
            .child(run)
            .child(Caption::new(scope).text_color(muted)),
    );
    if let Some(plan) = plan {
        body = body.child(summary_row(plan));
    }
    if let Some(progress) = progress {
        body = body.child(run_chrome::progress_card(
            "coral-run",
            "Planning upgrades",
            "The search keeps the best beam and reports each step.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_coral(cx));
            },
        ));
    }
    if flags.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "coral-cancelled",
            plan.is_some(),
        ));
    }
    if plan.is_none() && !flags.running {
        body = body.child(Caption::new(
            "Run the planner to project the current coral state and list the actions.",
        ));
    }
    if flags.stale {
        body = body.child(run_chrome::stale_banner("coral-stale"));
    }
    if let Some(plan) = plan
        && !plan.partial.is_empty()
    {
        body = body.child(
            Alert::new(
                "coral-partial",
                format!("Assumed inputs: {}.", plan.partial.join(", ")),
            )
            .with_variant(Tone::Warning.into())
            .banner(),
        );
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
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "kpi-coral-spent",
                        "Spent",
                        format!("{spent:.2}{spent_unit}"),
                    )
                    .caption("Worst case committed"),
                )
                .child(
                    Statistic::new(
                        "kpi-coral-remaining",
                        "Remaining",
                        format!("{remaining:.2}{remaining_unit}"),
                    )
                    .caption("Budget left"),
                )
                .child(
                    Statistic::new(
                        "kpi-coral-actions",
                        "Actions",
                        format::count(plan.actions.len() as u64),
                    )
                    .caption(format!("Goal: {}", plan.goal.label())),
                )
                .child(
                    Statistic::new(
                        "kpi-coral-confidence",
                        "Confidence",
                        format!("{:.1}%", plan.whole_plan_confidence * 100.0),
                    )
                    .caption("Whole-plan"),
                ),
        )
        .into_any_element()
}

/// The current against projected coral state.
fn projection_card(plan: &Plan, stale: bool) -> AnyElement {
    let rows = plan
        .projection()
        .into_iter()
        .enumerate()
        .map(|(index, row)| {
            TableRow::new()
                .id(("coral-projection-row", index))
                .child(TableCell::new().child(row.key.label()))
                .child(number_cell(metric(row, row.current)))
                .child(number_cell(metric(row, row.projected)))
                .child(number_cell(delta(row)))
        });
    Card::new()
        .header(
            CardHeader::new("Projected coral state").description(format!(
                "The save's current state against the plan's best state ({} levels, rank {}).",
                plan.best.levels.iter().sum::<u32>(),
                plan.best.god_rank
            )),
        )
        .when(stale, |card| {
            card.child(run_chrome::stale_badge("coral-projection"))
        })
        .child(div().id("coral-projection").child(table(
            &[
                ("Metric", false),
                ("Current", true),
                ("Projected", true),
                ("Δ", true),
            ],
            rows,
        )))
        .into_any_element()
}

/// The ordered Coral Kid levels and God Rank offerings the plan buys.
fn actions_card(
    state: &AppState,
    plan: &Plan,
    stale: bool,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let mut body = v_flex().gap_2();
    if plan.actions.is_empty() {
        body = body.child(Caption::new(
            "No action fits the budget and level cap; the plan holds the current state.",
        ));
    } else {
        // Sized to its rows up to a dozen, then the table scrolls.
        let shown = plan.actions.len().min(VISIBLE_ACTIONS) + 1;
        body = body
            .child(
                div()
                    .h(px(ROW_HEIGHT * shown as f32 + 2.0))
                    .child(DataTable::new(&state.coral_controls.actions)),
            )
            .child(
                Caption::new(
                    "Offering costs are the worst case: every attempt paid. Level costs are Divinity points.",
                )
                .text_color(muted),
            );
    }
    Card::new()
        .header(CardHeader::new("Plan actions").description("In the order the plan buys them."))
        .when(stale, |card| {
            card.child(run_chrome::stale_badge("coral-actions"))
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
