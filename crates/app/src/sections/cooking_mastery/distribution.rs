//! The Optimal Distribution card: the daily reset budget across the upgrades.
//!
//! The card draws one table under a "Points to spend" input, adds the goal the
//! section's inputs ask for, and offers the plan as text for the copy action.
//! Planning itself is [`idleon_cooking_mastery::Mastery::distribution`]; this
//! module only draws it.

use super::{number_cell, table, text_cell, whole};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::component::clipboard::Clipboard;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_cooking_mastery::{Goal, Plan, Row};
use idleon_ui::format;
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, Choice, Statistic, Tone, show_number,
};

/// The card: the controls, the plan's headline, its table and the copy action.
pub(crate) fn card(
    state: &AppState,
    plan: &Plan,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let controls = &state.mastery_controls;
    let goals = Goal::ALL
        .into_iter()
        .map(|goal| Choice::new(goal, goal.label()))
        .collect();
    controls.goal.show(goals, Some(&plan.goal), window, cx);
    show_number(&controls.budget, plan.budget, 0, window, cx);
    let goal = Select::new(controls.goal.state())
        .id("cooking-mastery-goal")
        .accessibility_label("Goal")
        .menu_width(gpui::px(200.0))
        .small();

    Card::new()
        .header(
            CardHeader::new("Optimal Distribution (Daily Reset)").description(
                "Every mastery point the rank and the Rift Spooker grant, spent where one more \
                 point buys the most EXP rate.",
            ),
        )
        .child(
            h_flex()
                .id("cooking-mastery-distribution-controls")
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(
                    v_flex()
                        .gap_1()
                        .child(Caption::new("Points to Spend").text_color(muted))
                        .child(
                            h_flex()
                                .gap_2()
                                .child(ui::number_field(
                                    "cooking-mastery-budget",
                                    &controls.budget,
                                    "Points to Spend",
                                ))
                                .child(
                                    Caption::new(format!("of {} total", whole(plan.available)))
                                        .text_color(muted),
                                ),
                        ),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .w(gpui::px(200.0))
                        .child(Caption::new("Goal").text_color(muted))
                        .child(goal),
                ),
        )
        .child(headline(plan, muted))
        .child(rows(plan))
        .child(notes(plan, muted))
        .child(
            h_flex()
                .id("cooking-mastery-distribution-copy-row")
                .gap_2()
                .child(
                    Caption::new(
                        "Copy the plan as text: the goal, the budget and every level change.",
                    )
                    .text_color(muted),
                )
                .child(div().flex_1())
                .child(Clipboard::new("cooking-mastery-distribution-copy").value(plan.copy_text())),
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
                super::span(plan.hours)
            ),
            format!("{:+.2}% vs saved", plan.improvement),
        )
    } else if plan.improvement < -0.01 {
        (
            format!(
                "{:.2}% below the saved levels · {} to the next level",
                plan.improvement,
                super::span(plan.hours)
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
    h_flex()
        .id("cooking-mastery-distribution")
        .role(Role::Label)
        .aria_label(label)
        .flex_wrap()
        .gap_3()
        .child(
            Statistic::new(
                "cooking-mastery-distribution-rate",
                "Optimal EXP Rate",
                format!("{value:.2}{suffix}"),
            )
            .suffix("/hr"),
        )
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .id("cooking-mastery-distribution-improvement")
                        .child(verdict),
                )
                .child(
                    Caption::new(format!(
                        "{} now · {} to the next level",
                        format::rate(plan.current_rate),
                        super::span(plan.current_hours)
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The page's table: current level, optimal level, the change and the bonus.
fn rows(plan: &Plan) -> AnyElement {
    div()
        .id("cooking-mastery-distribution-rows")
        .child(table(
            &[
                ("Upgrade", false),
                ("Current", true),
                ("Optimal", true),
                ("Change", true),
                ("EXP Bonus%", true),
            ],
            plan.rows.iter().map(|row| {
                TableRow::new()
                    .id(("cooking-mastery-distribution-table-row", row.index))
                    .children(row_cells(row))
            }),
        ))
        .into_any_element()
}

/// One comparison row; a locked upgrade says what rank it waits for.
fn row_cells(row: &Row) -> [gpui_kit::component::table::TableCell; 5] {
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
    let name = v_flex()
        .id(("cooking-mastery-distribution-row", row.index))
        .role(Role::Label)
        .aria_label(label)
        .gap_0p5()
        .child(row.name)
        .when(row.locked, |name| {
            name.child(Caption::new(format!(
                "needs mastery rank {}",
                row.required_level
            )))
        });
    [
        text_cell(name),
        number_cell(whole(row.current)),
        number_cell(whole(row.planned)),
        number_cell(change),
        number_cell(format!("{:.1}%", row.bonus)),
    ]
}

/// What the plan holds back, and what the budget cannot reach.
fn notes(plan: &Plan, muted: gpui::Hsla) -> AnyElement {
    let mut body = v_flex().id("cooking-mastery-distribution-notes").gap_1();
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
            Callout::new(Tone::Info, "No points to spend").child(
                Caption::new("Raise the budget, or level the mastery rank for more points.")
                    .text_color(muted),
            ),
        );
    }
    body.into_any_element()
}
