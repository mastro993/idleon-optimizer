//! Observation Unlocks: which undiscovered observation is worth rolling for.
//!
//! Ported from the upstream `Observation Unlocks` tab (`js/analysis.js`,
//! `js/renderers/render-analysis.js`). Each row is one observation the save has
//! not found yet: its roll requirement, the chance a day of rolls lands it, the
//! expected days to the first success and the research EXP/hour it adds. The
//! ranking runs through the shared task runner; the summary names the roll
//! range, the rolls a day and the failed-roll streak the probabilities read.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, SegmentedControl};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::SearchInput;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Palette};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::model::Effort;
use idleon_research::unlock::{Filter, Outcome, Priority, Row};
use idleon_ui::format;
use idleon_ui::runner::TaskState;

/// The Unlocks pane: the run controls, the roll summary and the ranked table.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let run = state.unlock_state(cx);
    ScrollArea::new("unlock-scroll")
        .vertical()
        .shadows()
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
                .child(kpi_row(&run, cx))
                .child(table_card(state, &run, cx)),
        )
}

/// The run control, the roll summary and what the save left out.
fn summary_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let run_entity = entity.clone();
    let effort_entity = entity.clone();
    let effort = match state.effort {
        Effort::Quick => "quick",
        Effort::Balanced => "balanced",
        Effort::Thorough => "thorough",
    };
    let effort_control = SegmentedControl::new("unlock-effort", effort)
        .size(ControlSize::Sm)
        .segment("quick", "Quick", None)
        .segment("balanced", "Balanced", None)
        .segment("thorough", "Thorough", None)
        .on_change(move |value, _window, cx| {
            let effort = match value.as_ref() {
                "quick" => Effort::Quick,
                "thorough" => Effort::Thorough,
                _ => Effort::Balanced,
            };
            effort_entity.update(cx, |state, cx| state.set_effort(effort, cx));
        });
    let button = Button::new(
        "unlock-run",
        if run.running {
            "Ranking…"
        } else {
            "Rank unlocks"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(run.running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_unlocks(cx));
    });
    let mut body = div()
        .id("unlock-summary")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(
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
        body = body.child(roll_line(result, &colors));
        for (index, missing) in result.missing.iter().enumerate() {
            body = body.child(Banner::new(
                format!("unlock-missing-{index}"),
                Severity::Warning,
                format!("Missing roll data — {missing}"),
            ));
        }
    } else if !run.running {
        body = body.child(
            Caption::new(
                "Rank the observations you have not found yet by the research EXP each adds per \
                 day of rolls.",
            )
            .text_color(colors.fg_muted),
        );
    }
    body = body.child(Banner::new(
        "unlock-experimental",
        Severity::Info,
        "Experimental estimates: they read the save's own roll data. Use your best judgement.",
    ));
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

/// The upstream summary line: the roll bounds, the pity, the rolls a day, the
/// failed-roll streak and the current rate.
fn roll_line(result: &Outcome, colors: &Palette) -> AnyElement {
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
        .child(Caption::new(text).text_color(colors.fg_muted))
        .into_any_element()
}

/// The triage strip: how many are left, how many the level can use, the best
/// pick and the rolls a day.
fn kpi_row(run: &TaskState<Outcome>, cx: &mut Context<AppState>) -> AnyElement {
    let Some(result) = run.result.as_ref() else {
        return div().id("unlock-kpis").into_any_element();
    };
    let colors = cx.theme().colors.clone();
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
    let summary = format!(
        "{} undiscovered · {} usable now · best: {best_row} · {per_day} rolls a day",
        result.rows.len(),
        usable
    );
    div()
        .id("unlock-kpis")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new(
                    "unlock-undiscovered",
                    "Undiscovered",
                    result.rows.len() as f64,
                )
                .decimals(0),
            )
            .caption("observations the save has not found"),
        )
        .child(
            KpiCard::new(Statistic::new("unlock-usable", "Usable now", usable as f64).decimals(0))
                .caption("research level already meets their requirement"),
        )
        .child(
            KpiCard::new(
                Statistic::new("unlock-per-day", "Rolls a day", f64::from(per_day)).decimals(0),
            )
            .caption("from Observationalistic, the event shop and Ring of Glory"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "unlock-best-chance",
                    "Best chance today",
                    result
                        .rows
                        .iter()
                        .filter_map(|row| row.chance_today)
                        .fold(0.0, f64::max)
                        * 100.0,
                )
                .decimals(0)
                .suffix("%"),
            )
            .caption(best_row),
        )
        .text_color(colors.fg)
        .into_any_element()
}

/// The ranked table with its availability filter and name search.
fn table_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let Some(result) = run.result.as_ref() else {
        return Card::new()
            .header(CardHeader::new("Observation Unlock Priority"))
            .child(
                div().id("unlock-table-empty").p_3().child(
                    Caption::new("Run the ranking to list the observations left to find.")
                        .text_color(colors.fg_muted),
                ),
            )
            .into_any_element();
    };

    let filter_entity = entity.clone();
    let mut filter = SegmentedControl::new("unlock-filter", state.research.unlock_filter.id())
        .size(ControlSize::Sm);
    for mode in Filter::ALL {
        filter = filter.segment(mode.id(), mode.label(), None);
    }
    let filter = filter.on_change(move |value, _window, cx| {
        let mode = Filter::ALL
            .into_iter()
            .find(|mode| mode.id() == value.as_ref())
            .unwrap_or_default();
        filter_entity.update(cx, |state, cx| state.set_unlock_filter(mode, cx));
    });
    let search_focus = state.research.unlock_search.read(cx).focus().clone();
    let query = state
        .research
        .unlock_search
        .read(cx)
        .text()
        .trim()
        .to_lowercase();
    let rows: Vec<&Row> = result
        .filtered(state.research.unlock_filter)
        .into_iter()
        .filter(|row| query.is_empty() || row.name.to_lowercase().contains(&query))
        .collect();
    let best = result.best_score();
    let mut table = Table::new([
        "Observation",
        "Roll Req",
        "Res LV Req",
        "P(unlock/day)",
        "E[days]",
        "EXP/hr Gain",
        "EXP/hr per E[day]",
        "Priority",
    ]);
    for row in &rows {
        table = table.row(row_cells(row, best, &colors));
    }
    let summary = format!("{} of {} observations shown", rows.len(), result.rows.len());
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
                                .on_click(move |_event, window, cx| {
                                    window.focus(&search_focus, cx);
                                })
                                .child(SearchInput::new(
                                    "unlock-search-field",
                                    &state.research.unlock_search,
                                )),
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
                .p_3()
                .when(rows.is_empty(), |body| {
                    body.child(
                        Caption::new(if result.rows.is_empty() {
                            "Every observation at this research level is already found."
                        } else {
                            "No observation matches the filter."
                        })
                        .text_color(colors.fg_muted),
                    )
                })
                .child(div().min_h_0().child(table)),
        )
        .into_any_element()
}

/// One row's cells: the observation, its requirements, its odds and its worth.
fn row_cells(row: &Row, best: f64, colors: &Palette) -> [AnyElement; 8] {
    let muted = colors.fg_muted;
    let level = if row.usable {
        format::count(row.required_level.max(0.0) as u64)
    } else {
        format!(
            "{} (wait)",
            format::count(row.required_level.max(0.0) as u64)
        )
    };
    let chance = match row.chance_today {
        Some(chance) => format!("{:.1}%", chance * 100.0),
        None => "—".to_string(),
    };
    let days = match row.expected_days {
        Some(days) if days.is_finite() => format!("{days:.1}d"),
        _ => "—".to_string(),
    };
    let gain = if row.exp_gain > 0.0 {
        format!("+{}", format::rate(row.exp_gain))
    } else {
        "—".to_string()
    };
    let score = row
        .score
        .filter(|_| row.exp_gain > 0.0)
        .map(format::rate)
        .unwrap_or_else(|| "—".to_string());
    let priority = row.priority(best);
    let priority_color = match priority {
        Priority::Best => colors.success,
        Priority::Good => colors.warning,
        Priority::Wait => colors.danger,
        Priority::Low | Priority::Minor => muted,
    };
    let chance_color = match row.chance_today {
        Some(chance) if chance < 0.01 => colors.danger,
        Some(chance) if chance < 0.5 => colors.warning,
        Some(_) => colors.fg,
        None => muted,
    };
    [
        div()
            .id(format!("unlock-obs-{}", row.obs))
            .child(row.name.clone())
            .into_any_element(),
        div()
            .child(format::count(row.threshold.max(0.0) as u64))
            .into_any_element(),
        div()
            .text_color(if row.usable {
                colors.success
            } else {
                colors.danger
            })
            .child(level)
            .into_any_element(),
        div()
            .id(format!("unlock-chance-{}", row.obs))
            .role(gpui::Role::Label)
            .aria_label(chance.clone())
            .text_color(chance_color)
            .child(chance)
            .into_any_element(),
        div().child(days).into_any_element(),
        div()
            .text_color(if row.exp_gain > 0.0 {
                colors.success
            } else {
                muted
            })
            .child(gain)
            .into_any_element(),
        div().text_color(colors.fg).child(score).into_any_element(),
        div()
            .id(format!("unlock-priority-{}", row.obs))
            .role(gpui::Role::Label)
            .aria_label(priority.label())
            .text_color(priority_color)
            .child(priority.label())
            .into_any_element(),
    ]
}

#[cfg(test)]
mod tests {
    use crate::sections::research::Tab;

    #[test]
    fn the_tab_is_named() {
        assert_eq!(Tab::Unlocks.label(), "Unlocks");
        assert_eq!(Tab::Unlocks.id(), "unlocks");
        assert_eq!(Tab::from_id("unlocks"), Some(Tab::Unlocks));
    }
}
