//! The Yellow Point Allocator pane.
//!
//! The card draws one table under a "Points to spend" input and a goal select,
//! both re-planning on every change, keeps the character context the stat goals
//! scale with beside them, shows the selected goal's value against the saved
//! points, and offers the table and the context the plan was computed with.
//! Planning itself is [`idleon_cooking_mastery::Yellow::allocation`]; this
//! module only draws it.

use super::controls::Derived;
use super::{number_cell, table, text_cell, whole};
use crate::state::AppState;
use crate::ui;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::component::select::Select;
use gpui_kit::component::table::TableRow;
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_cooking_mastery::{Allocation, MealRow, Yellow};
use idleon_ui::widgets::{
    Callout, Caption, Card, CardHeader, PANEL_GAP, TEXT_KPI, Tone, labeled, number_width,
    show_number,
};

/// The pane: the controls, the plan's headline, its table and the context.
pub(crate) fn pane(
    state: &AppState,
    derived: &Derived,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(yellow) = &state.yellow else {
        return super::prompt(cx);
    };
    let muted = cx.theme().muted_foreground;
    // No plan means no goal has two leveled meals, the page's own notice.
    let Some(plan) = &derived.yellow else {
        return no_goals(yellow, muted);
    };
    let card = Card::new()
        .header(CardHeader::new("Yellow Point Allocator").description(
            "Every yellow point spent across the meals of one goal, where one more point \
                 buys the most.",
        ))
        .child(controls(state, yellow, derived, plan, window, cx))
        .child(headline(plan, yellow, muted))
        .child(sources(plan, muted))
        .child(rows(plan))
        .child(notes(plan, yellow, muted));
    v_flex()
        .id("cooking-mastery-yellow-pane")
        .gap(PANEL_GAP)
        .child(card)
        .into_any_element()
}

/// The page's notice when no goal has two leveled meals to compare.
fn no_goals(yellow: &Yellow, muted: gpui::Hsla) -> AnyElement {
    v_flex()
        .id("cooking-mastery-yellow")
        .role(Role::Label)
        .aria_label(format!(
            "No goals have at least two leveled meals to compare · {} yellow points available",
            whole(yellow.budget)
        ))
        .gap(PANEL_GAP)
        .child(
            Callout::new(
                Tone::Info,
                "No goals have at least two leveled meals to compare",
            )
            .child(Caption::new("Level up meals in these categories first.")),
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
fn controls(
    state: &AppState,
    yellow: &Yellow,
    derived: &Derived,
    plan: &Allocation,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let controls = &state.mastery_controls;
    show_number(&controls.yellow_budget, plan.spent, 0, window, cx);
    controls.yellow_goal.show(
        derived.yellow_goals.clone(),
        Some(&plan.goal.id),
        window,
        cx,
    );
    let mut row = h_flex()
        .id("cooking-mastery-yellow-controls")
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(labeled(
            "Points to Spend",
            h_flex()
                .gap_2()
                .child(
                    ui::number_field(
                        "cooking-mastery-yellow-budget",
                        &controls.yellow_budget,
                        "Points to Spend",
                    )
                    .w(number_width(&controls.yellow_budget, window, cx)),
                )
                .child(
                    Caption::new(format!("of {} total", whole(plan.available))).text_color(muted),
                ),
        ))
        .child(labeled(
            "Goal",
            div()
                .max_w_full()
                .w(controls.yellow_goal.width(window, cx))
                .child(
                    Select::new(controls.yellow_goal.state())
                        .id("cooking-mastery-yellow-goal")
                        .accessibility_label("Goal")
                        .small(),
                ),
        ));
    if !yellow.characters.is_empty() {
        controls.character.show(
            derived.characters.clone(),
            Some(&plan.character),
            window,
            cx,
        );
        row = row.child(labeled(
            "Character Context",
            div()
                .max_w_full()
                .w(controls.character.width(window, cx))
                .child(
                    Select::new(controls.character.state())
                        .id("cooking-mastery-yellow-character")
                        .accessibility_label("Character Context")
                        .small(),
                ),
        ));
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
    h_flex()
        .id("cooking-mastery-yellow")
        .role(Role::Label)
        .aria_label(label)
        .flex_wrap()
        .gap_3()
        .child(
            v_flex()
                .gap_1()
                .child(Caption::new("Optimal Meal Bonus").text_color(muted))
                .child(
                    div()
                        .id("cooking-mastery-yellow-value")
                        .text_size(TEXT_KPI)
                        .child(unit.format(plan.value)),
                ),
        )
        .child(
            v_flex()
                .gap_1()
                .child(
                    div()
                        .id("cooking-mastery-yellow-improvement")
                        .child(reading),
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
        .child(Caption::new(line).text_color(muted))
        .into_any_element()
}

/// The page's allocation table, most points first.
fn rows(plan: &Allocation) -> AnyElement {
    let multi_category = plan.goal.categories.len() > 1;
    let mut columns = vec![("Meal", false)];
    if multi_category {
        columns.push(("Category", false));
    }
    columns.extend([
        ("Effect", false),
        ("Meal Lv", true),
        ("Per Lv", true),
        ("Base", true),
        ("Points Now", true),
        ("Points Optimal", true),
        ("Mastery", true),
    ]);
    let rows = plan.rows.iter().map(|row| {
        let mut cells = vec![text_cell(row_name(row))];
        if multi_category {
            cells.push(text_cell(
                plan.goal.category_label(&row.category).to_string(),
            ));
        }
        cells.extend([
            text_cell(row.effect.clone()),
            number_cell(whole(row.level)),
            number_cell(format!("{}", row.bonus)),
            number_cell(format!("{:.2}", row.base_weight)),
            number_cell(whole(row.current)),
            number_cell(whole(row.allocated)),
            number_cell(format!("{:.3}x", row.mastery)),
        ]);
        TableRow::new()
            .id(("cooking-mastery-yellow-table-row", row.index))
            .children(cells)
    });
    div()
        .id("cooking-mastery-yellow-rows")
        .child(table(&columns, rows))
        .into_any_element()
}

/// The meal's name, with the points it takes in the accessible label.
fn row_name(row: &MealRow) -> impl IntoElement {
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
        .child(row.name.clone())
}

/// What the plan was computed with: the context the stat goals scale by, the
/// points left over and a budget the plan had to reject.
fn notes(plan: &Allocation, yellow: &Yellow, muted: gpui::Hsla) -> AnyElement {
    let mut body = v_flex().id("cooking-mastery-yellow-notes").gap_1();
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
        body =
            body.child(Callout::new(Tone::Warning, "Budget rejected").child(Caption::new(detail)));
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
