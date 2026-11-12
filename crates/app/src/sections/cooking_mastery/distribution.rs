//! The Optimal Distribution card: the daily reset budget across the upgrades.
//!
//! The page renders one table under a "Points to spend" input. The card keeps
//! that input, adds the goal the section's inputs ask for, and offers the plan as
//! text for the copy action. Planning itself is
//! [`idleon_cooking_mastery::Mastery::distribution`]; this module only
//! draws it.

use super::whole;
use crate::state::AppState;
use ely_gpui_component::buttons::CopyButton;
use ely_gpui_component::data_display::Statistic;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_cooking_mastery::{Goal, Plan, Row};
use idleon_ui::format;

/// The card: the controls, the plan's headline, its table and the copy action.
pub(crate) fn card(
    state: &AppState,
    mastery: &idleon_cooking_mastery::Mastery,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let plan = state.mastery_plan.plan(mastery);
    let muted = cx.theme().colors.fg_muted;
    let text = plan.copy_text();

    let goal_entity = cx.entity();
    let goal = Select::new(
        "cooking-mastery-goal",
        Goal::ALL
            .into_iter()
            .map(|goal| Choice::new(goal.id(), goal.label())),
    )
    .label("Goal")
    .selected(plan.goal.id())
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        let entity = goal_entity.clone();
        let value = value.to_string();
        entity.update(cx, |state, cx| {
            state.mastery_plan.set_goal(&value);
            cx.notify();
        });
    });

    let budget_entity = cx.entity();
    let budget = Stepper::new("cooking-mastery-budget", plan.budget)
        .range(0.0, plan.available)
        .on_change(move |value, _window, cx| {
            let entity = budget_entity.clone();
            entity.update(cx, |state, cx| {
                state.mastery_plan.set_budget(value);
                cx.notify();
            });
        });

    Card::new()
        .header(
            CardHeader::new("Optimal Distribution (Daily Reset)").description(
                "Every mastery point the rank and the Rift Spooker grant, spent where one more \
                 point buys the most EXP rate.",
            ),
        )
        .child(
            div()
                .id("cooking-mastery-distribution-controls")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .px_3()
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Points to Spend").text_color(muted))
                        .child(
                            div().flex().items_center().gap_2().child(budget).child(
                                Caption::new(format!("of {} total", whole(plan.available)))
                                    .text_color(muted),
                            ),
                        ),
                )
                .child(goal),
        )
        .child(div().p_3().child(headline(&plan, muted)))
        .child(div().px_3().child(table(&plan)))
        .child(notes(&plan, muted))
        .child(
            div()
                .id("cooking-mastery-distribution-copy-row")
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pb_3()
                .child(
                    Caption::new(
                        "Copy the plan as text: the goal, the budget and every level change.",
                    )
                    .text_color(muted),
                )
                .child(div().flex_1())
                .child(CopyButton::new("cooking-mastery-distribution-copy", text)),
        )
        .into_any_element()
}

/// The plan's headline: the rate, the improvement and the hours it saves.
fn headline(plan: &Plan, muted: gpui::Hsla) -> AnyElement {
    let (value, suffix) = format::scale(plan.rate);
    let improved = plan.improvement > 0.01;
    // The page only claims "already optimal" when nothing is better; a save
    // that already spent more than this budget says so instead.
    let (reading, verdict) = if improved {
        (
            format!(
                "+{:.2}% vs the saved distribution · {} to the next level",
                plan.improvement,
                hours(plan.hours)
            ),
            format!("{:+.2}% vs saved", plan.improvement),
        )
    } else if plan.improvement < -0.01 {
        (
            format!(
                "{:.2}% below the saved levels · {} to the next level",
                plan.improvement,
                hours(plan.hours)
            ),
            format!("{:.2}% below saved", plan.improvement),
        )
    } else {
        (
            "The saved distribution is already optimal for this budget.".to_string(),
            "Already optimal".to_string(),
        )
    };
    // The label carries the budget and the rank the plan holds for, so the
    // harness reads the switch and the spend from the same node the cards show.
    let held = match plan.waits_for_rank() {
        Some(rank) => format!(", held until rank {rank}"),
        None => String::new(),
    };
    let label = format!(
        "Plan: {} EXP per hour with {} of {} points{held} · {reading}",
        format::rate(plan.rate),
        whole(plan.budget),
        whole(plan.available),
    );
    let row = div()
        .id("cooking-mastery-distribution")
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            Statistic::new(
                "cooking-mastery-distribution-rate",
                "Optimal EXP Rate",
                value,
            )
            .decimals(2)
            .suffix(SharedString::from(format!("{suffix}/hr")))
            .size(TextSize::Xl),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .id("cooking-mastery-distribution-improvement")
                        .child(SharedString::from(verdict)),
                )
                .child(
                    Caption::new(format!(
                        "{} now · {} to the next level",
                        format::rate(plan.current_rate),
                        hours(plan.current_hours)
                    ))
                    .text_color(muted),
                ),
        );
    row.into_any_element()
}

/// The page's table: current level, optimal level, the change and the bonus.
fn table(plan: &Plan) -> AnyElement {
    let mut table = Table::new(["Upgrade", "Current", "Optimal", "Change", "EXP Bonus%"]);
    for row in &plan.rows {
        table = table.row(row_cells(row));
    }
    div()
        .id("cooking-mastery-distribution-rows")
        .child(table)
        .into_any_element()
}

/// One comparison row; a locked upgrade says what rank it waits for.
fn row_cells(row: &Row) -> [AnyElement; 5] {
    let change = if row.change > 0.0 {
        format!("+{}", whole(row.change))
    } else {
        whole(row.change)
    };
    let label = format!(
        "{}: {} → {} ({change}) · {:.1}% bonus{}",
        row.name,
        whole(row.current),
        whole(row.planned),
        row.bonus,
        if row.locked {
            format!(" · locked, needs rank {}", row.required_level)
        } else {
            String::new()
        },
    );
    let mut name = div()
        .id(("cooking-mastery-distribution-row", row.index))
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_col()
        .gap_0p5()
        .child(SharedString::from(row.name));
    if row.locked {
        name = name.child(div().text_size(px(11.0)).child(SharedString::from(format!(
            "needs mastery rank {}",
            row.required_level
        ))));
    }
    [
        name.into_any_element(),
        cell(whole(row.current)),
        cell(whole(row.planned)),
        cell(change),
        cell(format!("{:.1}%", row.bonus)),
    ]
}

/// A plain table cell.
fn cell(text: String) -> AnyElement {
    div().child(SharedString::from(text)).into_any_element()
}

/// What the plan holds back, and what the budget cannot reach.
fn notes(plan: &Plan, muted: gpui::Hsla) -> AnyElement {
    let mut body = div()
        .id("cooking-mastery-distribution-notes")
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .pb_2();
    // The page's own note: SMOKY never enters the distribution.
    body = body.child(
        Caption::new(format!(
            "{} is not an EXP upgrade, so no point goes there.",
            idleon_cooking_mastery::UPGRADE_NAMES[idleon_cooking_mastery::RIBBON_CHANCE]
        ))
        .text_color(muted),
    );
    if let Some(rank) = plan.waits_for_rank() {
        let unlocking = plan
            .rows
            .iter()
            .find(|row| !row.locked && f64::from(row.required_level) == rank)
            .map(|row| row.name);
        body = body.child(
            Caption::new(match unlocking {
                Some(name) => format!(
                    "{name} unlocks at rank {rank}: this goal holds the same {} points until then.",
                    whole(plan.budget)
                ),
                None => format!(
                    "The next upgrade unlocks at rank {rank}: this goal holds the same {} points until then.",
                    whole(plan.budget)
                ),
            })
            .text_color(muted),
        );
    }
    if plan.budget < plan.available {
        body = body.child(
            Caption::new(format!(
                "{} points stay unspent: the budget is below the {} the rank grants.",
                whole(plan.available - plan.budget),
                whole(plan.available)
            ))
            .text_color(muted),
        );
    }
    if plan.budget == 0.0 {
        body = body.child(
            Callout::new(Severity::Info)
                .title("No points to spend")
                .child(
                    Caption::new("Raise the budget, or level the mastery rank for more points.")
                        .text_color(muted)
                        .into_any_element(),
                ),
        );
    }
    body.into_any_element()
}

/// A span of hours in the section's own style.
fn hours(hours: f64) -> String {
    if hours.is_finite() {
        format::hours(hours)
    } else {
        "∞".into()
    }
}
