//! The all-combination ranking for the selected pool: every lure/line pair,
//! best first, with the objective switch, the selected/saved markers and the
//! `Use` action that applies a row to the toolbar.
//!
//! Ported from `renderRanking` in the upstream `fishing-calc.html`. The table
//! shows all 225 rows instead of the upstream top ten, because the section
//! contract asks for the all-combination table.

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::{Choice, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Pixels, Role, Styled, div, px};
use idleon_fishing::{self as fishing, Evaluation, Objective};
use idleon_ui::format;

/// The ranking card. `rows` is already sorted by `objective`.
pub(crate) fn card(
    state: &AppState,
    rows: &[Evaluation],
    evaluation: &Evaluation,
    table_height: Pixels,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let objective = state.fishing.objective;
    let objective_select = Select::new(
        "fishing-objective",
        [
            Choice::new("exp", Objective::Exp.label()),
            Choice::new("fish", Objective::Fish.label()),
        ],
    )
    .label("Rank by")
    .size(ControlSize::Sm)
    .selected(match objective {
        Objective::Exp => "exp",
        Objective::Fish => "fish",
    });
    let objective_entity = cx.entity();
    let objective_select = objective_select.on_change(move |value, _window, cx| {
        let objective = if value == "fish" {
            Objective::Fish
        } else {
            Objective::Exp
        };
        objective_entity.update(cx, |state, cx| state.set_fishing_objective(objective, cx));
    });

    let best = &rows[0];
    let reference = objective.metric(&evaluation.outcome);
    let best_metric = objective.metric(&best.outcome);
    let tied = fishing::ties(rows, objective);
    let selected = (evaluation.lure.idx, evaluation.line.idx);
    let best_selection = (best.lure.idx, best.line.idx);
    let is_best_selected = best_selection == selected;
    let saved = saved_selection(state);

    let mut table = Table::new(["#", "Lure", "Line", "EXP", "Fish", "vs selected", ""]);
    let entity = cx.entity();
    for (index, row) in rows.iter().enumerate() {
        // The rank the table prints, so an id and a row never disagree.
        let rank = index + 1;
        let selection = (row.lure.idx, row.line.idx);
        let is_selected = selection == selected;
        let is_saved = selection == saved;
        let metric = objective.metric(&row.outcome);
        let delta = if reference > 0.0 {
            (metric / reference - 1.0) * 100.0
        } else {
            0.0
        };
        let standing = if index == 0 {
            " · best"
        } else if metric == best_metric {
            " · tied for best"
        } else {
            ""
        };
        let mut rank_cell = div()
            .id(("fishing-rank", rank))
            .role(Role::Label)
            .aria_label(format!(
                "Rank {rank}: {} + {}{standing}",
                row.lure.name, row.line.name
            ))
            .flex()
            .items_center()
            .gap_2()
            .child(rank.to_string());
        if index == 0 {
            rank_cell = rank_cell.child(Badge::new("Best").tone(Tone::Success));
        } else if metric == best_metric {
            rank_cell = rank_cell.child(Badge::new("tie"));
        }
        let mut line = div()
            .id(("fishing-line-cell", rank))
            .flex()
            .items_center()
            .gap_2()
            .child(row.line.name.clone());
        if is_saved {
            line = line.child(Badge::new("saved").tone(Tone::Info));
        }
        if is_selected {
            line = line.child(Badge::new("selected").tone(Tone::Accent));
        }
        let action = if is_selected {
            div().into_any_element()
        } else {
            let (lure, line) = selection;
            let entity = entity.clone();
            Button::new(("fishing-use", rank), "Use")
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| state.use_fishing_toolkit(lure, line, cx));
                })
                .into_any_element()
        };
        let delta_style = if delta > 0.005 {
            Tone::Success
        } else if delta < -0.005 {
            Tone::Warning
        } else {
            Tone::Neutral
        };
        table = table.row([
            rank_cell.into_any_element(),
            div()
                .id(("fishing-lure-cell", rank))
                .child(row.lure.name.clone())
                .into_any_element(),
            line.into_any_element(),
            div()
                .id(("fishing-rank-exp", rank))
                .child(format::number(row.outcome.exp))
                .into_any_element(),
            div()
                .id(("fishing-rank-fish", rank))
                .child(format::number(row.outcome.fish_total))
                .into_any_element(),
            div()
                .id(("fishing-rank-delta", rank))
                .text_color(delta_color(delta_style, cx))
                .child(if delta.abs() < 0.005 {
                    "—".to_string()
                } else {
                    format!("{delta:+.1}%")
                })
                .into_any_element(),
            action,
        ]);
    }

    let tie_note = if tied > 1 {
        format!(
            "Ranks 1–{tied} share the top {}: the fish split and counts are floored, so equal depth and speed totals round to the same score. Ties rank by EXP, then fish, then lure and line order.",
            objective.unit()
        )
    } else if is_best_selected {
        "The selected lure + line is the best combination for this pool and depth.".to_string()
    } else {
        format!(
            "The best combination beats the selected one by {} {}.",
            format::number(best_metric - reference),
            objective.unit()
        )
    };

    Card::new()
        .header(
            CardHeader::new(format!("Best lure + line · {}", evaluation.location.label))
                .description(format!(
                    "All 225 lure/line combinations · ranked by {} · {}h claim · {} + {}",
                    objective.label(),
                    format::number(evaluation.outcome.hours),
                    best.lure.name,
                    best.line.name
                )),
        )
        .child(
            div()
                .id("fishing-ranking")
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_end()
                        .gap_4()
                        .child(objective_select)
                        .child(
                            div()
                                .id("fishing-best-toolkit")
                                .role(Role::Label)
                                .aria_label(format!(
                                    "Best lure + line: {} + {}",
                                    best.lure.name, best.line.name
                                ))
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_size(px(12.0))
                                .child(format!(
                                    "Best: {} + {}",
                                    best.lure.name, best.line.name
                                ))
                                .when(tied > 1, |row| {
                                    row.child(Badge::new(format!("{tied} tied")))
                                }),
                        )
                        .when(!is_best_selected, |row| {
                            let entity = entity.clone();
                            let (lure, line) = best_selection;
                            row.child(
                                Button::new("fishing-use-best", "Use best")
                                    .size(ControlSize::Sm)
                                    .on_click(move |_event, _window, cx| {
                                        entity.update(cx, |state, cx| {
                                            state.use_fishing_toolkit(lure, line, cx)
                                        });
                                    }),
                            )
                        }),
                )
                .child(
                    ScrollArea::new("fishing-ranking-scroll")
                        .vertical()
                        .h(table_height)
                        .child(table),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .border_t_1()
                        .border_color(cx.theme().colors.border)
                        .pt_3()
                        .child(
                            div()
                                .id("fishing-ranking-objective")
                                .role(Role::Label)
                                .aria_label(format!(
                                    "Ranking by {} · {} combinations",
                                    objective.label(),
                                    rows.len()
                                ))
                                .child(
                                    Caption::new(format!(
                                        "{} combinations · ranked by {}",
                                        rows.len(),
                                        objective.label()
                                    ))
                                    .text_color(muted),
                                ),
                        )
                        .child(
                            div()
                                .id("fishing-tie-note")
                                .role(Role::Label)
                                .aria_label(tie_note.clone())
                                .child(Caption::new(tie_note).text_color(muted)),
                        )
                        .child(
                            Caption::new(
                                "Ranking holds the character and fishing location fixed, because the location's depth bonus changes which toolkit is best. It covers every toolkit combination, including pieces this account may not have unlocked yet.",
                            )
                            .text_color(muted),
                        ),
                ),
        )
        .into_any_element()
}

/// The toolbar's saved lure and line, for the `saved` marker.
fn saved_selection(state: &AppState) -> (usize, usize) {
    state
        .fishing_save
        .character(state.fishing.character)
        .map(|character| (character.saved_lure, character.saved_line))
        .unwrap_or_default()
}

fn delta_color(tone: Tone, cx: &mut Context<AppState>) -> Hsla {
    let colors = &cx.theme().colors;
    match tone {
        Tone::Success => colors.success,
        Tone::Warning => colors.warning,
        _ => colors.fg_muted,
    }
}
