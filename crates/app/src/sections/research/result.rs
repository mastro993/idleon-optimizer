//! The Run tab's result: the strategy comparison, the timeline, the EXP/hour
//! chart and the expandable step-by-step actions.
//!
//! The timeline and the chart are both read from the projection's samples: the
//! timeline as the segments between two events, the chart as the rate at each.
//! A phase's action carries what it ground out, and the first one carries the
//! optimizer's own steps, so a click opens the same work the run just did.

use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::charts::{LineChart, Series};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{Disclosure, Severity};
use ely_gpui_component::theme::{ActiveTheme as _, Palette};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Entity, Hsla, IntoElement, ParentElement, Role, Styled, div, px};
use idleon_research::model::{OptimizeResult, Strategy, Target};
use idleon_research::sim::{self, Event, Projection, Sample};
use idleon_research::timeline::{self, Phase};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// The Result card: what the last run found, or why there is nothing yet.
pub(crate) fn card(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let run = state.optimize_state(cx);
    let mut body = div()
        .id("optimizer-result-body")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0();

    if run.running {
        if let Some(progress) = run.progress.clone() {
            let cancel = cx.entity();
            body = body.child(run_chrome::progress_card(
                "run",
                "Optimizing",
                "Runs off the UI thread. Cancelling keeps the result this run replaced.",
                progress,
                move |_window, cx| {
                    cancel.update(cx, |state, cx| state.cancel_optimize(cx));
                },
            ));
        }
    } else if run.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "run-cancelled",
            run.result.is_some(),
        ));
    }

    let Some(result) = run.result else {
        if !run.running {
            body = body.child(
                Card::new()
                    .header(CardHeader::new("Result"))
                    .child(
                        div().p_3().flex().flex_col().gap_2().child(Banner::new(
                            "result-empty",
                            Severity::Info,
                            "Nothing yet. Pick a target and press Optimize; the run happens in this process, so it can be thorough.",
                        )),
                    ),
            );
        }
        return body;
    };

    let gain = result.gain_pct();
    let tone = if gain > 0.0 {
        Tone::Success
    } else {
        Tone::Neutral
    };
    let stale = run.stale;
    let entity = cx.entity();
    let phases = timeline::phases(&result.best.projection);
    let actions = actions(&result, &phases);
    let selected = state
        .research
        .run_phase
        .filter(|index| *index < actions.len());
    let colors = &cx.theme().colors;

    let apply = entity.clone();
    let dismiss = entity.clone();

    let mut trailing = div().flex().items_center().gap_2();
    if stale {
        trailing = trailing.child(run_chrome::stale_badge("result-stale"));
    }
    trailing = trailing.child(
        div()
            .id("result-gain")
            .role(Role::Label)
            .aria_label(format!("{gain:+.1}% EXP/hr"))
            .child(Badge::new(format!("{gain:+.1}% EXP/hr")).tone(tone)),
    );

    let mut card = Card::new().header(
        CardHeader::new("Result")
            .description(format!(
                "{} events · {} steps · {} ms",
                phases.len(),
                result.steps.len(),
                result.elapsed_ms
            ))
            .action(trailing),
    );
    if stale {
        card = card.child(run_chrome::stale_banner("result-stale"));
    }
    body.child(
        card.child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .min_h_0()
                .child(
                    div()
                        .flex()
                        .flex_wrap()
                        .gap_4()
                        .child(metric(
                            "result-before",
                            "Before",
                            format::rate(result.before_rate),
                        ))
                        .child(metric(
                            "result-after",
                            "After",
                            format::rate(result.after_rate),
                        ))
                        .child(metric(
                            "result-projection",
                            "Projected",
                            format!(
                                "{} · level {}",
                                format::hours(result.best.projection.hours),
                                result.best.projection.final_level
                            ),
                        ))
                        .child(metric(
                            "result-insight",
                            "Insight levels",
                            format!("{}", result.best.projection.insight_gained),
                        )),
                )
                .child(comparison(&result))
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("apply-result", "Apply layout")
                                .primary()
                                .disabled(stale)
                                .on_click(move |_event, _window, cx| {
                                    apply.update(cx, |state, cx| state.apply_result(cx));
                                }),
                        )
                        .child(Button::new("dismiss-result", "Dismiss").on_click(
                            move |_event, _window, cx| {
                                dismiss.update(cx, |state, cx| state.dismiss_result(cx));
                            },
                        )),
                )
                .child(timeline(&phases, selected, entity.clone(), colors))
                .child(rate_chart(&result.best.projection.samples))
                .child(div().h(px(1.0)).bg(colors.border))
                .child(
                    ScrollArea::new("actions-scroll")
                        .child(action_list(&actions, selected, entity, colors)),
                ),
        ),
    )
}

/// One row of the actions list: a timeline phase, or the initial layout when
/// the run recorded no events at all.
struct Action {
    event: Event,
    title: &'static str,
    start: f64,
    rate: f64,
    lines: Vec<String>,
}

/// The actions the timeline opens, oldest first. The first one carries the work
/// the optimizers did to the layout; the rest carry what each phase ground out.
fn actions(result: &OptimizeResult, phases: &[Phase]) -> Vec<Action> {
    if phases.is_empty() {
        let rate = result
            .best
            .projection
            .samples
            .first()
            .map_or(0.0, |sample| sample.rate);
        return vec![Action {
            event: Event::Start,
            title: "Initial optimization",
            start: 0.0,
            rate,
            lines: result.steps.clone(),
        }];
    }
    phases
        .iter()
        .enumerate()
        .map(|(index, phase)| {
            let mut lines = phase_lines(phase);
            if index == 0 {
                lines.extend(result.steps.iter().cloned());
            }
            Action {
                event: phase.event,
                title: event_label(phase.event),
                start: phase.start,
                rate: phase.rate,
                lines,
            }
        })
        .collect()
}

/// What a phase ground out, then what its closing event changed.
fn phase_lines(phase: &Phase) -> Vec<String> {
    let mut lines = vec![format!(
        "Held {} at {} EXP/hr: {} EXP.",
        format::hours(phase.hours),
        format::rate(phase.rate),
        format::value(phase.exp)
    )];
    if phase.to_level > phase.from_level {
        lines.push(format!(
            "Research level {} → {}.",
            phase.from_level, phase.to_level
        ));
    }
    if phase.insight > 0 {
        lines.push(format!("{} insight levels collected.", phase.insight));
    }
    if phase.bonus > 0.0 {
        lines.push(format!(
            "The tournament added {} of research time: {} EXP.",
            format::hours(sim::TOURNAMENT_BONUS_HOURS),
            format::value(phase.bonus)
        ));
    }
    lines
}

/// The best and worst layouts the run tested, and the lead between them.
fn comparison(result: &OptimizeResult) -> AnyElement {
    let lead = result.lead();
    div()
        .id("strategy-comparison")
        .flex()
        .flex_wrap()
        .items_start()
        .gap_4()
        .child(strategy(
            "strategy-best",
            "Best",
            &result.best,
            result.target,
        ))
        .child(strategy(
            "strategy-worst",
            "Worst",
            &result.worst,
            result.target,
        ))
        .when_some(lead, |row, lead| {
            row.child(
                div()
                    .id("strategy-lead")
                    .flex()
                    .flex_col()
                    .gap_0p5()
                    .child(Caption::new("Best vs worst"))
                    .child(
                        Badge::new(format!(
                            "{} faster · {:.1}%",
                            format::hours(lead.hours),
                            lead.pct
                        ))
                        .tone(Tone::Success),
                    ),
            )
        })
        .into_any_element()
}

fn strategy(
    id: &'static str,
    heading: &'static str,
    strategy: &Strategy,
    target: Target,
) -> impl IntoElement {
    let outcome = strategy_outcome(target, &strategy.projection);
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(format!("{heading} · {}: {outcome}", strategy.label))
        .flex()
        .flex_col()
        .gap_0p5()
        .child(Caption::new(format!("{heading} · {}", strategy.label)))
        .child(div().text_size(px(14.0)).child(outcome))
}

/// How one tested layout fared against the target.
fn strategy_outcome(target: Target, projection: &Projection) -> String {
    match target {
        Target::Level(level) if projection.final_level < level => format!(
            "stalls at level {} after {}",
            projection.final_level,
            format::hours(projection.hours)
        ),
        Target::Level(_) => format!("reaches the level in {}", format::hours(projection.hours)),
        Target::Hours(_) => format!(
            "{} EXP in {}",
            format::value(projection.total_exp),
            format::hours(projection.hours)
        ),
    }
}

/// The run across the timeline: one segment per phase, widest for the longest
/// stretch, coloured by the event that opened it. A press opens its action.
fn timeline(
    phases: &[Phase],
    selected: Option<usize>,
    entity: Entity<AppState>,
    colors: &Palette,
) -> AnyElement {
    if phases.is_empty() {
        return Caption::new("The run finished before its first event, so there is no timeline.")
            .into_any_element();
    }
    let mut segments = div()
        .id("run-timeline")
        .flex()
        .w_full()
        .h(px(14.0))
        .rounded(px(4.0))
        .overflow_hidden();
    for (index, phase) in phases.iter().enumerate() {
        let toggle = entity.clone();
        let open = selected == Some(index);
        segments = segments.child(
            div()
                .id(("run-timeline-phase", index))
                .role(Role::Button)
                .aria_label(format!(
                    "{} at {} · {} EXP/hr",
                    event_label(phase.event),
                    format::hours(phase.start),
                    format::rate(phase.rate)
                ))
                .flex_basis(px(0.0))
                .flex_grow(phase.hours.max(0.0) as f32)
                .h_full()
                .bg(event_color(phase.event, colors).opacity(if open { 1.0 } else { 0.45 }))
                .cursor_pointer()
                .on_click(move |_event, _window, cx| {
                    toggle.update(cx, |state, cx| state.toggle_run_phase(index, cx));
                }),
        );
    }
    div()
        .id("run-timeline-section")
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .child(segments)
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(11.0))
                .text_color(colors.fg_muted)
                .child(format::hours(0.0))
                .child(format!("{} phases", phases.len()))
                .child(format::hours(phases.last().map_or(0.0, Phase::end))),
        )
        .into_any_element()
}

/// The EXP/hour of the run at each of its samples: the same points the timeline
/// is cut from.
fn rate_chart(samples: &[Sample]) -> AnyElement {
    if samples.len() < 2 || samples.iter().any(|sample| !sample.rate.is_finite()) {
        return Caption::new("This run has no rate curve to draw.").into_any_element();
    }
    let labels: Vec<String> = samples
        .iter()
        .map(|sample| format::hours(sample.hours))
        .collect();
    let rates: Vec<f64> = samples.iter().map(|sample| sample.rate).collect();
    LineChart::new("projection-chart", labels)
        .series(Series::new("EXP/hr", rates))
        .format(format::rate)
        .h(px(160.0))
        .into_any_element()
}

/// The expandable actions, oldest first; the open one is the phase the timeline
/// marks.
fn action_list(
    actions: &[Action],
    selected: Option<usize>,
    entity: Entity<AppState>,
    colors: &Palette,
) -> AnyElement {
    let mut list = div().id("run-actions").flex().flex_col();
    for (index, action) in actions.iter().enumerate() {
        let open = selected == Some(index);
        let toggle = entity.clone();
        let mut row = div()
            .id(("run-action", index))
            .role(Role::Button)
            .aria_label(format!(
                "{} at {} · {} EXP/hr",
                action.title,
                format::hours(action.start),
                format::rate(action.rate)
            ))
            .flex()
            .flex_col()
            .gap_1()
            .py_2()
            .pl_2()
            .border_l_2()
            .border_color(event_color(action.event, colors))
            .cursor_pointer()
            .on_click(move |_event, _window, cx| {
                toggle.update(cx, |state, cx| state.toggle_run_phase(index, cx));
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(Disclosure::new(("run-action-disclosure", index), open))
                    .child(div().flex_1().min_w_0().child(format!(
                        "{} · {}",
                        format::hours(action.start),
                        action.title
                    )))
                    .child(Caption::new(format!(
                        "{} EXP/hr",
                        format::rate(action.rate)
                    ))),
            );
        if open {
            row = row.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .pl_5()
                    .text_color(colors.fg_muted)
                    .children(action.lines.iter().map(Caption::new)),
            );
        }
        list = list.child(row);
    }
    list.into_any_element()
}

/// What an event reads as in a title or a phase label.
fn event_label(event: Event) -> &'static str {
    match event {
        Event::Start => "Initial optimization",
        Event::Level => "Research level up",
        Event::Insight => "Insight level up",
        Event::LevelAndInsight => "Research and insight level up",
        Event::Tournament => "Tournament registration",
        Event::End => "Target reached",
    }
}

/// The colour of a phase: the chart's own hues, one per event kind.
fn event_color(event: Event, colors: &Palette) -> Hsla {
    match event {
        Event::Start | Event::End => colors.accent,
        Event::Level => colors.chart[2],
        Event::Insight => colors.chart[4],
        Event::LevelAndInsight => colors.chart[7],
        Event::Tournament => colors.chart[6],
    }
}

fn metric(id: &'static str, label: &'static str, value: String) -> impl IntoElement {
    div()
        .id(id)
        .role(Role::Label)
        .aria_label(value.clone())
        .flex()
        .flex_col()
        .gap_0p5()
        .child(Caption::new(label))
        .child(div().text_size(px(16.0)).child(value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;

    fn projection(samples: Vec<Sample>) -> Projection {
        Projection {
            hours: samples.last().map_or(0.0, |sample| sample.hours),
            final_level: samples.last().map_or(0, |sample| sample.level),
            total_exp: 1200.0,
            insight_gained: 0,
            samples,
        }
    }

    fn sample(hours: f64, level: u32, rate: f64, event: Event) -> Sample {
        Sample {
            hours,
            level,
            rate,
            event,
            ..Sample::default()
        }
    }

    fn result(target: Target, best: Projection, worst: Projection) -> OptimizeResult {
        OptimizeResult {
            before: Simulation::default(),
            after: Simulation::default(),
            before_rate: 100.0,
            after_rate: 150.0,
            best: Strategy {
                label: "Optimized layout",
                projection: best,
            },
            worst: Strategy {
                label: "Saved layout",
                projection: worst,
            },
            target,
            steps: vec!["Spent 6 grid points.".into()],
            elapsed_ms: 4,
        }
    }

    #[test]
    fn the_first_action_carries_the_optimizers_own_steps() {
        let run = result(
            Target::Level(6),
            projection(vec![
                sample(0.0, 5, 100.0, Event::Start),
                sample(4.0, 6, 150.0, Event::Level),
            ]),
            projection(vec![
                sample(0.0, 5, 90.0, Event::Start),
                sample(6.0, 6, 90.0, Event::Level),
            ]),
        );
        let actions = actions(&run, &timeline::phases(&run.best.projection));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].title, "Initial optimization");
        assert!(
            actions[0]
                .lines
                .iter()
                .any(|line| line == "Spent 6 grid points."),
            "the steps are missing: {:?}",
            actions[0].lines
        );
        assert!(
            actions[0]
                .lines
                .iter()
                .any(|line| line.contains("Research level 5 → 6")),
            "the closing level-up is missing: {:?}",
            actions[0].lines
        );
    }

    #[test]
    fn a_run_without_events_still_shows_its_work() {
        let run = result(
            Target::Hours(24.0),
            projection(vec![sample(0.0, 5, 100.0, Event::Start)]),
            projection(vec![sample(0.0, 5, 80.0, Event::Start)]),
        );
        let actions = actions(&run, &timeline::phases(&run.best.projection));
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].lines, ["Spent 6 grid points."]);
    }

    #[test]
    fn the_outcome_names_the_level_a_stall_or_the_exp_earned() {
        let reached = projection(vec![sample(4.0, 63, 0.0, Event::Level)]);
        let stalled = projection(vec![sample(3.0, 61, 0.0, Event::End)]);
        assert_eq!(
            strategy_outcome(Target::Level(63), &reached),
            "reaches the level in 4.0h"
        );
        assert_eq!(
            strategy_outcome(Target::Level(63), &stalled),
            "stalls at level 61 after 3.0h"
        );
        assert_eq!(
            strategy_outcome(Target::Hours(24.0), &reached),
            "1.2K EXP in 4.0h"
        );
    }

    #[test]
    fn the_lead_is_reported_only_when_both_layouts_reach_the_level() {
        let best = projection(vec![sample(4.0, 63, 0.0, Event::Level)]);
        let worst = projection(vec![sample(8.0, 63, 0.0, Event::Level)]);
        let lead = result(Target::Level(63), best.clone(), worst)
            .lead()
            .expect("both layouts reached the level");
        assert_eq!(lead.hours, 4.0);
        assert_eq!(lead.pct, 50.0);

        let stalled = projection(vec![sample(3.0, 61, 0.0, Event::End)]);
        assert!(
            result(Target::Level(63), best.clone(), stalled)
                .lead()
                .is_none()
        );
        assert!(
            result(Target::Hours(24.0), best.clone(), best)
                .lead()
                .is_none(),
            "an hour target has no time to save"
        );
    }
}
