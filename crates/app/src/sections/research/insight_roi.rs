//! Insight ROI: what one observation's next insight levels buy.
//!
//! Ported from the upstream `Insight ROI` tab (`js/analysis.js`,
//! `js/renderers/render-analysis.js`). One row per usable observation reports
//! how long its grind takes, the research EXP it gives up, the permanent
//! EXP/hour the new insight level adds and how long that gain needs to earn the
//! loss back. The analysis runs through the shared task runner, and the baseline
//! rate reads through the shared formula tree.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, SegmentedControl};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::Checkbox;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::insight::{Outcome, Row, Scenario, Sort};
use idleon_research::model::{Effort, coord};
use idleon_ui::format;
use idleon_ui::runner::TaskState;

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
    let breakpoint = crate::ui::layout::Breakpoint::of(window);
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

fn effort_id(effort: Effort) -> &'static str {
    match effort {
        Effort::Quick => "quick",
        Effort::Balanced => "balanced",
        Effort::Thorough => "thorough",
    }
}

/// The monocle count and the baseline rate, in the upstream summary's order.
fn monocle_line(result: &Outcome, colors: &ely_gpui_component::theme::Palette) -> AnyElement {
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
            .text_color(colors.success),
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
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let run_entity = entity.clone();
    let effort = effort_id(state.effort);
    let effort_entity = entity.clone();
    let effort_control = SegmentedControl::new("insight-effort", effort)
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
        "insight-run",
        if run.running {
            "Computing…"
        } else {
            "Compute ROI"
        },
    )
    .primary()
    .icon(IconName::Play)
    .disabled(run.running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.run_insight_roi(cx));
    });
    let result = run.result.as_ref();
    let mut body = div()
        .id("insight-summary")
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
    if let Some(result) = result {
        body = body.child(monocle_line(result, &colors));
        if result.monocles == 0 {
            let node = coord(91);
            body = body.child(Banner::new(
                "insight-no-monocle",
                Severity::Warning,
                format!(
                    "No Optical Monocles owned. Unlock grid node {node} (Optical Monocle) to use insight."
                ),
            ));
        }
    } else if !run.running {
        body = body.child(
            Caption::new(
                "Rank every usable observation by the research EXP its next insight levels buy back.",
            )
            .text_color(colors.fg_muted),
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
        .flex()
        .flex_wrap()
        .gap_3()
        .child(
            KpiCard::new(
                Statistic::new("insight-high-value", "High value", counts.high as f64).decimals(0),
            )
            .caption("over 10 EXP/hr per break-even hour"),
        )
        .child(
            KpiCard::new(
                Statistic::new("insight-moderate", "Moderate", counts.moderate as f64).decimals(0),
            )
            .caption("between 1 and 10 EXP/hr per hour"),
        )
        .child(
            KpiCard::new(
                Statistic::new("insight-low", "Low or none", counts.low as f64).decimals(0),
            )
            .caption("under 1, or no gain at all"),
        )
        .child(
            KpiCard::new(
                Statistic::new(
                    "insight-best-grind",
                    "Observations",
                    result.rows.len() as f64,
                )
                .decimals(0),
            )
            .caption(best),
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
    let colors = cx.theme().colors.clone();
    let mut body = div()
        .id("insight-baseline")
        .flex()
        .flex_col()
        .gap_2()
        .p_3()
        .child(
            Caption::new("The optimized magnifiers on the save's own shapes.")
                .text_color(colors.fg_muted),
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

/// The upstream triage buckets: rate gain per hour of break-even time.
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

/// The ranked table with its sort control and the hide-free filter.
fn table_card(
    state: &mut AppState,
    run: &TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let entity = cx.entity();
    let Some(result) = run.result.as_ref() else {
        return Card::new()
            .header(CardHeader::new("Insight ROI"))
            .child(
                div().id("insight-table-empty").p_3().child(
                    Caption::new("Run the analysis to rank the observations.")
                        .text_color(colors.fg_muted),
                ),
            )
            .into_any_element();
    };

    let sort_entity = entity.clone();
    let mut sort = SegmentedControl::new("insight-sort", state.research.insight_sort.id())
        .size(ControlSize::Sm);
    for mode in Sort::ALL {
        sort = sort.segment(mode.id(), mode.label(), None);
    }
    let sort = sort.on_change(move |value, _window, cx| {
        let mode = Sort::ALL
            .into_iter()
            .find(|mode| mode.id() == value.as_ref())
            .unwrap_or_default();
        sort_entity.update(cx, |state, cx| state.set_insight_sort(mode, cx));
    });
    let hide_entity = entity.clone();
    let hide = Checkbox::new("insight-hide-free", state.research.insight_hide_free)
        .label("Hide free")
        .on_change(move |hide, _window, cx| {
            hide_entity.update(cx, |state, cx| state.set_insight_hide_free(hide, cx));
        });

    let rows: Vec<&Row> = result
        .sorted(state.research.insight_sort)
        .into_iter()
        .filter(|row| keeps(row, state.research.insight_hide_free))
        .collect();
    let mut table = Table::new([
        "Observation",
        "Insight LV",
        "+LV",
        "Insight/hr",
        "Grind Time",
        "EXP/hr During",
        "EXP Lost",
        "Rate Gain",
        "Recoup",
        "Break-Even",
        "Efficiency",
    ]);
    for row in &rows {
        for (index, scenario) in row.scenarios.iter().enumerate() {
            table = table.row(scenario_row(row, scenario, index == 0, &colors));
        }
    }
    let summary = format!("{} of {} observations shown", rows.len(), result.rows.len());
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
                .p_3()
                .when(rows.is_empty(), |body| {
                    body.child(
                        Caption::new(
                            "Every row is hidden: no observation loses EXP while its insight grinds.",
                        )
                        .text_color(colors.fg_muted),
                    )
                })
                .child(div().min_h_0().child(table)),
        )
        .into_any_element()
}

/// Whether the hide-free filter keeps the row: the upstream filter drops rows
/// whose one-level grind loses no EXP.
fn keeps(row: &Row, hide_free: bool) -> bool {
    if !hide_free {
        return true;
    }
    let one = &row.scenarios[0];
    one.total_exp_lost.is_finite() && one.total_exp_lost > 0.0
}

/// One scenario line: the observation on the first row, then the numbers.
fn scenario_row(
    row: &Row,
    scenario: &Scenario,
    first: bool,
    colors: &ely_gpui_component::theme::Palette,
) -> [AnyElement; 11] {
    let muted = colors.fg_muted;
    let text = colors.fg;
    let insight = format!("{:.1}", scenario.insight_rate);
    let grind = format::projection(scenario.grind_hours);
    let during = format::rate(scenario.grind_rate);
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
    let recoup = format::projection(scenario.recoup_hours);
    let break_even = format::projection(scenario.break_even_hours);
    // The upstream text: a gain with no break-even to pay is forever ahead.
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
    let gain_color = if scenario.rate_gain > 0.0 {
        colors.success
    } else if scenario.rate_gain < 0.0 {
        colors.danger
    } else {
        muted
    };
    let break_even_color = if !scenario.worth {
        colors.danger
    } else if scenario.break_even_hours < 24.0 {
        colors.success
    } else if scenario.break_even_hours < 72.0 {
        colors.warning
    } else {
        text
    };
    let efficiency_color = if scenario.rate_gain <= 0.0 {
        colors.danger
    } else if idleon_research::insight::efficiency(scenario) > 10.0 {
        colors.success
    } else {
        muted
    };
    let cell = |id: String, child: String, color: gpui::Hsla| -> AnyElement {
        div()
            .id(id)
            .when(color != text, |cell| cell.text_color(color))
            .child(child)
            .into_any_element()
    };
    [
        div()
            .id(format!("insight-obs-{}", row.obs))
            .when(first, |cell| cell.font_weight(gpui::FontWeight::SEMIBOLD))
            .child(if first {
                row.name.clone()
            } else {
                String::new()
            })
            .into_any_element(),
        cell(
            format!("insight-level-{}-{}", row.obs, scenario.level_gain),
            if first {
                format!("{}", row.level)
            } else {
                String::new()
            },
            muted,
        ),
        cell(
            format!("insight-gain-{}-{}", row.obs, scenario.level_gain),
            format!("+{}", scenario.level_gain),
            colors.warning,
        ),
        cell(
            format!("insight-rate-{}-{}", row.obs, scenario.level_gain),
            insight,
            colors.accent,
        ),
        cell(
            format!("insight-grind-{}-{}", row.obs, scenario.level_gain),
            grind,
            text,
        ),
        cell(
            format!("insight-during-{}-{}", row.obs, scenario.level_gain),
            during,
            muted,
        ),
        cell(
            format!("insight-lost-{}-{}", row.obs, scenario.level_gain),
            lost,
            colors.danger,
        ),
        cell(
            format!("insight-rategain-{}-{}", row.obs, scenario.level_gain),
            gain,
            gain_color,
        ),
        cell(
            format!("insight-recoup-{}-{}", row.obs, scenario.level_gain),
            recoup,
            text,
        ),
        cell(
            format!("insight-breakeven-{}-{}", row.obs, scenario.level_gain),
            break_even,
            break_even_color,
        ),
        cell(
            format!("insight-efficiency-{}-{}", row.obs, scenario.level_gain),
            efficiency,
            efficiency_color,
        ),
    ]
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
    fn the_triage_buckets_follow_the_upstream_thresholds() {
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
        assert_eq!(Tab::from_id("insight"), Some(Tab::InsightRoi));
    }
}
