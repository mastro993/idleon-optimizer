//! FN-03: the Planner tab, the next-level ranking and the greedy optimizer.
//!
//! The planner measures one level of every accessible upgrade against the
//! current cavern and ranks the gain per unit cost; the optimizer buys the best
//! affordable step, then ranks again at the levels that purchase reached. The
//! optimizer runs through the shared [`idleon_ui::runner::TaskRunner`], so its
//! progress and cancel chrome match every other long job.

use super::controls::{cached, goal_choices, step_choices};
use super::overview::{num, odds, time};
use super::table::{data_height, number, table, tag, text};
use crate::sections::fountain::State;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{DataTable, TableRow};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, Goal, OptimizerPlan, PlanStep, PlannerRow};
use idleon_ui::widgets::{
    Caption, Card, CardHeader, Choice, ChoiceSelect, PANEL_GAP, RowColumn, RowsDelegate, Tone,
};

/// The Planner pane: the ranking table and the optimizer below it.
pub(super) fn planner_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    let request = state.fountain_state.planner_request();
    let rows = cached(
        &mut state.fountain_controls.planner,
        (state.save_revision, request),
        || fountain.planner(request),
    );
    v_flex()
        .id("fountain-planner")
        .gap(PANEL_GAP)
        .child(planner_card(state, fountain, &rows, window, cx))
        .child(optimizer_card(state, window, cx))
        .into_any_element()
}

/// The Upgrade Planner: the controls and the ranked next levels.
fn planner_card(
    state: &AppState,
    fountain: &Fountain,
    rows: &[PlannerRow],
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let selections = &state.fountain_state;
    let controls = &state.fountain_controls;
    let goal = selections.planner_goal;
    let per_hr = selections.planner_per_hr;
    let normalize = selections.planner_normalize;
    let mut row = h_flex().flex_wrap().gap_3().child(goal_select(
        "fountain-planner-goal",
        &controls.planner_goal,
        goal,
        window,
        cx,
    ));
    if goal == Goal::Optimal {
        row = row.child(desired_select(
            "fountain-planner-desired",
            &controls.planner_desired,
            selections.planner_desired,
            fountain.desired_currency(),
            window,
            cx,
        ));
    }
    let entity = cx.entity();
    row = row
        .child(flag(
            "fountain-planner-affordable",
            "Affordable only",
            selections.planner_affordable,
            false,
            entity.clone(),
            |state, on| state.planner_affordable = on,
        ))
        .child(flag(
            "fountain-planner-normalize",
            "Normalize cost by currency value",
            normalize,
            per_hr,
            entity.clone(),
            |state, on| state.planner_normalize = on,
        ))
        .child(flag(
            "fountain-planner-perhr",
            "Rank by /hr",
            per_hr,
            false,
            entity,
            |state, on| state.planner_per_hr = on,
        ));
    let body = if rows.is_empty() {
        div()
            .id("fountain-planner-empty")
            .child(Caption::new(
                "No upgrades affect this goal, or none are affordable.",
            ))
            .into_any_element()
    } else {
        planner_table(rows, goal)
    };

    Card::new()
        .header(
            CardHeader::new("Upgrade Planner")
                .description(format!(
                    "The next level of every accessible upgrade, ranked by {}",
                    if per_hr {
                        "the gain per hour needed to earn its cost"
                    } else if normalize {
                        "the gain per unit of the currency's own value"
                    } else {
                        "the gain per unit of cost"
                    }
                ))
                .action(Caption::new(format!("{} candidates", rows.len())).text_color(muted)),
        )
        .child(
            v_flex()
                .id("fountain-planner-controls-card")
                .gap_3()
                .child(row)
                .child(body),
        )
        .into_any_element()
}

/// The ranking table, with the goal's own column as one detail line.
fn planner_table(rows: &[PlannerRow], goal: Goal) -> AnyElement {
    let top = format!(
        "Top pick: {} ({} Water)",
        rows[0].name,
        fountain::water_names()[rows[0].water]
    );
    let table = table(
        &[
            ("#", true),
            ("Water", false),
            ("Upgrade", false),
            ("Level", true),
            ("Cost", true),
            ("Time to Afford", true),
            ("Gain", true),
            ("% Gain", true),
            (goal_detail_label(goal), false),
            ("Efficiency", true),
            ("Affordable", false),
        ],
        rows.iter().take(30).enumerate().map(|(rank, row)| {
            TableRow::new()
                .id(("fountain-planner-row", rank))
                .child(number((rank + 1).to_string()))
                .child(text(fountain::water_names()[row.water].clone()))
                .child(text(row.name))
                .child(number(format!(
                    "{} → {}",
                    num(row.level),
                    num(row.level + 1.0)
                )))
                .child(number(format!(
                    "{} {}",
                    num(row.cost),
                    fountain::currency_label(row.currency)
                )))
                .child(number(eta(row.seconds_to_afford)))
                .child(number(format!("+{}", num(row.delta))))
                .child(number(format!("+{:.3}%", row.pct_gain)))
                .child(text(goal_detail(row, goal)))
                .child(number(format!("{:.3e}", row.efficiency)))
                .child(text(affordable(row.affordable)))
        }),
    );
    v_flex()
        .id("fountain-planner-table")
        .gap_2()
        .child(table)
        .child(
            div()
                .id("fountain-planner-top")
                .role(gpui::Role::Label)
                .aria_label(top.clone())
                .child(Caption::new(top)),
        )
        .child(Caption::new(
            "The top thirty rows. Every number is measured one level above the saved cavern, so a \
             capped effect and a locked prerequisite read exactly as the Upgrades tab shows them.",
        ))
        .into_any_element()
}

/// The goal's extra column header.
pub(super) fn goal_detail_label(goal: Goal) -> &'static str {
    match goal {
        Goal::Fill => "Coin / Marble / Duck Δ",
        Goal::Royal => "Royal Δ",
        Goal::Duck => "Duck Δ",
        Goal::Lucky => "Lucky Δ",
        Goal::Optimal => "Speed / Value Δ",
        _ => "Goal Δ",
    }
}

/// The goal's own reading for one candidate.
fn goal_detail(row: &PlannerRow, goal: Goal) -> String {
    match goal {
        Goal::Fill => {
            let mut parts = vec![format!("-{} coin", time(row.fill_delta))];
            if row.marble_fill_delta > 0.0005 {
                parts.push(format!("-{} marble", time(row.marble_fill_delta)));
            }
            if let Some(delta) = row.duck_fill_delta.filter(|delta| *delta > 0.0005) {
                parts.push(format!("-{} duck", time(delta)));
            }
            parts.join(" · ")
        }
        Goal::Royal => format!(
            "{} → {}",
            odds(row.royal_chance - row.royal_delta),
            odds(row.royal_chance)
        ),
        Goal::Duck => format!(
            "{:.3}% → {:.3}%",
            100.0 * (row.duck_chance - row.duck_delta),
            100.0 * row.duck_chance
        ),
        Goal::Lucky => format!("{:+.4}%", 100.0 * row.lucky_delta),
        Goal::Optimal => format!(
            "Speed {:+.1}% · Value {:+.1}%",
            row.speed_pct, row.value_pct
        ),
        _ => "—".to_owned(),
    }
}

/// The Upgrade Optimizer: its controls, the run chrome and the stored plan.
fn optimizer_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let runner = state.fountain_plan.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = runner.is_stale(&state.fountain_state.plan_request());
    // The stored plan drawn is the one the runner still holds.
    let plan = runner.result().and(state.fountain_controls.plan.clone());
    let selections = &state.fountain_state;
    let controls = &state.fountain_controls;
    let per_hr = selections.optimizer_per_hr;
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let run_entity = entity.clone();
    let cancel_entity = entity.clone();
    let run = Button::new("run-fountain-plan")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .icon(IconName::Play)
        .disabled(running || !state.save_loaded)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_fountain_plan(cx));
        });
    let cancel = Button::new("cancel-fountain-plan")
        .label("Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_plan(cx));
        });
    let row = h_flex()
        .id("fountain-optimizer-controls")
        .flex_wrap()
        .gap_3()
        .child(goal_select(
            "fountain-optimizer-goal",
            &controls.optimizer_goal,
            selections.optimizer_goal,
            window,
            cx,
        ))
        .child(steps_select(
            "fountain-optimizer-steps",
            &controls.optimizer_steps,
            selections.optimizer_steps,
            window,
            cx,
        ))
        .child(flag(
            "fountain-optimizer-affordable",
            "Affordable only",
            selections.optimizer_affordable,
            false,
            entity.clone(),
            |state, on| state.optimizer_affordable = on,
        ))
        .child(flag(
            "fountain-optimizer-normalize",
            "Normalize cost by currency value",
            selections.optimizer_normalize,
            per_hr,
            entity.clone(),
            |state, on| state.optimizer_normalize = on,
        ))
        .child(flag(
            "fountain-optimizer-perhr",
            "Rank by /hr",
            per_hr,
            false,
            entity.clone(),
            |state, on| state.optimizer_per_hr = on,
        ))
        .child(run)
        .child(cancel);
    let mut body = v_flex().id("fountain-optimizer").gap_3().child(row).child(
        Caption::new(
            "Each purchase is the best gain per unit cost at the levels the plan has reached, \
                 paid from the saved opal balance. Affordable only skips a step the balance cannot \
                 cover; otherwise the plan waits for the currency's own income and adds that wait \
                 to the cumulative ETA.",
        )
        .text_color(muted),
    );
    if running && let Some(progress) = progress {
        body = body.child(idleon_ui::run_chrome::progress_card(
            "fountain-plan",
            "Planning the upgrades",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_fountain_plan(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-plan-cancelled",
            plan.is_some(),
        ));
    }
    if let Some(plan) = plan {
        body = body.child(plan_result(state, &plan, stale));
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade Optimizer")
                .description("Greedily buys the best upgrade for the selected goal."),
        )
        .child(body)
        .into_any_element()
}

/// One optimizer step, formatted once per stored plan.
pub(crate) struct StepRow {
    index: SharedString,
    water: SharedString,
    name: SharedString,
    level: SharedString,
    cost: SharedString,
    gain: SharedString,
    eta: SharedString,
}

/// The step list's columns.
pub(crate) fn step_table() -> RowsDelegate<StepRow> {
    RowsDelegate::new(vec![
        RowColumn::number("index", "#", |row: &StepRow| row.index.clone())
            .column(|column| column.width(px(48.0))),
        RowColumn::text("water", "Water", |row: &StepRow| row.water.clone()),
        RowColumn::text("upgrade", "Upgrade", |row: &StepRow| row.name.clone()),
        RowColumn::number("level", "Level", |row: &StepRow| row.level.clone()),
        RowColumn::number("cost", "Cost", |row: &StepRow| row.cost.clone()),
        RowColumn::number("gain", "Gain", |row: &StepRow| row.gain.clone()),
        RowColumn::number("eta", "Cumulative ETA", |row: &StepRow| row.eta.clone()),
    ])
}

/// Every step of a plan, in the order the optimizer buys it.
pub(crate) fn step_rows(plan: &OptimizerPlan) -> Vec<StepRow> {
    plan.steps
        .iter()
        .enumerate()
        .map(|(index, step): (usize, &PlanStep)| StepRow {
            index: (index + 1).to_string().into(),
            water: fountain::water_names()[step.water].clone().into(),
            name: step.name.into(),
            level: num(step.level).into(),
            cost: format!(
                "{} {}",
                num(step.cost),
                fountain::currency_label(step.currency)
            )
            .into(),
            gain: format!("+{:.3}%", step.pct_gain).into(),
            eta: eta_text(step.cumulative_seconds).into(),
        })
        .collect()
}

/// The stored plan: its summary, its purchases and its individual steps.
fn plan_result(state: &AppState, plan: &OptimizerPlan, stale: bool) -> AnyElement {
    let mut header = CardHeader::new("Plan").description(format!(
        "{} steps · {} → {} ({:+.1}%)",
        plan.steps.len(),
        num(plan.start_value),
        num(plan.end_value),
        plan.gain_pct
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("fountain-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("fountain-plan"));
    }
    if plan.steps.is_empty() {
        return card
            .child(
                div()
                    .id("fountain-plan-empty")
                    .child(Caption::new("No affordable upgrades affect this goal.")),
            )
            .into_any_element();
    }
    let remaining: Vec<String> = plan
        .remaining
        .iter()
        .enumerate()
        .filter(|(_, left)| **left > 0.0)
        .map(|(currency, left)| format!("{}: {}", fountain::currency_label(currency), num(*left)))
        .collect();
    let summary = h_flex()
        .id("fountain-plan-result")
        .flex_wrap()
        .gap_3()
        .child(tag(format!("{} steps", plan.steps.len()), Tone::Info))
        .child(Caption::new(format!(
            "Value {} → {} ({:+.1}%)",
            num(plan.start_value),
            num(plan.end_value),
            plan.gain_pct
        )));
    let groups = table(
        &[
            ("Water", false),
            ("Upgrade", false),
            ("Levels", true),
            ("From", true),
            ("To", true),
            ("Total Cost", true),
            ("Cumulative ETA", true),
        ],
        plan.groups.iter().enumerate().map(|(index, group)| {
            TableRow::new()
                .id(("fountain-plan-group", index))
                .child(text(fountain::water_names()[group.water].clone()))
                .child(text(group.name))
                .child(number(group.count.to_string()))
                .child(number(num(group.start_level)))
                .child(number(num(group.end_level)))
                .child(number(format!(
                    "{} {}",
                    num(group.total_cost),
                    fountain::currency_label(group.currency)
                )))
                .child(number(eta(group.cumulative_seconds)))
        }),
    );
    card.child(
        v_flex()
            .gap_3()
            .child(summary)
            .child(
                v_flex()
                    .id("fountain-plan-steps")
                    .gap_2()
                    .child(Caption::new("Purchases, grouped by upgrade"))
                    .child(groups),
            )
            .child(
                v_flex()
                    .id("fountain-plan-step-list")
                    .gap_2()
                    .child(Caption::new(
                        "Every step, in the order the optimizer buys it",
                    ))
                    .child(
                        div()
                            .h(data_height(plan.steps.len()))
                            .child(DataTable::new(&state.fountain_controls.plan_steps)),
                    ),
            )
            .child(
                div()
                    .id("fountain-plan-remaining")
                    .child(Caption::new(format!(
                        "Remaining: {}",
                        remaining.join(" • ")
                    ))),
            ),
    )
    .into_any_element()
}

/// A labelled select under the element id the harnesses focus.
fn labeled<V: Clone + PartialEq + 'static>(
    id: &'static str,
    label: &'static str,
    select: &ChoiceSelect<V>,
    width: f32,
) -> impl IntoElement {
    h_flex().gap_2().child(Caption::new(label)).child(
        div().w(px(width)).child(
            Select::new(select.state())
                .id(id)
                .accessibility_label(label)
                .small(),
        ),
    )
}

/// A goal selector shared by the planners and the optimizers.
pub(super) fn goal_select(
    id: &'static str,
    select: &ChoiceSelect<Goal>,
    selected: Goal,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    select.show(goal_choices(), Some(&selected), window, cx);
    labeled(id, "Goal", select, 170.0)
}

/// An optimizer's step cap.
pub(super) fn steps_select(
    id: &'static str,
    select: &ChoiceSelect<usize>,
    selected: usize,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    select.show(step_choices(), Some(&selected), window, cx);
    labeled(id, "Max steps", select, 90.0)
}

/// The `optimal` goal's simulated desired currency.
pub(super) fn desired_select(
    id: &'static str,
    select: &ChoiceSelect<Option<usize>>,
    selected: Option<usize>,
    saved: usize,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let choices = std::iter::once(Choice::new(
        None,
        format!("Save ({})", fountain::currency_label(saved)),
    ))
    .chain(
        (0..fountain::CURRENCIES)
            .map(|currency| Choice::new(Some(currency), fountain::currency_name(currency))),
    )
    .collect();
    select.show(choices, Some(&selected), window, cx);
    labeled(id, "Currency", select, 150.0)
}

/// One boolean planner or optimizer control.
pub(super) fn flag(
    id: &'static str,
    label: &'static str,
    checked: bool,
    disabled: bool,
    entity: gpui::Entity<AppState>,
    edit: fn(&mut State, bool),
) -> Checkbox {
    Checkbox::new(id)
        .label(label)
        .checked(checked)
        .disabled(disabled)
        .on_click(move |on, _window, cx| {
            let on = *on;
            entity.update(cx, |state, cx| {
                state.edit_fountain_state(cx, |state| edit(state, on));
            });
        })
}

/// Yes or No, by whether the saved balance covers a candidate.
pub(super) fn affordable(affordable: bool) -> impl IntoElement {
    if affordable {
        tag("Yes", Tone::Success)
    } else {
        tag("No", Tone::Warning)
    }
}

/// A wait: Now, a duration or infinity, as the time to afford a level or a
/// plan's cumulative ETA.
pub(super) fn eta(seconds: f64) -> AnyElement {
    if seconds <= 0.0 {
        return div()
            .flex()
            .justify_end()
            .child(tag("Now", Tone::Success))
            .into_any_element();
    }
    div().child(eta_text(seconds)).into_any_element()
}

/// A cumulative plan ETA as plain text.
fn eta_text(seconds: f64) -> String {
    if seconds <= 0.0 {
        "Now".to_owned()
    } else if seconds.is_finite() {
        time(seconds)
    } else {
        "∞".to_owned()
    }
}
