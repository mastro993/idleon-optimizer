//! The preset comparison: every built-in goal planned from the current
//! baseline, as the upstream Land Rank tab's "Preset Comparison" table.
//!
//! The run plans all seven goals, so it goes through the shared task runner:
//! it reports progress per goal, cancels without dropping the previous table,
//! and a later baseline or crop-target edit marks the table stale.

use crate::state::AppState;
use crate::ui::format;
use crate::ui::task;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, div, px};
use idleon_core::farming::{Allocation, Comparison, Objective, data};

/// The compare button, the run chrome and the per-goal table.
pub(super) fn comparison_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.farming_compare_task(cx);
    let running = task.running;
    let result = task.result.clone();
    let entity = cx.entity();
    let compare = entity.clone();
    let cancel = entity.clone();
    let toolbar = div()
        .id("farming-compare-toolbar")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(
            Button::new(
                "farming-compare-run",
                format!("Compare {} goals", Objective::ALL.len()),
            )
            .variant(ButtonVariant::Primary)
            .size(ControlSize::Sm)
            .disabled(!state.save_loaded || running)
            .on_click(move |_event, _window, cx| {
                compare.update(cx, |state, cx| state.compare_farming(cx));
            }),
        )
        .child(
            Button::new("farming-compare-cancel", "Cancel")
                .size(ControlSize::Sm)
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

    let mut body = div()
        .id("farming-comparison")
        .role(gpui::Role::Group)
        .aria_label("Preset comparison")
        .flex()
        .flex_col()
        .gap_3()
        .px_4()
        .pb_4()
        .child(toolbar);

    if running {
        if let Some(progress) = task.progress.clone() {
            let on_cancel = entity.clone();
            body = body.child(task::progress_card(
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
        body = body.child(task::cancelled_banner(
            "farming-compare-cancelled",
            result.is_some(),
        ));
    }
    if task.stale {
        body = body.child(task::stale_banner("farming-compare-stale"));
    }

    match &result {
        Some(comparison) => {
            body = body.child(rows_table(comparison, state.farming.objective, muted))
        }
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
            CardHeader::new("Preset comparison").description(
                "Each upstream goal's plan from the same baseline, largest moves first",
            ),
        )
        .child(body)
        .into_any_element()
}

/// One row per goal: the gain, the points it spends and the rows it moves.
fn rows_table(comparison: &Comparison, current: Objective, muted: gpui::Hsla) -> AnyElement {
    let mut table = Table::new(["Goal", "Gain", "Points used", "Largest moves"]);
    for row in &comparison.rows {
        let goal = row.objective.label();
        let gain = format::percent(row.relative_gain());
        let moves = largest_moves(row);
        table = table.row([
            div()
                .id(format!("farming-compare-{}", row.objective.id()))
                .role(gpui::Role::Label)
                .aria_label(format!("{goal}: {gain} · {moves}"))
                .flex()
                .items_center()
                .gap_2()
                .child(goal)
                .children(
                    (row.objective == current).then(|| Badge::new("Current").tone(Tone::Accent)),
                )
                .into_any_element(),
            div().child(gain).into_any_element(),
            div()
                .child(format!(
                    "{} / {}",
                    format::number(f64::from(row.points_spent)),
                    format::number(f64::from(row.points_available))
                ))
                .into_any_element(),
            div().child(moves).into_any_element(),
        ]);
    }
    div()
        .id("farming-compare-rows")
        .flex()
        .flex_col()
        .gap_2()
        .child(table)
        .child(
            div()
                .id("farming-compare-note")
                .text_size(px(11.0))
                .text_color(muted)
                .child(
                    "Gains are relative to the saved levels each goal starts from; the moves name the Land Rank rows it raises.",
                ),
        )
        .into_any_element()
}

/// The upstream "Largest Moves" column: the four biggest rows, or a note.
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
    use idleon_core::farming::{Farm, Mode};

    fn farm() -> Farm {
        let mut farm = Farm {
            rank_ranks: vec![4; idleon_core::farming::PLOTS],
            ..Farm::default()
        };
        farm.plots[0].seed_family = 0;
        farm
    }

    #[test]
    fn the_largest_moves_name_the_upstream_rows() {
        let farm = farm();
        let comparison = farm
            .comparison(Mode::Reset, Some(240), &mut |_progress| {
                idleon_core::model::Control::Continue
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
