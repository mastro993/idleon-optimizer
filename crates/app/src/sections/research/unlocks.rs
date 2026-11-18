//! Observation Unlocks: which undiscovered observation is worth rolling for.
//!
//! Each row is one observation the save has not found yet: its roll
//! requirement, the chance a day of rolls lands it, the expected days to the
//! first success and the research EXP/hour it adds. The ranking runs through
//! the shared task runner; the summary names the roll range, the rolls a day
//! and the failed-roll streak the probabilities read.

use crate::state::AppState;
use gpui::Focusable as _;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use idleon_research::model::Effort;
use idleon_research::unlock::{Filter, Outcome, Priority, Row};
use idleon_ui::format;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone, search_input};
use std::rc::Rc;

use super::controls::segmented;

/// The Unlocks pane: the run controls, the roll summary and the ranked table.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let run = state.unlock_state(cx);
    div()
        .flex_1()
        .min_h_0()
        .child(
            div()
                .id("unlocks")
                .flex()
                .flex_col()
                .gap_3()
                .min_w_0()
                .child(summary_card(state, &run, cx))
                .child(kpi_row(&run))
                .child(table_card(state, &run, cx)),
        )
        .overflow_y_scrollbar()
        .id("unlock-scroll")
}

/// The run control, the roll summary and what the save left out.
fn summary_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let run_entity = entity.clone();
    let effort_entity = entity.clone();
    let effort_control = segmented(
        "unlock-effort",
        &[
            (Effort::Quick, "Quick"),
            (Effort::Balanced, "Balanced"),
            (Effort::Thorough, "Thorough"),
        ],
        state.effort,
        move |effort, _window, cx| {
            effort_entity.update(cx, |state, cx| state.set_effort(effort, cx));
        },
    );
    let button = Button::new("unlock-run")
        .label(if run.running {
            "Ranking…"
        } else {
            "Rank unlocks"
        })
        .primary()
        .icon(IconName::Play)
        .disabled(run.running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_unlocks(cx));
        });
    let mut body = div().id("unlock-summary").flex().flex_col().gap_3().child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(effort_control)
            .child(div().flex_1())
            .child(button),
    );
    if let Some(result) = run.result.as_ref() {
        body = body.child(roll_line(result));
        for (index, missing) in result.missing.iter().enumerate() {
            let text = format!("Missing roll data — {missing}");
            // The wrapper keeps the Ely banner's id and reads its message.
            body = body.child(
                div()
                    .id(SharedString::from(format!("unlock-missing-{index}")))
                    .role(gpui::Role::Label)
                    .aria_label(text.clone())
                    .child(
                        Alert::new(("unlock-missing-alert", index), text)
                            .with_variant(Tone::Warning.into())
                            .banner(),
                    ),
            );
        }
    } else if !run.running {
        body = body.child(
            Caption::new(
                "Rank the observations you have not found yet by the research EXP each adds per \
                 day of rolls.",
            )
            .text_color(muted),
        );
    }
    body = body.child(
        Alert::new(
            "unlock-experimental",
            "Experimental estimates: they read the save's own roll data. Use your best judgement.",
        )
        .with_variant(Tone::Info.into())
        .banner(),
    );
    if run.running {
        if let Some(progress) = run.progress.clone() {
            let cancel = entity.clone();
            body = body.child(idleon_ui::run_chrome::progress_card(
                "unlock",
                "Ranking observation unlocks",
                "Runs off the UI thread; a cancelled run keeps the previous table.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_unlocks(cx));
                },
            ));
        }
    } else if run.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "unlock-cancelled",
            run.result.is_some(),
        ));
    }
    if run.stale {
        body = body
            .child(idleon_ui::run_chrome::stale_banner("unlock-stale"))
            .child(idleon_ui::run_chrome::stale_badge("unlock"));
    }
    Card::new()
        .header(CardHeader::new("Observation Unlocks").description(
            "Chance a day of rolls lands each undiscovered observation, and the EXP/hour it adds",
        ))
        .child(body)
        .into_any_element()
}

/// The summary line: the roll bounds, the pity, the rolls a day, the
/// failed-roll streak and the current rate.
fn roll_line(result: &Outcome) -> AnyElement {
    let rolls = result.rolls;
    let mut text = format!(
        "Roll range: {}–{} · Rolls/day: {} · Current EXP/hr: {}",
        format::count(rolls.min_roll.max(0.0) as u64),
        format::count(rolls.max_roll.max(0.0) as u64),
        rolls.per_day,
        format::rate(result.current_rate)
    );
    if rolls.smart_eye > 0 {
        text.push_str(&format!(
            " · Smart Eye pity +{}/fail, cap +{}",
            rolls.smart_eye, rolls.smart_eye_cap
        ));
    }
    if let Some(remaining) = rolls.remaining {
        text.push_str(&format!(" · Rolls left today: {remaining}"));
    }
    if let Some(failed) = rolls.failed {
        text.push_str(&format!(
            " · Failed streak: {}",
            format::count(failed as u64)
        ));
    }
    div()
        .id("unlock-rolls")
        .role(gpui::Role::Label)
        .aria_label(text.clone())
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(Caption::new(text))
        .into_any_element()
}

/// The triage strip: how many are left, how many the level can use, the best
/// pick and the rolls a day.
fn kpi_row(run: &TaskState<Outcome>) -> AnyElement {
    let Some(result) = run.result.as_ref() else {
        return div().id("unlock-kpis").into_any_element();
    };
    let usable = result.rows.iter().filter(|row| row.usable).count();
    let best_row = if result.rows.iter().any(|row| row.chance_today.is_some()) {
        result
            .rows
            .iter()
            .max_by(|left, right| {
                left.chance_today
                    .unwrap_or(0.0)
                    .total_cmp(&right.chance_today.unwrap_or(0.0))
            })
            .map(|row| row.name.clone())
            .unwrap_or_else(|| "—".into())
    } else {
        "not estimated without the save's roll data".to_string()
    };
    let per_day = result.rolls.per_day;
    let best_chance = result
        .rows
        .iter()
        .filter_map(|row| row.chance_today)
        .fold(0.0, f64::max)
        * 100.0;
    let summary = format!(
        "{} undiscovered · {} usable now · best: {best_row} · {per_day} rolls a day",
        result.rows.len(),
        usable
    );
    div()
        .id("unlock-kpis")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "unlock-undiscovered",
                        "Undiscovered",
                        format::count(result.rows.len() as u64),
                    )
                    .caption("observations the save has not found"),
                )
                .child(
                    Statistic::new("unlock-usable", "Usable now", format::count(usable as u64))
                        .caption("research level already meets their requirement"),
                )
                .child(
                    Statistic::new(
                        "unlock-per-day",
                        "Rolls a day",
                        format::count(u64::from(per_day)),
                    )
                    .caption("from Observationalistic, the event shop and Ring of Glory"),
                )
                .child(
                    Statistic::new(
                        "unlock-best-chance",
                        "Best chance today",
                        format!("{best_chance:.0}"),
                    )
                    .suffix("%")
                    .caption(best_row),
                ),
        )
        .into_any_element()
}

/// What the cached table rows were built from.
#[derive(PartialEq)]
pub(super) struct RowsKey {
    generation: u64,
    filter: Filter,
    query: String,
}

/// One table row, formatted when its result, filter or search last changed.
pub(super) struct UnlockRow {
    obs: usize,
    name: SharedString,
    threshold: SharedString,
    level: SharedString,
    usable: bool,
    chance: SharedString,
    /// `None` draws the chance in the body color.
    chance_tone: Option<Tone>,
    days: SharedString,
    gain: SharedString,
    gainful: bool,
    score: SharedString,
    priority: Priority,
}

impl UnlockRow {
    fn new(row: &Row, best: f64) -> Self {
        let threshold = format::count(row.threshold.max(0.0) as u64);
        let level = format::count(row.required_level.max(0.0) as u64);
        Self {
            obs: row.obs,
            name: row.name.clone().into(),
            threshold: threshold.into(),
            level: if row.usable {
                level
            } else {
                format!("{level} (wait)")
            }
            .into(),
            usable: row.usable,
            chance: match row.chance_today {
                Some(chance) => format!("{:.1}%", chance * 100.0),
                None => "—".to_string(),
            }
            .into(),
            chance_tone: match row.chance_today {
                Some(chance) if chance < 0.01 => Some(Tone::Danger),
                Some(chance) if chance < 0.5 => Some(Tone::Warning),
                Some(_) => None,
                None => Some(Tone::Neutral),
            },
            days: match row.expected_days {
                Some(days) if days.is_finite() => format!("{days:.1}d"),
                _ => "—".to_string(),
            }
            .into(),
            gain: if row.exp_gain > 0.0 {
                format!("+{}", format::rate(row.exp_gain))
            } else {
                "—".to_string()
            }
            .into(),
            gainful: row.exp_gain > 0.0,
            score: row
                .score
                .filter(|_| row.exp_gain > 0.0)
                .map(format::rate)
                .unwrap_or_else(|| "—".to_string())
                .into(),
            priority: row.priority(best),
        }
    }
}

/// The rows the filter and the name search keep, in the result's order.
fn table_rows(state: &mut AppState, result: &Outcome, cx: &App) -> Rc<Vec<UnlockRow>> {
    let research = &mut state.research;
    let key = RowsKey {
        generation: research.run_generation,
        filter: research.unlock_filter,
        query: research
            .unlock_search
            .read(cx)
            .value()
            .trim()
            .to_lowercase(),
    };
    let best = result.best_score();
    let (filter, query) = (key.filter, key.query.clone());
    research.unlock_rows.get(key, || {
        result
            .filtered(filter)
            .into_iter()
            .filter(|row| query.is_empty() || row.name.to_lowercase().contains(&query))
            .map(|row| UnlockRow::new(row, best))
            .collect()
    })
}

/// The ranked table with its availability filter and name search.
fn table_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let Some(result) = run.result.as_ref() else {
        return Card::new()
            .header(CardHeader::new("Observation Unlock Priority"))
            .child(
                div().id("unlock-table-empty").child(
                    Caption::new("Run the ranking to list the observations left to find.")
                        .text_color(muted),
                ),
            )
            .into_any_element();
    };

    let filter_entity = entity.clone();
    let modes: Vec<(Filter, &'static str)> = Filter::ALL
        .into_iter()
        .map(|mode| (mode, mode.label()))
        .collect();
    let filter = segmented(
        "unlock-filter",
        &modes,
        state.research.unlock_filter,
        move |mode, _window, cx| {
            filter_entity.update(cx, |state, cx| state.set_unlock_filter(mode, cx));
        },
    );
    let rows = table_rows(state, result, cx);
    let head = |label: &'static str| TableHead::new().child(label);
    let number = |label: &'static str| TableHead::new().text_right().child(label);
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(head("Observation"))
                    .child(number("Roll Req"))
                    .child(number("Res LV Req"))
                    .child(number("P(unlock/day)"))
                    .child(number("E[days]"))
                    .child(number("EXP/hr Gain"))
                    .child(number("EXP/hr per E[day]"))
                    .child(head("Priority")),
            ),
        )
        .child(TableBody::new().children(rows.iter().map(|row| row_cells(row, cx))));
    let summary = format!("{} of {} observations shown", rows.len(), result.rows.len());
    let search_focus = state.research.unlock_search.focus_handle(cx);
    Card::new()
        .header(
            CardHeader::new("Observation Unlock Priority")
                .description("Higher priority is a bigger reward for easier rolls")
                .action(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(filter)
                        .child(
                            div()
                                .id("unlock-search")
                                .role(gpui::Role::Search)
                                .aria_label("Find an observation")
                                .w(px(220.0))
                                .on_click(move |_event, window, cx| window.focus(&search_focus, cx))
                                .child(
                                    search_input(&state.research.unlock_search)
                                        .aria_label("Find an observation"),
                                ),
                        ),
                ),
        )
        .child(
            div()
                .id("unlock-table")
                .role(gpui::Role::Label)
                .aria_label(summary)
                .flex()
                .flex_col()
                .gap_2()
                .when(rows.is_empty(), |body| {
                    body.child(
                        Caption::new(if result.rows.is_empty() {
                            "Every observation at this research level is already found."
                        } else {
                            "No observation matches the filter."
                        })
                        .text_color(muted),
                    )
                })
                .child(div().min_h_0().child(table)),
        )
        .into_any_element()
}

/// One row's cells: the observation, its requirements, its odds and its worth.
fn row_cells(row: &UnlockRow, cx: &App) -> TableRow {
    let theme = cx.theme();
    let priority_color = match row.priority {
        Priority::Best => theme.success,
        Priority::Good => theme.warning,
        Priority::Wait => theme.danger,
        Priority::Low | Priority::Minor => theme.muted_foreground,
    };
    let chance_color = row
        .chance_tone
        .map_or(theme.foreground, |tone| tone.color(cx));
    TableRow::new()
        .child(
            TableCell::new().child(
                div()
                    .id(SharedString::from(format!("unlock-obs-{}", row.obs)))
                    .child(row.name.clone()),
            ),
        )
        .child(TableCell::new().text_right().child(row.threshold.clone()))
        .child(
            TableCell::new()
                .text_right()
                .text_color(if row.usable {
                    theme.success
                } else {
                    theme.danger
                })
                .child(row.level.clone()),
        )
        .child(
            TableCell::new().text_right().child(
                div()
                    .id(SharedString::from(format!("unlock-chance-{}", row.obs)))
                    .role(gpui::Role::Label)
                    .aria_label(row.chance.clone())
                    .text_color(chance_color)
                    .child(row.chance.clone()),
            ),
        )
        .child(TableCell::new().text_right().child(row.days.clone()))
        .child(
            TableCell::new()
                .text_right()
                .text_color(if row.gainful {
                    theme.success
                } else {
                    theme.muted_foreground
                })
                .child(row.gain.clone()),
        )
        .child(
            TableCell::new()
                .text_right()
                .text_color(theme.foreground)
                .child(row.score.clone()),
        )
        .child(
            TableCell::new().child(
                div()
                    .id(SharedString::from(format!("unlock-priority-{}", row.obs)))
                    .role(gpui::Role::Label)
                    .aria_label(row.priority.label())
                    .text_color(priority_color)
                    .child(row.priority.label()),
            ),
        )
}

#[cfg(test)]
mod tests {
    use crate::sections::research::Tab;

    #[test]
    fn the_tab_is_named() {
        assert_eq!(Tab::Unlocks.label(), "Unlocks");
        assert_eq!(Tab::Unlocks.id(), "unlocks");
    }
}
