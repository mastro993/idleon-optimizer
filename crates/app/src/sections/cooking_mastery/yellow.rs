//! The Yellow Point Allocator pane: the page's `meal-alloc` tab.
//!
//! The page renders one table under a "Points to spend" input and a goal select,
//! both re-planning on every change, and keeps the character context the stat
//! goals scale with beside them. The card keeps those controls, shows the
//! selected goal's value against the saved points, and offers the table and the
//! context the plan was computed with. Planning itself is
//! [`idleon_cooking_mastery::Yellow::allocation`]; this module only draws it.

use super::whole;
use crate::state::AppState;
use ely_gpui_component::feedback::Callout;
use ely_gpui_component::forms::{Choice, Select, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_cooking_mastery::{Allocation, MealRow, Yellow};

/// The pane: the controls, the plan's headline, its table and the context.
pub(crate) fn pane(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(yellow) = state.yellow.clone() else {
        return super::prompt(cx);
    };
    let muted = cx.theme().colors.fg_muted;
    // No plan means no goal has two leveled meals, the page's own notice.
    let Some(plan) = state.mastery_plan.yellow_plan(&yellow) else {
        return no_goals(&yellow, muted);
    };
    let card = Card::new()
        .header(CardHeader::new("Yellow Point Allocator").description(
            "Every yellow point spent across the meals of one goal, where one more point \
                 buys the most.",
        ))
        .child(controls(&yellow, &plan, cx))
        .child(div().p_3().child(headline(&plan, &yellow, muted)))
        .child(sources(&plan, muted))
        .child(div().px_3().child(table(&plan)))
        .child(notes(&plan, &yellow, muted));
    div()
        .id("cooking-mastery-yellow-pane")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(card)
        .into_any_element()
}

/// The page's notice when no goal has two leveled meals to compare.
fn no_goals(yellow: &Yellow, muted: gpui::Hsla) -> AnyElement {
    div()
        .id("cooking-mastery-yellow")
        .role(Role::Label)
        .aria_label(format!(
            "No goals have at least two leveled meals to compare · {} yellow points available",
            whole(yellow.budget)
        ))
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
            Callout::new(Severity::Info)
                .title("No goals have at least two leveled meals to compare")
                .child(
                    Caption::new("Level up meals in these categories first.").into_any_element(),
                ),
        )
        .child(
            Caption::new(format!(
                "The account holds {} yellow points, from the mastery rank and the bonuses that \
                 grant them.",
                whole(yellow.budget),
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// The budget, goal and character controls; every change re-plans.
fn controls(yellow: &Yellow, plan: &Allocation, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;

    let budget_entity = cx.entity();
    let budget = Stepper::new("cooking-mastery-yellow-budget", plan.spent)
        .range(0.0, plan.available)
        .on_change(move |value, _window, cx| {
            let entity = budget_entity.clone();
            entity.update(cx, |state, cx| {
                state.mastery_plan.set_yellow_budget(value);
                cx.notify();
            });
        });

    let goal_entity = cx.entity();
    let goal = Select::new(
        "cooking-mastery-yellow-goal",
        yellow
            .goals()
            .into_iter()
            .map(|goal| Choice::new(goal.id, goal.name)),
    )
    .label("Goal")
    .selected(plan.goal.id)
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        let entity = goal_entity.clone();
        let value = value.to_string();
        entity.update(cx, |state, cx| {
            state.mastery_plan.set_yellow_goal(&value);
            cx.notify();
        });
    });

    let character: Option<AnyElement> = (!yellow.characters.is_empty()).then(|| {
        let character_entity = cx.entity();
        div()
            .flex()
            .flex_col()
            .gap_1()
            .child(Caption::new("Character Context").text_color(muted))
            .child(
                Select::new(
                    "cooking-mastery-yellow-character",
                    yellow
                        .characters
                        .iter()
                        .enumerate()
                        .map(|(index, character)| {
                            Choice::new(index.to_string(), character.name.clone())
                        }),
                )
                .selected(plan.character.to_string())
                .size(ControlSize::Sm)
                .on_change(move |value, _window, cx| {
                    let entity = character_entity.clone();
                    let index = value.to_string().parse::<usize>().unwrap_or(0);
                    entity.update(cx, |state, cx| {
                        state.mastery_plan.set_character(index);
                        cx.notify();
                    });
                })
                .into_any_element(),
            )
            .into_any_element()
    });

    let mut row = div()
        .id("cooking-mastery-yellow-controls")
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
                .child(div().flex().items_center().gap_2().child(budget).child(
                    Caption::new(format!("of {} total", whole(plan.available))).text_color(muted),
                )),
        )
        .child(goal);
    if let Some(character) = character {
        row = row.child(character);
    }
    row.into_any_element()
}

/// The selected goal's value, against the saved points.
fn headline(plan: &Allocation, yellow: &Yellow, muted: gpui::Hsla) -> AnyElement {
    let unit = plan.goal.unit;
    let improvement = plan.improvement();
    let reading = if improvement > 0.01 {
        format!("{improvement:+.2}% over the saved points")
    } else if improvement < -0.01 {
        format!("{improvement:.2}% below the saved points")
    } else {
        "the saved points are already optimal for this budget".to_string()
    };
    let label = format!(
        "{}: {} with {} of {} yellow points on {} · {} · {} meals",
        plan.goal.name,
        unit.format(plan.value),
        whole(plan.spent),
        whole(plan.available),
        plan.character_name(yellow),
        reading,
        plan.rows.len(),
    );
    div()
        .id("cooking-mastery-yellow")
        .role(Role::Label)
        .aria_label(label)
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(Caption::new("Optimal Meal Bonus").text_color(muted))
                .child(
                    div()
                        .id("cooking-mastery-yellow-value")
                        .text_size(px(20.0))
                        .child(SharedString::from(unit.format(plan.value))),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .id("cooking-mastery-yellow-improvement")
                        .child(SharedString::from(reading)),
                )
                .child(
                    Caption::new(format!(
                        "{} with the saved points · {} of {} points spent",
                        unit.format(plan.current_value),
                        whole(plan.spent),
                        whole(plan.available),
                    ))
                    .text_color(muted),
                ),
        )
        .into_any_element()
}

/// The page's "Meal sources" line and the goal's note.
fn sources(plan: &Allocation, muted: gpui::Hsla) -> AnyElement {
    let categories: Vec<&str> = plan
        .goal
        .categories
        .iter()
        .map(|category| plan.goal.category_label(category))
        .collect();
    let mut line = format!("Meal sources: {}", categories.join(" · "));
    if !plan.goal.note.is_empty() {
        line.push_str(" · ");
        line.push_str(plan.goal.note);
    }
    div()
        .id("cooking-mastery-yellow-sources")
        .px_3()
        .pb_2()
        .child(Caption::new(line).text_color(muted))
        .into_any_element()
}

/// The page's allocation table, most points first.
fn table(plan: &Allocation) -> AnyElement {
    let multi_category = plan.goal.categories.len() > 1;
    let mut headers = vec!["Meal"];
    if multi_category {
        headers.push("Category");
    }
    headers.extend([
        "Effect",
        "Meal Lv",
        "Per Lv",
        "Base",
        "Points Now",
        "Points Optimal",
        "Mastery",
    ]);
    let mut table = Table::new(headers);
    for row in &plan.rows {
        let mut cells = vec![row_name(row)];
        if multi_category {
            cells.push(cell(plan.goal.category_label(&row.category).to_string()));
        }
        cells.extend([
            cell(row.effect.clone()),
            cell(whole(row.level)),
            cell(format!("{}", row.bonus)),
            cell(format!("{:.2}", row.base_weight)),
            cell(whole(row.current)),
            cell(whole(row.allocated)),
            cell(format!("{:.3}x", row.mastery)),
        ]);
        table = table.row(cells);
    }
    div()
        .id("cooking-mastery-yellow-rows")
        .child(table)
        .into_any_element()
}

/// The meal's name, with the points it takes in the accessible label.
fn row_name(row: &MealRow) -> AnyElement {
    div()
        .id(("cooking-mastery-yellow-row", row.index))
        .role(Role::Label)
        .aria_label(format!(
            "{}: {} points now, {} optimal · {}",
            row.name,
            whole(row.current),
            whole(row.allocated),
            row.effect,
        ))
        .child(SharedString::from(row.name.clone()))
        .into_any_element()
}

/// A plain table cell.
fn cell(text: String) -> AnyElement {
    div().child(SharedString::from(text)).into_any_element()
}

/// What the plan was computed with: the context the stat goals scale by, the
/// points left over and a budget the plan had to reject.
fn notes(plan: &Allocation, yellow: &Yellow, muted: gpui::Hsla) -> AnyElement {
    let mut body = div()
        .id("cooking-mastery-yellow-notes")
        .flex()
        .flex_col()
        .gap_1()
        .px_3()
        .pt_2()
        .pb_3();
    if plan.rejected() {
        let detail = if plan.requested.is_finite() {
            format!(
                "A budget of {} was rejected and clamped to {} of the {} points the account holds.",
                whole(plan.requested),
                whole(plan.spent),
                whole(plan.available),
            )
        } else {
            format!(
                "The stored budget was not a number, so the plan spends nothing of the {} points \
                 the account holds.",
                whole(plan.available),
            )
        };
        body = body.child(
            Callout::new(Severity::Warning)
                .title("Budget rejected")
                .child(Caption::new(detail).into_any_element()),
        );
    }
    let kitchens = yellow.kitchens();
    if kitchens.count > 0 {
        body = body.child(
            Caption::new(format!(
                "Kitchens: {} unlocked · {} average upgrade levels · {:.0} rate-weighted Cabbage \
                 steps for meals and {:.0} for recipes ({:.0}–{:.0} per kitchen)",
                kitchens.count,
                whole(kitchens.average_levels),
                kitchens.meal_steps,
                kitchens.recipe_steps,
                kitchens.min_steps,
                kitchens.max_steps,
            ))
            .text_color(muted),
        );
    }
    if plan.rows.is_empty() {
        body = body
            .child(Caption::new("No leveled meals in this goal's categories.").text_color(muted));
    } else if plan.unspent > 0.0 {
        body = body.child(
            Caption::new(format!(
                "{} points remain for other categories.",
                whole(plan.unspent)
            ))
            .text_color(muted),
        );
    }
    if plan.character < yellow.characters.len() {
        let character = &yellow.characters[plan.character];
        body = body.child(
            Caption::new(format!(
                "{}: Farming Lv {} · Summoning Lv {} → {} and {} steps for the two scaled goals.",
                character.name,
                whole(character.farming),
                whole(character.summoning),
                ((character.farming + 1.0) / 50.0).ceil(),
                ((character.summoning + 1.0) / 50.0).ceil(),
            ))
            .text_color(muted),
        );
    }
    body.into_any_element()
}
