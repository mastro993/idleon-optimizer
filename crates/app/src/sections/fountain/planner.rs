//! FN-03: the Planner tab, the next-level ranking and the greedy optimizer.
//!
//! Ported from the `#tab-planner` half of `fountain-calc.html`. The planner
//! measures one level of every accessible upgrade against the current cavern
//! and ranks the gain per unit cost; the optimizer buys the best affordable
//! step, then ranks again at the levels that purchase reached. The optimizer
//! runs through the shared [`idleon_ui::runner::TaskRunner`], so its progress
//! and cancel chrome match every other long job.

use super::overview::{num, odds, time};
use crate::sections::fountain::{STEP_CHOICES, State};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Checkbox, Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, IntoElement, ParentElement, Styled, div};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, Goal, OptimizerPlan, PlannerRow};

/// The Planner pane: the ranking table and the optimizer below it.
pub(super) fn planner_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(fountain) = state.fountain.clone() else {
        return div().into_any_element();
    };
    let rows = fountain.planner(state.fountain_state.planner_request());
    div()
        .id("fountain-planner")
        .flex()
        .flex_col()
        .gap_3()
        .child(planner_card(&fountain, &rows, state, cx))
        .child(optimizer_card(state, cx))
        .into_any_element()
}

/// The Upgrade Planner: the controls and the ranked next levels.
fn planner_card(
    fountain: &Fountain,
    rows: &[PlannerRow],
    state: &mut AppState,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let goal = state.fountain_state.planner_goal;
    let desired = state.fountain_state.planner_desired;
    let affordable = state.fountain_state.planner_affordable;
    let normalize = state.fountain_state.planner_normalize;
    let per_hr = state.fountain_state.planner_per_hr;
    let entity = cx.entity();
    let goal_picker = goal_select(
        "fountain-planner-goal",
        goal,
        entity.clone(),
        |state, goal| {
            state.planner_goal = goal;
        },
    );
    let mut controls = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(goal_picker);
    if goal == Goal::Optimal {
        let saved = fountain.desired_currency();
        controls = controls.child(desired_select(
            "fountain-planner-desired",
            desired,
            saved,
            entity.clone(),
            |state, desired| state.planner_desired = desired,
        ));
    }
    controls = controls
        .child(flag(
            "fountain-planner-affordable",
            "Affordable only",
            affordable,
            entity.clone(),
            |state, on| state.planner_affordable = on,
        ))
        .child(
            Checkbox::new("fountain-planner-normalize", normalize)
                .label("Normalize cost by currency value")
                .disabled(per_hr)
                .on_change({
                    let entity = entity.clone();
                    move |on, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.edit_fountain_state(cx, |state| state.planner_normalize = on);
                        });
                    }
                }),
        )
        .child(flag(
            "fountain-planner-perhr",
            "Rank by /hr",
            per_hr,
            entity,
            |state, on| state.planner_per_hr = on,
        ));
    let body = if rows.is_empty() {
        div()
            .id("fountain-planner-empty")
            .p_3()
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
            div()
                .id("fountain-planner-controls-card")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(controls)
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
    let mut table = Table::new([
        "#",
        "Water",
        "Upgrade",
        "Level",
        "Cost",
        "Time to Afford",
        "Gain",
        "% Gain",
        goal_detail_label(goal),
        "Efficiency",
        "Affordable",
    ]);
    for (rank, row) in rows.iter().take(30).enumerate() {
        table = table.row([
            div().child((rank + 1).to_string()).into_any_element(),
            div()
                .child(fountain::water_names()[row.water].clone())
                .into_any_element(),
            div().child(row.name).into_any_element(),
            div()
                .child(format!("{} → {}", num(row.level), num(row.level + 1.0)))
                .into_any_element(),
            div()
                .child(format!(
                    "{} {}",
                    num(row.cost),
                    fountain::currency_label(row.currency)
                ))
                .into_any_element(),
            afford(row.seconds_to_afford),
            div()
                .child(format!("+{}", num(row.delta)))
                .into_any_element(),
            div()
                .child(format!("+{:.3}%", row.pct_gain))
                .into_any_element(),
            div().child(goal_detail(row, goal)).into_any_element(),
            div()
                .child(format!("{:.3e}", row.efficiency))
                .into_any_element(),
            div()
                .child(if row.affordable {
                    Badge::new("Yes").tone(Tone::Success).into_any_element()
                } else {
                    Badge::new("No").tone(Tone::Warning).into_any_element()
                })
                .into_any_element(),
        ]);
    }
    div()
        .id("fountain-planner-table")
        .flex()
        .flex_col()
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

/// The goal's extra column, transcribed from the page's per-goal table headers.
fn goal_detail_label(goal: Goal) -> &'static str {
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
fn optimizer_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.fountain_plan_task(cx);
    let goal = state.fountain_state.optimizer_goal;
    let steps = state.fountain_state.optimizer_steps;
    let affordable = state.fountain_state.optimizer_affordable;
    let normalize = state.fountain_state.optimizer_normalize;
    let per_hr = state.fountain_state.optimizer_per_hr;
    let running = task.running;
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let run = Button::new(
        "run-fountain-plan",
        if running { "Optimizing…" } else { "Optimize" },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || !state.save_loaded)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_fountain_plan(cx));
    });
    let cancel = Button::new("cancel-fountain-plan", "Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_plan(cx));
        });
    let steps_picker = Select::new(
        "fountain-optimizer-steps",
        STEP_CHOICES.map(|count| Choice::new(count.to_string(), count.to_string())),
    )
    .label("Max steps")
    .selected(steps.to_string())
    .size(ControlSize::Sm)
    .on_change({
        let entity = entity.clone();
        move |value, _window, cx| {
            if let Ok(count) = value.parse::<usize>() {
                entity.update(cx, |state, cx| {
                    state.edit_fountain_state(cx, |state| state.optimizer_steps = count);
                });
            }
        }
    });
    let controls = div()
        .id("fountain-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(goal_select(
            "fountain-optimizer-goal",
            goal,
            entity.clone(),
            |state, goal| state.optimizer_goal = goal,
        ))
        .child(steps_picker)
        .child(flag(
            "fountain-optimizer-affordable",
            "Affordable only",
            affordable,
            entity.clone(),
            |state, on| state.optimizer_affordable = on,
        ))
        .child(
            Checkbox::new("fountain-optimizer-normalize", normalize)
                .label("Normalize cost by currency value")
                .disabled(per_hr)
                .on_change({
                    let entity = entity.clone();
                    move |on, _window, cx| {
                        entity.update(cx, |state, cx| {
                            state.edit_fountain_state(cx, |state| state.optimizer_normalize = on);
                        });
                    }
                }),
        )
        .child(flag(
            "fountain-optimizer-perhr",
            "Rank by /hr",
            per_hr,
            entity,
            |state, on| state.optimizer_per_hr = on,
        ))
        .child(run)
        .child(cancel);
    let mut body = div()
        .id("fountain-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls)
        .child(
            Caption::new(
                "Each purchase is the best gain per unit cost at the levels the plan has reached, \
                 paid from the saved opal balance. Affordable only skips a step the balance cannot \
                 cover; otherwise the plan waits for the currency's own income and adds that wait \
                 to the cumulative ETA.",
            )
            .text_color(muted),
        );
    if running && let Some(progress) = task.progress.clone() {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "fountain-plan",
            "Planning the upgrades",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_fountain_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.clone() {
        body = body.child(plan_result(&plan, task.stale));
    }
    Card::new()
        .header(
            CardHeader::new("Upgrade Optimizer")
                .description("Greedily buys the best upgrade for the selected goal."),
        )
        .child(body)
        .into_any_element()
}

/// The stored plan: its summary, its purchases and its individual steps.
fn plan_result(plan: &OptimizerPlan, stale: bool) -> AnyElement {
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
                    .p_3()
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
    let summary = div()
        .id("fountain-plan-result")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(Badge::new(format!("{} steps", plan.steps.len())).tone(Tone::Info))
        .child(Caption::new(format!(
            "Value {} → {} ({:+.1}%)",
            num(plan.start_value),
            num(plan.end_value),
            plan.gain_pct
        )));
    let mut groups = Table::new([
        "Water",
        "Upgrade",
        "Levels",
        "From",
        "To",
        "Total Cost",
        "Cumulative ETA",
    ]);
    for group in &plan.groups {
        groups = groups.row([
            div()
                .child(fountain::water_names()[group.water].clone())
                .into_any_element(),
            div().child(group.name).into_any_element(),
            div().child(group.count.to_string()).into_any_element(),
            div().child(num(group.start_level)).into_any_element(),
            div().child(num(group.end_level)).into_any_element(),
            div()
                .child(format!(
                    "{} {}",
                    num(group.total_cost),
                    fountain::currency_label(group.currency)
                ))
                .into_any_element(),
            eta(group.cumulative_seconds),
        ]);
    }
    let mut steps = Table::new([
        "#",
        "Water",
        "Upgrade",
        "Level",
        "Cost",
        "Gain",
        "Cumulative ETA",
    ]);
    for (index, step) in plan.steps.iter().enumerate() {
        steps = steps.row([
            div().child((index + 1).to_string()).into_any_element(),
            div()
                .child(fountain::water_names()[step.water].clone())
                .into_any_element(),
            div().child(step.name).into_any_element(),
            div().child(num(step.level)).into_any_element(),
            div()
                .child(format!(
                    "{} {}",
                    num(step.cost),
                    fountain::currency_label(step.currency)
                ))
                .into_any_element(),
            div()
                .child(format!("+{:.3}%", step.pct_gain))
                .into_any_element(),
            eta(step.cumulative_seconds),
        ]);
    }
    card.child(
        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .child(summary)
            .child(
                div()
                    .id("fountain-plan-steps")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Caption::new("Purchases, grouped by upgrade"))
                    .child(groups),
            )
            .child(
                div()
                    .id("fountain-plan-step-list")
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(Caption::new(
                        "Every step, in the order the optimizer buys it",
                    ))
                    .child(steps),
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

/// A goal selector shared by the planner and the optimizer.
pub(super) fn goal_select(
    id: &'static str,
    selected: Goal,
    entity: Entity<AppState>,
    edit: fn(&mut State, Goal),
) -> Select {
    Select::new(
        id,
        Goal::ALL.map(|goal| Choice::new(goal.id(), goal.label())),
    )
    .label("Goal")
    .selected(selected.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        if let Some(goal) = Goal::from_id(value) {
            entity.update(cx, |state, cx| {
                state.edit_fountain_state(cx, |state| edit(state, goal));
            });
        }
    })
}

/// The `optimal` goal's simulated desired currency.
pub(super) fn desired_select(
    id: &'static str,
    selected: Option<usize>,
    saved: usize,
    entity: Entity<AppState>,
    edit: fn(&mut State, Option<usize>),
) -> Select {
    let mut choices = vec![Choice::new(
        "-1",
        format!("Save ({})", fountain::currency_label(saved)),
    )];
    choices.extend(
        (0..fountain::CURRENCIES)
            .map(|currency| Choice::new(currency.to_string(), fountain::currency_name(currency))),
    );
    Select::new(id, choices)
        .label("Currency")
        .selected(selected.map_or("-1".to_owned(), |currency| currency.to_string()))
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let desired = value
                .parse::<i64>()
                .ok()
                .and_then(|value| (value >= 0).then_some(value as usize));
            entity.update(cx, |state, cx| {
                state.edit_fountain_state(cx, |state| edit(state, desired));
            });
        })
}

/// One boolean planner or optimizer control.
pub(super) fn flag(
    id: &'static str,
    label: &'static str,
    checked: bool,
    entity: Entity<AppState>,
    edit: fn(&mut State, bool),
) -> Checkbox {
    Checkbox::new(id, checked)
        .label(label)
        .on_change(move |on, _window, cx| {
            entity.update(cx, |state, cx| {
                state.edit_fountain_state(cx, |state| edit(state, on));
            });
        })
}

/// The seconds until an upgrade is affordable.
pub(super) fn afford(seconds: f64) -> AnyElement {
    if seconds <= 0.0 {
        return div()
            .child(Badge::new("Now").tone(Tone::Success))
            .into_any_element();
    }
    div()
        .child(if seconds.is_finite() {
            time(seconds)
        } else {
            "∞".to_owned()
        })
        .into_any_element()
}

/// A cumulative plan ETA.
pub(super) fn eta(seconds: f64) -> AnyElement {
    if seconds <= 0.0 {
        return div()
            .child(Badge::new("Now").tone(Tone::Success))
            .into_any_element();
    }
    div()
        .child(if seconds.is_finite() {
            time(seconds)
        } else {
            "∞".to_owned()
        })
        .into_any_element()
}
