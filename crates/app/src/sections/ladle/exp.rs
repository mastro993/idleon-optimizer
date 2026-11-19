//! The Cooking EXP tab: one balloon at a time, with the overflow carried.
//!
//! [`figures`] works out the table and the balloon tree when the rates or the
//! balloon controls change; render only formats what it kept.

use super::table::{blank, table, text};
use super::{State, summary::tag};
use crate::state::AppState;
use crate::ui::number_field;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div, px};
use gpui_kit::component::Sizable as _;
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::select::Select;
use idleon_common::FormulaNode;
use idleon_ladle::{
    self as ladle, EXP_BALLOONS, Ladle, balloons_to_level, levels_from_balloons, split_balloons,
};
use idleon_ui::format;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, TableColumn, Tone, show_number};

/// One character's row, formatted once.
#[derive(Clone, Debug, Default)]
pub(super) struct Row {
    name: String,
    cooking: bool,
    level: String,
    progress: String,
    multi: String,
    per_hour: String,
    per_balloon: String,
    next: String,
    target: String,
    afk: String,
    reached: String,
    /// The balloons the split gives the character, when the split is on.
    spent: Option<String>,
}

/// The table's rows and totals.
#[derive(Clone, Debug, Default)]
pub(super) struct Figures {
    rows: Vec<Row>,
    next_total: f64,
    target_total: f64,
    owned: f64,
    /// The balloons the split hands out, when the split is on.
    split_used: Option<f64>,
    /// The balloons the count could not read, when it is a floor.
    partial: Option<String>,
}

/// The table for the section's balloon controls, and the chosen character's
/// balloon tree.
pub(super) fn figures(state: &State, ladle: &Ladle) -> (Figures, Option<FormulaNode>) {
    let stats = &state.derived.stats;
    let balloon = state.balloon_key();
    let count = ladle.balloons.get(state.balloon);
    let owned = count.map_or(0.0, |count| count.value);
    let target_level = state.target_level;
    let split = state.split.then(|| {
        let entries: Vec<Option<(f64, f64)>> =
            stats.iter().map(|row| Some((row.level, row.exp))).collect();
        split_balloons(&entries, owned, state.panel.exp_multi, balloon.item_id)
    });
    let tree = stats.get(state.chosen()).map(|row| {
        row.balloons_tree(
            balloon,
            owned,
            target_choice(target_level, row.level),
            split.as_ref(),
        )
    });
    let mut figures = Figures {
        owned,
        split_used: split.as_ref().map(|split| split.used),
        partial: count
            .filter(|count| count.partial)
            .map(|count| count.missing.join(", ")),
        ..Figures::default()
    };
    for row in stats {
        let target = target_choice(target_level, row.level);
        let next = row.next_level_balloons;
        let to_target = match target {
            Some(target) => {
                balloons_to_level(row.level, row.exp, target, row.exp_multi, balloon.item_id)
                    .balloons
            }
            None => next,
        };
        let spend = match &split {
            Some(split) => match split.characters.get(row.index) {
                Some(Some(share)) => (share.level, share.used),
                _ => (row.level, 0.0),
            },
            None => {
                let all =
                    levels_from_balloons(row.level, row.exp, owned, row.exp_multi, balloon.item_id);
                (all.level, all.used)
            }
        };
        // AFK time to the target: the banked EXP pays some of it first.
        let needed = match target {
            Some(target) => exp_to_level(row.level, row.exp, target),
            None => (row.exp_req - row.exp).max(0.0),
        };
        let afk_hours = match row.exp_per_hour > 0.0 {
            true => needed / row.exp_per_hour,
            false => f64::INFINITY,
        };
        figures.next_total += next;
        figures.target_total += to_target;
        let fraction = match row.exp_req > 0.0 {
            true => (row.exp / row.exp_req).clamp(0.0, 1.0),
            false => 0.0,
        };
        figures.rows.push(Row {
            name: row.name.clone(),
            cooking: row.cooking,
            level: format::number(row.level),
            progress: format!(
                "{}% · {} / {}",
                (fraction * 100.0).round(),
                format::rate(row.exp),
                format::rate(row.exp_req)
            ),
            multi: format!("×{:.2}", row.exp_multi),
            per_hour: format::rate(row.exp_per_hour),
            per_balloon: format::number(ladle::balloon_gain(
                balloon.item_id,
                row.level,
                row.exp_multi,
            )),
            next: format::number(next),
            target: match target {
                Some(target) => format!("Lv {target}: {}", format::number(to_target)),
                None => format!(
                    "Lv {}: {}",
                    format::number(row.level + 1.0),
                    format::number(to_target)
                ),
            },
            afk: format::projection(afk_hours),
            reached: match spend.0 > row.level {
                true => format!(
                    "{} → {}",
                    format::number(row.level),
                    format::number(spend.0)
                ),
                false => format::number(row.level),
            },
            spent: split
                .is_some()
                .then(|| format!("{} balloons", format::number(spend.1))),
        });
    }
    (figures, tree)
}

/// The whole Cooking EXP tab.
pub(crate) fn pane(
    state: &AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    div()
        .id("ladle-exp-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .flex()
                .flex_col()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(controls(state, ladle, window, cx))
                .child(table_card(state, cx))
                .child(super::breakdowns::panel(
                    "ladle-breakdown-balloons",
                    "Cooking EXP balloons",
                    "Item ID × (Lv + 1) × EXP multi, one at a time with overflow",
                    &state.ladle_state.balloons_tree,
                ))
                .overflow_y_scrollbar()
                .id("ladle-exp-scroll"),
        )
        .into_any_element()
}

/// The balloon, the target level and the split toggle.
fn controls(
    state: &AppState,
    ladle: &Ladle,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let ladle_state = &state.ladle_state;
    let controls = &ladle_state.controls;
    let choices: Vec<Choice<usize>> = EXP_BALLOONS
        .iter()
        .enumerate()
        .map(|(index, balloon)| {
            let count = ladle
                .balloons
                .get(index)
                .map(|count| count.value)
                .unwrap_or(0.0);
            Choice::new(
                index,
                format!("{} ({})", balloon.label, format::number(count)),
            )
        })
        .collect();
    controls
        .balloon
        .show(choices, Some(&ladle_state.balloon), window, cx);
    show_number(
        &controls.target_level,
        f64::from(ladle_state.target_level),
        0,
        window,
        cx,
    );
    let split_entity = cx.entity();
    div()
        .id("ladle-exp-controls")
        .flex()
        .items_end()
        .gap_3()
        .child(
            super::field("EXP balloon").child(
                div().flex_none().w(px(240.0)).child(
                    Select::new(controls.balloon.state())
                        .id("ladle-balloon")
                        .accessibility_label("EXP balloon")
                        .small(),
                ),
            ),
        )
        .child(
            super::field("Target Cooking level (0 = next)").child(number_field(
                "ladle-target-level",
                &controls.target_level,
                "Target Cooking level",
            )),
        )
        .child(
            Checkbox::new("ladle-split")
                .label("Share the owned balloons to keep levels even")
                .checked(ladle_state.split)
                .on_click(move |on, _window, cx| {
                    let on = *on;
                    split_entity.update(cx, |state, cx| {
                        state.ladle_state.split = on;
                        cx.notify();
                    });
                }),
        )
        .into_any_element()
}

/// The per-character balloon table.
fn table_card(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let figures = &state.ladle_state.derived.exp;
    let split = figures.split_used.is_some();
    let cols = [
        TableColumn::text("Character"),
        TableColumn::number("Cook Lv"),
        TableColumn::text("EXP progress"),
        TableColumn::number("EXP multi"),
        TableColumn::number("EXP / hr"),
        TableColumn::number("EXP / balloon"),
        TableColumn::number("Balloons → next Lv"),
        TableColumn::text("Balloons → target"),
        TableColumn::number("AFK time → target"),
        TableColumn::text(match split {
            true => "Lv with split share",
            false => "Lv if given all owned",
        }),
    ];
    let mut rows: Vec<Vec<AnyElement>> = figures
        .rows
        .iter()
        .map(|row| {
            let mut name = div().flex().items_center().gap_2().child(row.name.clone());
            if row.cooking {
                name = name.child(tag("cooking", Tone::Success));
            }
            let reached = div()
                .flex()
                .flex_col()
                .child(row.reached.clone())
                .children(row.spent.clone().map(Caption::new));
            vec![
                name.into_any_element(),
                text(row.level.clone()),
                text(row.progress.clone()),
                text(row.multi.clone()),
                text(row.per_hour.clone()),
                text(row.per_balloon.clone()),
                text(row.next.clone()),
                text(row.target.clone()),
                text(row.afk.clone()),
                reached.into_any_element(),
            ]
        })
        .collect();
    rows.push(vec![
        text("All characters"),
        blank(),
        blank(),
        blank(),
        blank(),
        blank(),
        text(format::number(figures.next_total)),
        text(format::number(figures.target_total)),
        blank(),
        text(format!(
            "{}{} owned",
            match figures.split_used {
                Some(used) => format!("{} of ", format::number(used)),
                None => String::new(),
            },
            format::number(figures.owned)
        )),
    ]);
    Card::new()
        .header(CardHeader::new("Cooking EXP").description(
            "Balloons add item ID × (Cooking Lv + 1) × EXP multi, one at a time with overflow",
        ))
        .child(
            div()
                .id("ladle-exp-table")
                .role(Role::Label)
                .aria_label(format!(
                    "{} characters, {} balloons owned",
                    figures.rows.len(),
                    format::number(figures.owned)
                ))
                .child(table("ladle-exp-rows", &cols, rows, cx)),
        )
        .child(Caption::new(
            "Each balloon is read at the current level, so the overflow lands on the next one; a \
             character with banked EXP may need none at all.",
        ))
        .children(figures.partial.as_ref().map(|missing| {
            Caption::new(format!("The count is a floor: {missing} could not be read"))
        }))
        .into_any_element()
}

/// The target the balloon tables plan for: the entered level, or the next one.
pub(crate) fn target_choice(target_level: u32, level: f64) -> Option<f64> {
    match f64::from(target_level) > level {
        true => Some(f64::from(target_level)),
        false => None,
    }
}

/// The EXP between the current level and `target`.
pub(crate) fn exp_to_level(level: f64, exp: f64, target: f64) -> f64 {
    let mut need = -exp;
    let mut each = level;
    while each < target {
        need += ladle::cooking_exp_req(each);
        each += 1.0;
    }
    need.max(0.0)
}
