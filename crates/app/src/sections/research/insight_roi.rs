//! Insight ROI: what one observation's next insight levels buy.
//!
//! One row per usable observation reports how long its grind takes, the
//! research EXP it gives up, the permanent EXP/hour the new insight level adds
//! and how long that gain needs to earn the loss back. The analysis runs
//! through the shared task runner, and the baseline rate reads through the
//! shared formula tree.

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, FontWeight, Hsla, IntoElement, ParentElement, SharedString, Styled,
    Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::table::{Table, TableBody, TableCell, TableHead, TableHeader, TableRow};
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use idleon_research::insight::{Outcome, Row, Scenario, Sort};
use idleon_research::model::{Effort, coord};
use idleon_ui::format;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{Caption, Card, CardHeader, KpiStrip, Statistic, Tone};
use std::rc::Rc;

use super::controls::segmented;

/// The Insight ROI pane: the baseline summary and the ranked table.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let run = state.insight_state(cx);
    if let Some(result) = run.result.as_ref() {
        update_tree(state, result, cx);
    }
    let breakpoint = crate::ui::layout::Breakpoint::of(window, cx);
    div()
        .id("insight-roi")
        .flex()
        .flex_col()
        .gap_3()
        .flex_1()
        .min_h_0()
        .min_w_0()
        .child(kpi_row(state, &run, cx))
        .child(crate::ui::layout::split_pane(
            "insight-body",
            breakpoint,
            div()
                .id("insight-column")
                .flex()
                .flex_col()
                .min_w_0()
                .when(!breakpoint.is_compact(), |column| column.flex_1().min_h_0())
                .child(table_card(state, &run, cx)),
            div()
                .id("insight-baseline-column")
                .flex()
                .flex_col()
                .gap_3()
                .min_h_0()
                .child(baseline_card(state, &run, cx)),
            px(300.0),
        ))
}

/// Read the baseline rate through the shared formula tree, as the Rates pane
/// reads the save's own layout.
fn update_tree(state: &AppState, result: &Outcome, cx: &mut Context<AppState>) {
    let mut simulation = state.simulation.clone();
    simulation.mags = result.base_mags.clone();
    simulation.saved_overlay = Some(result.base_overlay.clone());
    let root = idleon_research::breakdown::exp(&simulation, &state.context);
    let tree = state.insight_breakdown.clone();
    tree.update(cx, |tree, cx| tree.set_root(root, cx));
}

/// The monocle count and the baseline rate, in the summary's order.
fn monocle_line(result: &Outcome, cx: &App) -> AnyElement {
    div()
        .id("insight-monocles")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Monocles: {} · Baseline {} EXP/hr ({})",
            result.monocles,
            format::rate(result.base_rate),
            format::exact(result.base_rate)
        ))
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .child(Caption::new(format!("Monocles: {}", result.monocles)))
        .child(
            Caption::new(format!(
                "Baseline EXP/hr: {} ({})",
                format::rate(result.base_rate),
                format::exact(result.base_rate)
            ))
            .text_color(cx.theme().success),
        )
        .into_any_element()
}

/// The row that pays back soonest, in the Observations card's caption.
fn best_row(row: &Row) -> String {
    let hours = row.scenarios[0].break_even_hours;
    let when = if !hours.is_finite() {
        "never pays back".to_string()
    } else if hours <= 0.0 {
        "instant".to_string()
    } else {
        format::projection(hours)
    };
    format!("{} ({when})", row.name)
}

/// The summary strip: the monocle count, the baseline rate and the triage counts.
fn kpi_row(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let run_entity = entity.clone();
    let effort_entity = entity.clone();
    let effort_control = segmented(
        "insight-effort",
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
    let button = Button::new("insight-run")
        .label(if run.running {
            "Computing…"
        } else {
            "Compute ROI"
        })
        .primary()
        .icon(IconName::Play)
        .disabled(run.running)
        .on_click(move |_event, _window, cx| {
            run_entity.update(cx, |state, cx| state.run_insight_roi(cx));
        });
    let result = run.result.as_ref();
    let mut body = div().id("insight-summary").flex().flex_col().gap_3().child(
        div()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(effort_control)
            .child(div().flex_1())
            .child(button),
    );
    if let Some(result) = result {
        body = body.child(monocle_line(result, cx));
        if result.monocles == 0 {
            let node = coord(91);
            body = body.child(
                Alert::new(
                    "insight-no-monocle",
                    format!(
                        "No Optical Monocles owned. Unlock grid node {node} (Optical Monocle) to use insight."
                    ),
                )
                .with_variant(Tone::Warning.into())
                .banner(),
            );
        }
    } else if !run.running {
        body = body.child(
            Caption::new(
                "Rank every usable observation by the research EXP its next insight levels buy back.",
            )
            .text_color(muted),
        );
    }
    if run.running {
        if let Some(progress) = run.progress.clone() {
            let cancel = entity.clone();
            body = body.child(idleon_ui::run_chrome::progress_card(
                "insight",
                "Computing insight ROI",
                "Runs off the UI thread; a cancelled run keeps the previous table.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_insight_roi(cx));
                },
            ));
        }
    } else if run.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "insight-cancelled",
            run.result.is_some(),
        ));
    }
    if run.stale {
        body = body
            .child(idleon_ui::run_chrome::stale_banner("insight-stale"))
            .child(idleon_ui::run_chrome::stale_badge("insight"));
    }
    let card = Card::new()
        .header(CardHeader::new("Insight ROI").description(
            "Time to level, research EXP lost during the grind, the permanent gain and the break-even",
        ))
        .child(body);
    let Some(result) = result else {
        return card.into_any_element();
    };
    let counts = triage(result);
    let best = result
        .sorted(Sort::BreakEven)
        .into_iter()
        .next()
        .map(best_row)
        .unwrap_or_else(|| "—".into());
    let summary = format!(
        "Monocles: {} · Baseline {} EXP/hr · {} observations · best break-even: {best}",
        result.monocles,
        format::rate(result.base_rate),
        result.rows.len()
    );
    let kpis = div()
        .id("insight-kpis")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .child(
            KpiStrip::new()
                .child(
                    Statistic::new(
                        "insight-high-value",
                        "High value",
                        format::count(counts.high as u64),
                    )
                    .caption("over 10 EXP/hr per break-even hour"),
                )
                .child(
                    Statistic::new(
                        "insight-moderate",
                        "Moderate",
                        format::count(counts.moderate as u64),
                    )
                    .caption("between 1 and 10 EXP/hr per hour"),
                )
                .child(
                    Statistic::new(
                        "insight-low",
                        "Low or none",
                        format::count(counts.low as u64),
                    )
                    .caption("under 1, or no gain at all"),
                )
                .child(
                    Statistic::new(
                        "insight-best-grind",
                        "Observations",
                        format::count(result.rows.len() as u64),
                    )
                    .caption(best),
                ),
        );
    div()
        .id("insight-summary-row")
        .flex()
        .flex_col()
        .gap_3()
        .child(card)
        .child(kpis)
        .into_any_element()
}

/// The baseline rate as the shared formula tree, in the side rail.
fn baseline_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let mut body = div()
        .id("insight-baseline")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            Caption::new("The optimized magnifiers on the save's own shapes.")
                .text_color(cx.theme().muted_foreground),
        );
    if run.result.is_some() {
        body = body.child(
            div()
                .h(px(220.0))
                .min_h_0()
                .child(state.insight_breakdown.clone()),
        );
    }
    Card::new()
        .header(
            CardHeader::new("Baseline EXP/hr").description("Every factor, searchable and copyable"),
        )
        .child(body)
        .into_any_element()
}

/// The triage buckets: rate gain per hour of break-even time.
#[derive(Clone, Copy, Debug, Default)]
struct Triage {
    high: usize,
    moderate: usize,
    low: usize,
}

fn triage(result: &Outcome) -> Triage {
    let mut triage = Triage::default();
    for row in &result.rows {
        let one = &row.scenarios[0];
        if one.rate_gain <= 0.0 || !one.break_even_hours.is_finite() || one.break_even_hours <= 0.0
        {
            triage.low += 1;
            continue;
        }
        let efficiency = one.rate_gain / one.break_even_hours;
        if efficiency > 10.0 {
            triage.high += 1;
        } else if efficiency > 1.0 {
            triage.moderate += 1;
        } else {
            triage.low += 1;
        }
    }
    triage
}

/// What the cached table rows were built from.
#[derive(PartialEq)]
pub(super) struct RowsKey {
    generation: u64,
    sort: Sort,
    hide_free: bool,
}

/// The rows the sort and the hide-free filter give, one per scenario, built
/// when the result or either control changes.
fn table_rows(state: &mut AppState, result: &Outcome) -> (Rc<Vec<ScenarioRow>>, usize) {
    let research = &mut state.research;
    let (sort, hide_free) = (research.insight_sort, research.insight_hide_free);
    let key = RowsKey {
        generation: research.run_generation,
        sort,
        hide_free,
    };
    let rows = research.insight_rows.get(key, || {
        result
            .sorted(sort)
            .into_iter()
            .filter(|row| keeps(row, hide_free))
            .flat_map(|row| {
                row.scenarios
                    .iter()
                    .enumerate()
                    .map(|(index, scenario)| ScenarioRow::new(row, scenario, index == 0))
            })
            .collect()
    });
    let shown = rows.iter().filter(|row| row.first).count();
    (rows, shown)
}

/// The ranked table with its sort control and the hide-free filter.
fn table_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let Some(result) = run.result.as_ref() else {
        return Card::new()
            .header(CardHeader::new("Insight ROI"))
            .child(div().id("insight-table-empty").child(
                Caption::new("Run the analysis to rank the observations.").text_color(muted),
            ))
            .into_any_element();
    };

    let sort_entity = entity.clone();
    let modes: Vec<(Sort, &'static str)> = Sort::ALL
        .into_iter()
        .map(|mode| (mode, mode.label()))
        .collect();
    let sort = segmented(
        "insight-sort",
        &modes,
        state.research.insight_sort,
        move |mode, _window, cx| {
            sort_entity.update(cx, |state, cx| state.set_insight_sort(mode, cx));
        },
    );
    let hide_entity = entity.clone();
    let hide = Checkbox::new("insight-hide-free")
        .label("Hide free")
        .checked(state.research.insight_hide_free)
        .on_click(move |hide, _window, cx| {
            let hide = *hide;
            hide_entity.update(cx, |state, cx| state.set_insight_hide_free(hide, cx));
        });

    let (rows, shown) = table_rows(state, result);
    let head = |label: &'static str| TableHead::new().child(label);
    let number = |label: &'static str| TableHead::new().text_right().child(label);
    let table = Table::new()
        .child(
            TableHeader::new().child(
                TableRow::new()
                    .child(head("Observation"))
                    .child(number("Insight LV"))
                    .child(number("+LV"))
                    .child(number("Insight/hr"))
                    .child(number("Grind Time"))
                    .child(number("EXP/hr During"))
                    .child(number("EXP Lost"))
                    .child(number("Rate Gain"))
                    .child(number("Recoup"))
                    .child(number("Break-Even"))
                    .child(number("Efficiency")),
            ),
        )
        .child(TableBody::new().children(rows.iter().map(|row| scenario_row(row, cx))));
    let summary = format!("{shown} of {} observations shown", result.rows.len());
    Card::new()
        .header(
            CardHeader::new("Insight ROI")
                .description("Permanent EXP/hour bought by each observation's next insight levels")
                .action(
                    div()
                        .flex()
                        .flex_wrap()
                        .items_center()
                        .gap_2()
                        .child(sort)
                        .child(hide),
                ),
        )
        .child(
            div()
                .id("insight-table")
                .role(gpui::Role::Label)
                .aria_label(summary)
                .flex()
                .flex_col()
                .gap_2()
                .when(rows.is_empty(), |body| {
                    body.child(
                        Caption::new(
                            "Every row is hidden: no observation loses EXP while its insight grinds.",
                        )
                        .text_color(muted),
                    )
                })
                .child(div().min_h_0().child(table)),
        )
        .into_any_element()
}

/// Whether the hide-free filter keeps the row: the filter drops rows
/// whose one-level grind loses no EXP.
fn keeps(row: &Row, hide_free: bool) -> bool {
    if !hide_free {
        return true;
    }
    let one = &row.scenarios[0];
    one.total_exp_lost.is_finite() && one.total_exp_lost > 0.0
}

/// The color a scenario cell draws in: a tone, or the body text.
#[derive(Clone, Copy, PartialEq)]
enum Ink {
    Body,
    Tone(Tone),
}

impl Ink {
    fn color(self, cx: &App) -> Hsla {
        match self {
            Self::Body => cx.theme().foreground,
            Self::Tone(tone) => tone.color(cx),
        }
    }
}

/// One scenario line, formatted when the table rows were built.
pub(super) struct ScenarioRow {
    obs: usize,
    level_gain: u32,
    /// The observation's first scenario names it; the later ones leave the
    /// name and level blank.
    first: bool,
    name: SharedString,
    /// The text and ink of the remaining cells, after the name, in column order.
    cells: [(SharedString, Ink); 10],
}

/// The cell ids after the name, in column order.
const CELL_IDS: [&str; 10] = [
    "insight-level",
    "insight-gain",
    "insight-rate",
    "insight-grind",
    "insight-during",
    "insight-lost",
    "insight-rategain",
    "insight-recoup",
    "insight-breakeven",
    "insight-efficiency",
];

impl ScenarioRow {
    fn new(row: &Row, scenario: &Scenario, first: bool) -> Self {
        let muted = Ink::Tone(Tone::Neutral);
        let lost = if scenario.total_exp_lost.is_finite() && scenario.total_exp_lost > 0.0 {
            format::rate(scenario.total_exp_lost)
        } else {
            "—".into()
        };
        let gain = format!(
            "{}{}",
            if scenario.rate_gain > 0.0 { "+" } else { "" },
            format::rate(scenario.rate_gain)
        );
        // A gain with no break-even to pay is forever ahead.
        let efficiency = if scenario.rate_gain <= 0.0 {
            "No gain".to_string()
        } else if scenario.break_even_hours == 0.0 {
            "∞".to_string()
        } else if scenario.break_even_hours.is_finite() {
            format!(
                "+{}",
                format::rate(idleon_research::insight::efficiency(scenario))
            )
        } else {
            "—".to_string()
        };
        let gain_ink = if scenario.rate_gain > 0.0 {
            Ink::Tone(Tone::Success)
        } else if scenario.rate_gain < 0.0 {
            Ink::Tone(Tone::Danger)
        } else {
            muted
        };
        let break_even_ink = if !scenario.worth {
            Ink::Tone(Tone::Danger)
        } else if scenario.break_even_hours < 24.0 {
            Ink::Tone(Tone::Success)
        } else if scenario.break_even_hours < 72.0 {
            Ink::Tone(Tone::Warning)
        } else {
            Ink::Body
        };
        let efficiency_ink = if scenario.rate_gain <= 0.0 {
            Ink::Tone(Tone::Danger)
        } else if idleon_research::insight::efficiency(scenario) > 10.0 {
            Ink::Tone(Tone::Success)
        } else {
            muted
        };
        Self {
            obs: row.obs,
            level_gain: scenario.level_gain,
            first,
            name: if first {
                row.name.clone()
            } else {
                String::new()
            }
            .into(),
            cells: [
                (
                    if first {
                        row.level.to_string()
                    } else {
                        String::new()
                    }
                    .into(),
                    muted,
                ),
                (
                    format!("+{}", scenario.level_gain).into(),
                    Ink::Tone(Tone::Warning),
                ),
                (
                    format!("{:.1}", scenario.insight_rate).into(),
                    Ink::Tone(Tone::Accent),
                ),
                (format::projection(scenario.grind_hours).into(), Ink::Body),
                (format::rate(scenario.grind_rate).into(), muted),
                (lost.into(), Ink::Tone(Tone::Danger)),
                (gain.into(), gain_ink),
                (format::projection(scenario.recoup_hours).into(), Ink::Body),
                (
                    format::projection(scenario.break_even_hours).into(),
                    break_even_ink,
                ),
                (efficiency.into(), efficiency_ink),
            ],
        }
    }
}

/// One scenario line: the observation on the first row, then the numbers.
fn scenario_row(row: &ScenarioRow, cx: &App) -> TableRow {
    let (obs, gain) = (row.obs, row.level_gain);
    // The name cell is blank on an observation's later rows; each row still
    // needs its own id, or the semantic tree drops them all.
    let name = div()
        .id(SharedString::from(if row.first {
            format!("insight-obs-{obs}")
        } else {
            format!("insight-obs-{obs}-{gain}")
        }))
        .when(row.first, |cell| cell.font_weight(FontWeight::SEMIBOLD))
        .child(row.name.clone());
    TableRow::new()
        .child(TableCell::new().child(name))
        .children(row.cells.iter().zip(CELL_IDS).map(|((text, ink), id)| {
            TableCell::new().text_right().child(
                div()
                    .id(SharedString::from(format!("{id}-{obs}-{gain}")))
                    .when(*ink != Ink::Body, |cell| cell.text_color(ink.color(cx)))
                    .child(text.clone()),
            )
        }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sections::research::Tab;
    use idleon_research::insight::{Outcome, Row};

    fn row(lost: f64, gain: f64, break_even: f64) -> Row {
        let scenario = Scenario {
            level_gain: 1,
            grind_hours: break_even,
            grind_rate: 0.0,
            exp_lost_per_hour: lost,
            total_exp_lost: lost,
            post_rate: gain,
            rate_gain: gain,
            recoup_hours: 0.0,
            break_even_hours: break_even,
            worth: gain > 0.0 && break_even.is_finite(),
            insight_rate: 1.0,
        };
        Row {
            obs: 0,
            name: "Bored Tree".into(),
            level: 1,
            scenarios: [scenario, scenario],
            grind_mags: Vec::new(),
            grind_overlay: None,
            insight_shapes: false,
            after_mags: Vec::new(),
            after_overlay: Vec::new(),
        }
    }

    #[test]
    fn the_hide_free_filter_drops_rows_that_lose_nothing() {
        assert!(keeps(&row(10.0, 1.0, 2.0), true));
        assert!(!keeps(&row(0.0, 1.0, 2.0), true));
        assert!(keeps(&row(0.0, 1.0, 2.0), false));
        assert!(!keeps(&row(f64::INFINITY, 1.0, f64::INFINITY), true));
    }

    #[test]
    fn the_triage_buckets_follow_their_thresholds() {
        let result = Outcome {
            monocles: 2,
            base_rate: 1.0,
            base_mags: Vec::new(),
            base_overlay: Vec::new(),
            rows: vec![
                row(10.0, 100.0, 1.0),
                row(10.0, 10.0, 5.0),
                row(10.0, 1.0, 5.0),
                row(0.0, -1.0, f64::INFINITY),
            ],
        };
        let triage = triage(&result);
        assert_eq!(triage.high, 1);
        assert_eq!(triage.moderate, 1);
        assert_eq!(triage.low, 2);
    }

    #[test]
    fn every_sort_mode_keeps_every_row() {
        let mut rows = vec![row(10.0, 1.0, 2.0), row(5.0, 20.0, 1.0), row(1.0, 2.0, 9.0)];
        rows[1].obs = 1;
        rows[2].obs = 2;
        let result = Outcome {
            monocles: 2,
            base_rate: 1.0,
            base_mags: Vec::new(),
            base_overlay: Vec::new(),
            rows,
        };
        for sort in Sort::ALL {
            assert_eq!(result.sorted(sort).len(), 3);
        }
        assert_eq!(
            result
                .sorted(Sort::GrindTime)
                .iter()
                .map(|row| row.obs)
                .collect::<Vec<_>>(),
            vec![1, 0, 2]
        );
        assert_eq!(
            result
                .sorted(Sort::RateGain)
                .iter()
                .map(|row| row.obs)
                .collect::<Vec<_>>(),
            vec![1, 2, 0]
        );
    }

    #[test]
    fn the_tab_is_named() {
        assert_eq!(Tab::InsightRoi.label(), "Insight ROI");
        assert_eq!(Tab::InsightRoi.id(), "insight");
    }
}
