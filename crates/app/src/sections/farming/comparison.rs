//! The preset comparison: every built-in goal planned from the current
//! baseline, as the Land Rank tab's "Preset Comparison" table.
//!
//! The run plans all seven goals, so it goes through the shared task runner:
//! it reports progress per goal, cancels without dropping the previous table,
//! and a later baseline or crop-target edit marks the table stale.

use super::table::{Col, note, table, tag, text};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, App, Context, IntoElement, ParentElement, Styled, div};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_farming::{Allocation, Comparison, Objective, data};
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone};

/// The compare button, the run chrome and the per-goal table.
pub(super) fn comparison_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let task = state.farming_compare_task(cx);
    let running = task.running;
    let result = task.result.clone();
    let entity = cx.entity();
    let compare = entity.clone();
    let cancel = entity.clone();
    let toolbar = h_flex()
        .id("farming-compare-toolbar")
        .gap_3()
        .child(
            Button::new("farming-compare-run")
                .label(format!("Compare {} goals", Objective::ALL.len()))
                .primary()
                .small()
                .disabled(!state.save_loaded || running)
                .on_click(move |_event, _window, cx| {
                    compare.update(cx, |state, cx| state.compare_farming(cx));
                }),
        )
        .child(
            Button::new("farming-compare-cancel")
                .label("Cancel")
                .small()
                .disabled(!running)
                .on_click(move |_event, _window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_farming_compare(cx));
                }),
        )
        .child(
            div()
                .id("farming-compare-baseline")
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Baseline: {} · target crop #{}",
                    state.farming.mode.label(),
                    state.farming.target_crop
                ))
                .child(
                    Caption::new(format!(
                        "{} · target crop #{}",
                        state.farming.mode.label(),
                        state.farming.target_crop
                    ))
                    .text_color(muted),
                ),
        );

    let mut body = v_flex()
        .id("farming-comparison")
        .role(gpui::Role::Group)
        .aria_label("Preset comparison")
        .gap_3()
        .child(toolbar);

    if running {
        if let Some(progress) = task.progress.clone() {
            let on_cancel = entity.clone();
            body = body.child(run_chrome::progress_card(
                "farming-compare",
                "Comparing preset goals",
                "Every goal runs off the UI thread. Cancelling keeps the table this run replaced.",
                progress,
                move |_window, cx| {
                    on_cancel.update(cx, |state, cx| state.cancel_farming_compare(cx));
                },
            ));
        }
    } else if task.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "farming-compare-cancelled",
            result.is_some(),
        ));
    }
    if task.stale {
        body = body.child(run_chrome::stale_banner("farming-compare-stale"));
    }

    match &result {
        Some(comparison) => body = body.child(rows_table(comparison, state.farming.objective, cx)),
        None => {
            body = body.child(
                div()
                    .id("farming-compare-empty")
                    .role(gpui::Role::Label)
                    .aria_label("No comparison yet")
                    .child(
                        Caption::new(if state.save_loaded {
                            "Compare the goals to see what each preset would spend and gain from this baseline."
                        } else {
                            "Import a save to plan its Land Rank points for every preset goal."
                        })
                        .text_color(muted),
                    ),
            );
        }
    }
    Card::new()
        .header(
            CardHeader::new("Preset comparison")
                .description("Each preset goal's plan from the same baseline, largest moves first"),
        )
        .child(body)
        .into_any_element()
}

/// One row per goal: the gain, the points it spends and the rows it moves.
fn rows_table(comparison: &Comparison, current: Objective, cx: &App) -> AnyElement {
    let rows = comparison.rows.iter().map(|row| {
        let goal = row.objective.label();
        let gain = format::percent(row.relative_gain());
        let moves = largest_moves(row);
        [
            h_flex()
                .id(format!("farming-compare-{}", row.objective.id()))
                .role(gpui::Role::Label)
                .aria_label(format!("{goal}: {gain} · {moves}"))
                .gap_2()
                .child(goal)
                .children((row.objective == current).then(|| tag("Current", Tone::Accent)))
                .into_any_element(),
            text(gain),
            text(format!(
                "{} / {}",
                format::number(f64::from(row.points_spent)),
                format::number(f64::from(row.points_available))
            )),
            text(moves),
        ]
    });
    let table = table(
        "farming-compare",
        &[
            Col::Text("Goal"),
            Col::Number("Gain"),
            Col::Number("Points used"),
            Col::Text("Largest moves"),
        ],
        rows,
        cx,
    );
    v_flex()
        .id("farming-compare-rows")
        .gap_2()
        .child(table)
        .child(div().id("farming-compare-note").child(note(
            "Gains are relative to the saved levels each goal starts from; the moves name the Land Rank rows it raises.",
            cx,
        )))
        .into_any_element()
}

/// The "Largest Moves" column: the four biggest rows, or a note.
fn largest_moves(row: &Allocation) -> String {
    let moves: Vec<String> = row
        .changes
        .iter()
        .take(4)
        .map(|change| {
            let name = data()
                .rank_names
                .get(change.id)
                .cloned()
                .unwrap_or_else(|| format!("Row {}", change.id));
            format!("{name} +{}", change.delta())
        })
        .collect();
    if moves.is_empty() {
        "No positive move".into()
    } else {
        moves.join(", ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_farming::{Farm, Mode};

    fn farm() -> Farm {
        let mut farm = Farm {
            rank_ranks: vec![4; idleon_farming::PLOTS],
            ..Farm::default()
        };
        farm.plots[0].seed_family = 0;
        farm
    }

    #[test]
    fn the_largest_moves_name_the_biggest_rows() {
        let farm = farm();
        let comparison = farm
            .comparison(Mode::Reset, Some(240), &mut |_progress| {
                idleon_common::Control::Continue
            })
            .expect("the comparison completes");
        let farming_exp = comparison
            .rows
            .iter()
            .find(|row| row.objective == Objective::FarmingExp)
            .expect("every goal has a row");
        let moves = largest_moves(farming_exp);
        assert!(!moves.contains("No positive move"), "{moves}");
        assert!(moves.split(", ").count() <= 4, "{moves}");
    }

    #[test]
    fn a_goal_that_cannot_move_a_row_says_so() {
        let farm = Farm::default();
        let growth = farm.plan(Objective::Growth, Mode::Reset, Some(1));
        assert_eq!(largest_moves(&growth), "No positive move");
    }
}
