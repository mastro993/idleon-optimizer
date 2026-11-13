//! FN-04: the Marble tab, the marble planner and the greedy marble optimizer.
//!
//! Ported from the `#tab-marble` half of `fountain-calc.html`. The planner
//! measures one marble level of every upgrade the save owns and ranks the gain
//! per marble; the optimizer buys the best affordable marble level and ranks
//! again at the levels that purchase reached. The optimizer runs through the
//! shared [`idleon_ui::runner::TaskRunner`], so its progress and cancel chrome
//! match every other long job.

use super::overview::{num, odds, time};
use super::planner::{afford, desired_select, eta, flag, goal_select};
use super::state::STEP_CHOICES;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, Goal, MarblePlan, MarbleRow};

/// The Marble pane: the marble planner and the marble optimizer.
pub(super) fn marble_pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(fountain) = state.fountain.clone() else {
        return div().into_any_element();
    };
    let rows = fountain.marble_planner(state.fountain_state.marble_planner_request());
    div()
        .id("fountain-marble")
        .flex()
        .flex_col()
        .gap_3()
        .child(planner_card(&fountain, &rows, state, cx))
        .child(optimizer_card(state, cx))
        .into_any_element()
}

/// The Marble Planner: its controls and the ranked next marble levels.
fn planner_card(
    fountain: &Fountain,
    rows: &[MarbleRow],
    state: &mut AppState,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let goal = state.fountain_state.marble_goal;
    let desired = state.fountain_state.marble_desired;
    let affordable = state.fountain_state.marble_affordable;
    let entity = cx.entity();
    let mut controls = div()
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(goal_select(
            "fountain-marble-goal",
            goal,
            entity.clone(),
            |state, goal| state.marble_goal = goal,
        ));
    if goal == Goal::Optimal {
        controls = controls.child(desired_select(
            "fountain-marble-desired",
            desired,
            fountain.desired_currency(),
            entity.clone(),
            |state, desired| state.marble_desired = desired,
        ));
    }
    controls = controls.child(flag(
        "fountain-marble-affordable",
        "Affordable only",
        affordable,
        entity,
        |state, on| state.marble_affordable = on,
    ));
    let body = if rows.is_empty() {
        div()
            .id("fountain-marble-planner-empty")
            .p_3()
            .child(Caption::new(
                "No marble upgrades affect this goal, or none are affordable.",
            ))
            .into_any_element()
    } else {
        planner_table(rows, goal)
    };
    Card::new()
        .header(
            CardHeader::new("Marble Planner")
                .description(
                    "One marble level of every upgrade you own and can marble, ranked by the gain \
                     toward the selected goal per marble spent.",
                )
                .action(Caption::new(format!("{} candidates", rows.len())).text_color(muted)),
        )
        .child(
            div()
                .id("fountain-marble-planner-card")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(controls)
                .child(body)
                .child(Caption::new(
                    "A marble level multiplies that upgrade's bonus by 1.5 + 0.5 × its marble \
                     level, so the same next level is worth more on a higher-level upgrade. Only \
                     upgrades you already own can be marbled.",
                )),
        )
        .into_any_element()
}

/// The planner's ranking table, with the goal's own column.
fn planner_table(rows: &[MarbleRow], goal: Goal) -> AnyElement {
    let top = format!(
        "Top pick: {} ({} Water)",
        rows[0].name,
        fountain::water_names()[rows[0].water]
    );
    let mut table = Table::new([
        "#",
        "Water",
        "Upgrade",
        "Marble Lv",
        "Cost",
        "Time to Afford",
        "Gain",
        "% Gain",
        marble_detail_label(goal),
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
                .child(format!(
                    "{} → {}",
                    num(row.marble_level),
                    num(row.marble_level + 1.0)
                ))
                .into_any_element(),
            div()
                .child(format!("{} marble", num(row.cost)))
                .into_any_element(),
            afford(row.seconds_to_afford),
            div()
                .child(format!("+{}", num(row.delta)))
                .into_any_element(),
            div()
                .child(format!("+{:.3}%", row.pct_gain))
                .into_any_element(),
            div().child(marble_detail(row, goal)).into_any_element(),
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
        .id("fountain-marble-planner-table")
        .flex()
        .flex_col()
        .gap_2()
        .child(table)
        .child(
            div()
                .id("fountain-marble-planner-top")
                .role(gpui::Role::Label)
                .aria_label(top.clone())
                .child(Caption::new(top)),
        )
        .child(Caption::new(
            "The top thirty rows. Every number is measured one marble level above the saved \
             cavern, so a goal the marble level does not move leaves no row.",
        ))
        .into_any_element()
}

/// The goal's extra column, transcribed from the page's headers.
fn marble_detail_label(goal: Goal) -> &'static str {
    match goal {
        Goal::Fill => "Coin / Marble / Duck Δ",
        Goal::Royal => "Royal Δ",
        Goal::Duck => "Duck Δ",
        Goal::Lucky => "Lucky Δ",
        Goal::Optimal => "Speed / Value Δ",
        _ => "Goal Δ",
    }
}

/// The goal's own reading for one marble candidate.
fn marble_detail(row: &MarbleRow, goal: Goal) -> String {
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

/// The Marble Optimizer: its controls, the run chrome and the stored plan.
fn optimizer_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let task = state.fountain_marble_plan_task(cx);
    let goal = state.fountain_state.marble_optimizer_goal;
    let steps = state.fountain_state.marble_optimizer_steps;
    let affordable = state.fountain_state.marble_optimizer_affordable;
    let running = task.running;
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let run_entity = cx.entity();
    let cancel_entity = cx.entity();
    let run = Button::new(
        "run-fountain-marble-plan",
        if running {
            "Optimizing…"
        } else {
            "Optimize marble"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(running || !state.save_loaded)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_fountain_marble_plan(cx));
    });
    let cancel = Button::new("cancel-fountain-marble-plan", "Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_marble_plan(cx));
        });
    let steps_picker = Select::new(
        "fountain-marble-optimizer-steps",
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
                    state.edit_fountain_state(cx, |state| state.marble_optimizer_steps = count);
                });
            }
        }
    });
    let controls = div()
        .id("fountain-marble-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(goal_select(
            "fountain-marble-optimizer-goal",
            goal,
            entity.clone(),
            |state, goal| state.marble_optimizer_goal = goal,
        ))
        .child(steps_picker)
        .child(flag(
            "fountain-marble-optimizer-affordable",
            "Affordable only",
            affordable,
            entity,
            |state, on| state.marble_optimizer_affordable = on,
        ))
        .child(run)
        .child(cancel);
    let mut body = div()
        .id("fountain-marble-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(controls)
        .child(
            Caption::new(
                "Each purchase is the best gain per marble at the levels the plan has reached, \
                 paid from the saved marble balance. Affordable only skips a level the balance \
                 cannot cover; otherwise the plan waits for the marble bar's own income and adds \
                 that wait to the cumulative ETA.",
            )
            .text_color(muted),
        );
    if running && let Some(progress) = task.progress.clone() {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "fountain-marble-plan",
            "Planning the marble levels",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_fountain_marble_plan(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-marble-plan-cancelled",
            task.result.is_some(),
        ));
    }
    if let Some(plan) = task.result.clone() {
        body = body.child(plan_result(&plan, task.stale));
    }
    Card::new()
        .header(
            CardHeader::new("Marble Optimizer")
                .description("Greedily buys the best marble level for the selected goal."),
        )
        .child(body)
        .into_any_element()
}

/// The stored marble plan: its summary and per-upgrade purchases.
fn plan_result(plan: &MarblePlan, stale: bool) -> AnyElement {
    let mut header = CardHeader::new("Marble plan").description(format!(
        "{} marble levels · {} → {} ({:+.1}%)",
        plan.steps.len(),
        num(plan.start_value),
        num(plan.end_value),
        plan.gain_pct
    ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("fountain-marble-plan"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("fountain-marble-plan"));
    }
    if plan.steps.is_empty() {
        return card
            .child(
                div()
                    .id("fountain-marble-plan-empty")
                    .p_3()
                    .child(Caption::new(
                        "No affordable marble upgrades affect this goal.",
                    )),
            )
            .into_any_element();
    }
    let summary = div()
        .id("fountain-marble-plan-result")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(Badge::new(format!("{} marble levels", plan.steps.len())).tone(Tone::Info))
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
                .child(format!("{} marble", num(group.total_cost)))
                .into_any_element(),
            eta(group.cumulative_seconds),
        ]);
    }
    let mut body = div()
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(summary)
        .child(
            div()
                .id("fountain-marble-plan-groups")
                .flex()
                .flex_col()
                .gap_2()
                .child(Caption::new("Purchases, grouped by upgrade"))
                .child(groups),
        )
        .child(
            div()
                .id("fountain-marble-plan-remaining")
                .child(Caption::new(format!(
                    "Remaining: {} marble",
                    num(plan.remaining)
                ))),
        );
    if plan.groups.len() < plan.steps.len() {
        body = body.child(Caption::new(format!(
            "{} marble levels; the table groups them by upgrade.",
            plan.steps.len()
        )));
    }
    card.child(body).into_any_element()
}
