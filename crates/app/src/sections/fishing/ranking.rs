//! The all-combination ranking for the selected pool: every lure/line pair,
//! best first, with the objective switch, the selected/saved markers and the
//! `Use` action that applies a row to the toolbar.
//!
//! The table shows all 225 rows instead of a top ten, because the section
//! contract asks for the all-combination table. It is a virtualized
//! `DataTable` whose rows are built when the evaluation changes
//! (`docs/ui-kit/data.md`).

use super::controls::objective_choices;
use super::table;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontFeatures, IntoElement, ParentElement, Pixels, Role, SharedString,
    Styled, WeakEntity, Window, div, px,
};
use gpui_kit::component::button::Button;
use gpui_kit::component::select::Select;
use gpui_kit::component::table::{Column, DataTable, TableDelegate, TableState};
use gpui_kit::component::{ActiveTheme as _, Sizable as _, h_flex, v_flex};
use idleon_fishing::{self as fishing, Evaluation};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, ColumnFit, FitColumns, TEXT_MONO, Tone, fit_columns,
};
use std::sync::Arc;

/// Where a row stands against the best.
#[derive(Clone, Copy, PartialEq)]
enum Standing {
    Best,
    Tied,
    Behind,
}

/// One lure/line combination, formatted.
pub(crate) struct RankRow {
    rank: usize,
    label: SharedString,
    standing: Standing,
    lure: SharedString,
    line: SharedString,
    saved: bool,
    selected: bool,
    exp: SharedString,
    fish: SharedString,
    delta: SharedString,
    delta_tone: Tone,
    toolkit: (usize, usize),
}

/// The ranked combinations and the app their `Use` buttons edit.
pub(crate) struct Ranking {
    app: WeakEntity<AppState>,
    rows: Vec<RankRow>,
    fit: ColumnFit,
}

/// The ranking's table state, with no selection, sorting or column moves.
pub(crate) fn state(
    app: WeakEntity<AppState>,
    window: &mut Window,
    cx: &mut Context<TableState<Ranking>>,
) -> TableState<Ranking> {
    TableState::new(
        Ranking {
            app,
            rows: Vec::new(),
            fit: ColumnFit::default(),
        },
        window,
        cx,
    )
    .row_selectable(false)
    .col_selectable(false)
    .col_movable(false)
    .sortable(false)
}

impl FitColumns for Ranking {
    fn column_fit(&mut self) -> &mut ColumnFit {
        &mut self.fit
    }

    fn natural_column(&self, col_ix: usize, _: &App) -> Column {
        match col_ix {
            0 => Column::new("rank", "#").width(px(110.0)),
            1 => Column::new("lure", "Lure").width(px(170.0)),
            2 => Column::new("line", "Line").width(px(260.0)),
            3 => Column::new("exp", "EXP").width(px(100.0)).text_right(),
            4 => Column::new("fish", "Fish").width(px(100.0)).text_right(),
            5 => Column::new("delta", "vs selected")
                .width(px(110.0))
                .text_right(),
            _ => Column::new("use", "").width(px(80.0)),
        }
    }
}

impl TableDelegate for Ranking {
    fn columns_count(&self, _: &App) -> usize {
        7
    }

    fn rows_count(&self, _: &App) -> usize {
        self.rows.len()
    }

    fn column(&self, col_ix: usize, cx: &App) -> Column {
        self.fit.apply(self.natural_column(col_ix, cx))
    }

    fn render_td(
        &mut self,
        row_ix: usize,
        col_ix: usize,
        _: &mut Window,
        cx: &mut Context<TableState<Self>>,
    ) -> impl IntoElement {
        let Some(row) = self.rows.get(row_ix) else {
            return div().into_any_element();
        };
        let rank = row.rank;
        match col_ix {
            0 => h_flex()
                .id(("fishing-rank", rank))
                .role(Role::Label)
                .aria_label(row.label.clone())
                .gap_2()
                .child(rank.to_string())
                .when(row.standing == Standing::Best, |cell| {
                    cell.child(table::tag("Best", Tone::Success))
                })
                .when(row.standing == Standing::Tied, |cell| {
                    cell.child(table::tag("tie", Tone::Neutral))
                })
                .into_any_element(),
            1 => div()
                .id(("fishing-lure-cell", rank))
                .child(row.lure.clone())
                .into_any_element(),
            2 => h_flex()
                .id(("fishing-line-cell", rank))
                .gap_2()
                .child(row.line.clone())
                .when(row.saved, |cell| {
                    cell.child(table::tag("saved", Tone::Info))
                })
                .when(row.selected, |cell| {
                    cell.child(table::tag("selected", Tone::Accent))
                })
                .into_any_element(),
            3 => mono(("fishing-rank-exp", rank), row.exp.clone(), None, cx),
            4 => mono(("fishing-rank-fish", rank), row.fish.clone(), None, cx),
            5 => mono(
                ("fishing-rank-delta", rank),
                row.delta.clone(),
                Some(row.delta_tone.color(cx)),
                cx,
            ),
            _ if row.selected => div().into_any_element(),
            _ => {
                let (lure, line) = row.toolkit;
                let app = self.app.clone();
                Button::new(("fishing-use", rank))
                    .label("Use")
                    .xsmall()
                    .on_click(move |_event, _window, cx| {
                        app.update(cx, |state, cx| state.use_fishing_toolkit(lure, line, cx))
                            .ok();
                    })
                    .into_any_element()
            }
        }
    }

    fn cell_label(&self, row_ix: usize, col_ix: usize, _: &App) -> Option<SharedString> {
        let row = self.rows.get(row_ix)?;
        Some(match col_ix {
            0 => row.label.clone(),
            1 => row.lure.clone(),
            2 => row.line.clone(),
            3 => row.exp.clone(),
            4 => row.fish.clone(),
            5 => row.delta.clone(),
            _ => "Use".into(),
        })
    }

    fn cell_text(&self, row_ix: usize, col_ix: usize, cx: &App) -> String {
        self.cell_label(row_ix, col_ix, cx)
            .map(String::from)
            .unwrap_or_default()
    }
}

/// A number in the mono face with tabular figures, under `id`.
fn mono(
    id: (&'static str, usize),
    text: SharedString,
    color: Option<gpui::Hsla>,
    cx: &App,
) -> AnyElement {
    div()
        .id(id)
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(TEXT_MONO)
        .font_features(FontFeatures(Arc::new(vec![("tnum".into(), 1)])))
        .when_some(color, |cell, color| cell.text_color(color))
        .child(text)
        .into_any_element()
}

/// What the card says about the ranking, formatted once per evaluation.
pub(crate) struct Summary {
    title: SharedString,
    description: SharedString,
    best: SharedString,
    tied: usize,
    best_toolkit: Option<(usize, usize)>,
    count: usize,
    objective: SharedString,
    tie_note: SharedString,
}

/// Format the ranking once and hand its rows to the table.
pub(crate) fn fill(
    state: &AppState,
    rows: &[Evaluation],
    evaluation: &Evaluation,
    cx: &mut App,
) -> Summary {
    let objective = state.fishing.objective;
    let best = &rows[0];
    let reference = objective.metric(&evaluation.outcome);
    let best_metric = objective.metric(&best.outcome);
    let tied = fishing::ties(rows, objective);
    let selected = (evaluation.lure.idx, evaluation.line.idx);
    let best_selection = (best.lure.idx, best.line.idx);
    let is_best_selected = best_selection == selected;
    let saved = saved_selection(state);
    let table_rows = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            // The rank the table prints, so an id and a row never disagree.
            let rank = index + 1;
            let toolkit = (row.lure.idx, row.line.idx);
            let metric = objective.metric(&row.outcome);
            let delta = if reference > 0.0 {
                (metric / reference - 1.0) * 100.0
            } else {
                0.0
            };
            let standing = if index == 0 {
                Standing::Best
            } else if metric == best_metric {
                Standing::Tied
            } else {
                Standing::Behind
            };
            RankRow {
                rank,
                label: format!(
                    "Rank {rank}: {} + {}{}",
                    row.lure.name,
                    row.line.name,
                    match standing {
                        Standing::Best => " · best",
                        Standing::Tied => " · tied for best",
                        Standing::Behind => "",
                    }
                )
                .into(),
                standing,
                lure: row.lure.name.clone().into(),
                line: row.line.name.clone().into(),
                saved: toolkit == saved,
                selected: toolkit == selected,
                exp: format::number(row.outcome.exp).into(),
                fish: format::number(row.outcome.fish_total).into(),
                delta: if delta.abs() < 0.005 {
                    "—".to_string()
                } else {
                    format!("{delta:+.1}%")
                }
                .into(),
                delta_tone: if delta > 0.005 {
                    Tone::Success
                } else if delta < -0.005 {
                    Tone::Warning
                } else {
                    Tone::Neutral
                },
                toolkit,
            }
        })
        .collect();
    state.fishing_controls.ranking.update(cx, |table, cx| {
        table.delegate_mut().rows = table_rows;
        cx.notify();
    });
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
    Summary {
        title: format!("Best lure + line · {}", evaluation.location.label).into(),
        description: format!(
            "All 225 lure/line combinations · ranked by {} · {}h claim · {} + {}",
            objective.label(),
            format::number(evaluation.outcome.hours),
            best.lure.name,
            best.line.name
        )
        .into(),
        best: format!("{} + {}", best.lure.name, best.line.name).into(),
        tied,
        best_toolkit: (!is_best_selected).then_some(best_selection),
        count: rows.len(),
        objective: objective.label().into(),
        tie_note: tie_note.into(),
    }
}

/// The ranking card.
pub(crate) fn card(
    state: &AppState,
    summary: &Summary,
    table_height: Pixels,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let theme = cx.theme();
    let (muted, border) = (theme.muted_foreground, theme.border);
    let controls = &state.fishing_controls;
    controls.objective.show(
        objective_choices(),
        Some(&state.fishing.objective),
        window,
        cx,
    );
    let entity = cx.entity();
    let use_best = summary.best_toolkit.map(|(lure, line)| {
        Button::new("fishing-use-best")
            .label("Use best")
            .small()
            .on_click(move |_event, _window, cx| {
                entity.update(cx, |state, cx| state.use_fishing_toolkit(lure, line, cx));
            })
    });
    Card::new()
        .header(CardHeader::new(summary.title.clone()).description(summary.description.clone()))
        .child(
            v_flex()
                .id("fishing-ranking")
                .gap_3()
                .child(
                    h_flex()
                        .gap_4()
                        .child(
                            div().w(px(160.0)).child(
                                Select::new(controls.objective.state())
                                    .id("fishing-objective")
                                    .accessibility_label("Rank by")
                                    .small(),
                            ),
                        )
                        .child(
                            h_flex()
                                .id("fishing-best-toolkit")
                                .role(Role::Label)
                                .aria_label(format!("Best lure + line: {}", summary.best))
                                .gap_2()
                                .text_size(TEXT_MONO)
                                .child(format!("Best: {}", summary.best))
                                .when(summary.tied > 1, |row| {
                                    row.child(table::tag(
                                        format!("{} tied", summary.tied),
                                        Tone::Neutral,
                                    ))
                                }),
                        )
                        .children(use_best),
                )
                .child(
                    div()
                        .id("fishing-ranking-table")
                        .h(table_height)
                        .child(fit_columns(DataTable::new(&controls.ranking), &controls.ranking)),
                )
                .child(
                    v_flex()
                        .gap_1()
                        .border_t_1()
                        .border_color(border)
                        .pt_3()
                        .child(
                            div()
                                .id("fishing-ranking-objective")
                                .role(Role::Label)
                                .aria_label(format!(
                                    "Ranking by {} · {} combinations",
                                    summary.objective, summary.count
                                ))
                                .child(
                                    Caption::new(format!(
                                        "{} combinations · ranked by {}",
                                        summary.count, summary.objective
                                    ))
                                    .text_color(muted),
                                ),
                        )
                        .child(
                            div()
                                .id("fishing-tie-note")
                                .role(Role::Label)
                                .aria_label(summary.tie_note.clone())
                                .child(Caption::new(summary.tie_note.clone()).text_color(muted)),
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
