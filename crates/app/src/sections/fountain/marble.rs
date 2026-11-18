//! FN-04: the Marble tab, the marble planner and the greedy marble optimizer.
//!
//! The planner measures one marble level of every upgrade the save owns and
//! ranks the gain per marble; the optimizer buys the best affordable marble
//! level and ranks again at the levels that purchase reached. The optimizer
//! runs through the shared [`idleon_ui::runner::TaskRunner`], so its progress
//! and cancel chrome match every other long job.

use super::controls::cached;
use super::overview::{num, odds, time};
use super::planner::{
    affordable, desired_select, eta, flag, goal_detail_label, goal_select, steps_select,
};
use super::table::{number, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, h_flex, v_flex};
use idleon_fountain as fountain;
use idleon_fountain::{Fountain, Goal, MarblePlan, MarbleRow};
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, Tone};

/// The Marble pane: the marble planner and the marble optimizer.
pub(super) fn marble_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(fountain) = state.fountain.as_ref() else {
        return div().into_any_element();
    };
    let request = state.fountain_state.marble_planner_request();
    let rows = cached(
        &mut state.fountain_controls.marble,
        (state.save_revision, request),
        || fountain.marble_planner(request),
    );
    v_flex()
        .id("fountain-marble")
        .gap(PANEL_GAP)
        .child(planner_card(state, fountain, &rows, window, cx))
        .child(optimizer_card(state, window, cx))
        .into_any_element()
}

/// The Marble Planner: its controls and the ranked next marble levels.
fn planner_card(
    state: &AppState,
    fountain: &Fountain,
    rows: &[MarbleRow],
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let selections = &state.fountain_state;
    let controls = &state.fountain_controls;
    let goal = selections.marble_goal;
    let mut row = h_flex().flex_wrap().gap_3().child(goal_select(
        "fountain-marble-goal",
        &controls.marble_goal,
        goal,
        window,
        cx,
    ));
    if goal == Goal::Optimal {
        row = row.child(desired_select(
            "fountain-marble-desired",
            &controls.marble_desired,
            selections.marble_desired,
            fountain.desired_currency(),
            window,
            cx,
        ));
    }
    row = row.child(flag(
        "fountain-marble-affordable",
        "Affordable only",
        selections.marble_affordable,
        false,
        cx.entity(),
        |state, on| state.marble_affordable = on,
    ));
    let body = if rows.is_empty() {
        div()
            .id("fountain-marble-planner-empty")
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
            v_flex()
                .id("fountain-marble-planner-card")
                .gap_3()
                .child(row)
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
    let table = table(
        &[
            ("#", true),
            ("Water", false),
            ("Upgrade", false),
            ("Marble Lv", true),
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
                .id(("fountain-marble-planner-row", rank))
                .child(number((rank + 1).to_string()))
                .child(text(fountain::water_names()[row.water].clone()))
                .child(text(row.name))
                .child(number(format!(
                    "{} → {}",
                    num(row.marble_level),
                    num(row.marble_level + 1.0)
                )))
                .child(number(format!("{} marble", num(row.cost))))
                .child(number(eta(row.seconds_to_afford)))
                .child(number(format!("+{}", num(row.delta))))
                .child(number(format!("+{:.3}%", row.pct_gain)))
                .child(text(marble_detail(row, goal)))
                .child(number(format!("{:.3e}", row.efficiency)))
                .child(text(affordable(row.affordable)))
        }),
    );
    v_flex()
        .id("fountain-marble-planner-table")
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
fn optimizer_card(state: &AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let runner = state.fountain_marble_plan.read(cx);
    let running = runner.running();
    let cancelled = runner.cancelled();
    let progress = runner.progress().cloned();
    let stale = runner.is_stale(&state.fountain_marble_inputs());
    // The stored plan drawn is the one the runner still holds.
    let plan = runner
        .result()
        .and(state.fountain_controls.marble_plan.clone());
    let selections = &state.fountain_state;
    let controls = &state.fountain_controls;
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let run_entity = entity.clone();
    let cancel_entity = entity.clone();
    let run = Button::new("run-fountain-marble-plan")
        .label(if running {
            "Optimizing…"
        } else {
            "Optimize marble"
        })
        .primary()
        .icon(IconName::Play)
        .disabled(running || !state.save_loaded)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_fountain_marble_plan(cx));
        });
    let cancel = Button::new("cancel-fountain-marble-plan")
        .label("Cancel")
        .icon(IconName::X)
        .disabled(!running)
        .on_click(move |_event, _window, cx| {
            cancel_entity.update(cx, |state, cx| state.cancel_fountain_marble_plan(cx));
        });
    let row = h_flex()
        .id("fountain-marble-optimizer-controls")
        .flex_wrap()
        .gap_3()
        .child(goal_select(
            "fountain-marble-optimizer-goal",
            &controls.marble_optimizer_goal,
            selections.marble_optimizer_goal,
            window,
            cx,
        ))
        .child(steps_select(
            "fountain-marble-optimizer-steps",
            &controls.marble_optimizer_steps,
            selections.marble_optimizer_steps,
            window,
            cx,
        ))
        .child(flag(
            "fountain-marble-optimizer-affordable",
            "Affordable only",
            selections.marble_optimizer_affordable,
            false,
            entity.clone(),
            |state, on| state.marble_optimizer_affordable = on,
        ))
        .child(run)
        .child(cancel);
    let mut body = v_flex()
        .id("fountain-marble-optimizer")
        .gap_3()
        .child(row)
        .child(
            Caption::new(
                "Each purchase is the best gain per marble at the levels the plan has reached, \
                 paid from the saved marble balance. Affordable only skips a level the balance \
                 cannot cover; otherwise the plan waits for the marble bar's own income and adds \
                 that wait to the cumulative ETA.",
            )
            .text_color(muted),
        );
    if running && let Some(progress) = progress {
        body = body.child(idleon_ui::run_chrome::progress_card(
            "fountain-marble-plan",
            "Planning the marble levels",
            "Runs off the UI thread; cancelling keeps the plan this run replaced.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_fountain_marble_plan(cx));
            },
        ));
    }
    if cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "fountain-marble-plan-cancelled",
            plan.is_some(),
        ));
    }
    if let Some(plan) = plan {
        body = body.child(plan_result(&plan, stale));
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
            .child(div().id("fountain-marble-plan-empty").child(Caption::new(
                "No affordable marble upgrades affect this goal.",
            )))
            .into_any_element();
    }
    let summary = h_flex()
        .id("fountain-marble-plan-result")
        .flex_wrap()
        .gap_3()
        .child(tag(
            format!("{} marble levels", plan.steps.len()),
            Tone::Info,
        ))
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
                .id(("fountain-marble-plan-group", index))
                .child(text(fountain::water_names()[group.water].clone()))
                .child(text(group.name))
                .child(number(group.count.to_string()))
                .child(number(num(group.start_level)))
                .child(number(num(group.end_level)))
                .child(number(format!("{} marble", num(group.total_cost))))
                .child(number(eta(group.cumulative_seconds)))
        }),
    );
    let mut body = v_flex()
        .gap_3()
        .child(summary)
        .child(
            v_flex()
                .id("fountain-marble-plan-groups")
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
