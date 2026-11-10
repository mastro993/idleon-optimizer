//! Optimize pane: the target, the effort, and what the last run found.

use crate::state::AppState;
use crate::ui::format;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::{Button, SegmentedControl, ToggleGroup, ToggleItem};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, Hsla, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::game;
use idleon_core::model::{Effort, Target};
use idleon_core::sim::Sample;

pub fn optimizer_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    layout::split_pane_reversed(
        "optimizer",
        Breakpoint::of(window),
        div()
            .id("optimizer-result")
            .flex_1()
            .min_w_0()
            .min_h_0()
            .child(result(state, cx)),
        div()
            .id("optimizer-controls")
            .flex()
            .flex_col()
            .gap_3()
            .min_h_0()
            .child(controls(state, cx)),
        px(380.0),
    )
}

fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let entity = cx.entity();

    let kind = if matches!(state.target, Target::Hours(_)) {
        "hours"
    } else {
        "level"
    };
    let target_entity = entity.clone();
    let kind_control = SegmentedControl::new("target-kind", kind)
        .size(ControlSize::Sm)
        .segment("level", "Research level", None)
        .segment("hours", "Hours", None)
        .on_change(move |value, _window, cx| {
            let value = value.to_string();
            target_entity.update(cx, |state, cx| state.set_target_kind(&value, cx));
        });

    let (value, min, max, step) = match state.target {
        Target::Level(level) => (level as f64, 1.0, 500.0, 1.0),
        Target::Hours(hours) => (hours, 0.5, 10_000.0, 1.0),
    };
    let entity_for_target = entity.clone();
    let value_control = Stepper::new("target-value", value)
        .range(min, max)
        .step(step)
        .on_change(move |value, _window, cx| {
            entity_for_target.update(cx, |state, cx| match state.target {
                Target::Level(_) => state.set_target_level(value, cx),
                Target::Hours(_) => state.set_target_hours(value, cx),
            });
        });

    let effort = match state.effort {
        Effort::Quick => "quick",
        Effort::Balanced => "balanced",
        Effort::Thorough => "thorough",
    };
    let effort_entity = entity.clone();
    let effort_control = SegmentedControl::new("effort", effort)
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

    let mut presets = ToggleGroup::new("preset").size(ControlSize::Sm);
    for preset in &game().presets {
        presets = presets.item(ToggleItem::new(preset.id.clone()).label(preset.name.clone()));
    }
    let preset_entity = entity.clone();
    let presets = presets
        .selected([state.simulation.preset.clone()])
        .on_change(move |selected, _window, cx| {
            let Some(id) = selected.first() else { return };
            let id = id.to_string();
            preset_entity.update(cx, |state, cx| state.set_preset(id, cx));
        });

    let run_entity = entity.clone();
    let running = state.running;
    let run = Button::new(
        "run-optimize",
        if running { "Optimizing…" } else { "Optimize" },
    )
    .primary()
    .icon(IconName::Play)
    .full_width()
    .disabled(running)
    .on_click(move |_event, _window, cx| {
        run_entity.update(cx, |state, cx| state.optimize(cx));
    });

    Card::new()
        .header(CardHeader::new("Target").description(
            "The optimizer stops when this level is reached, or after this many hours.",
        ))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_3()
                .p_3()
                .child(kind_control)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(Caption::new(match state.target {
                            Target::Level(_) => "Research level",
                            Target::Hours(_) => "Hours to simulate",
                        }))
                        .child(value_control),
                )
                .child(effort_control)
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Preset priority"))
                        .child(presets),
                )
                .child(run)
                .child(Caption::new(state.status.clone()).text_color(colors.fg_muted)),
        )
}

fn result(state: &mut AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let colors = &cx.theme().colors;
    let Some(result) = state.result.clone() else {
        return Card::new()
            .header(CardHeader::new("Result"))
            .child(
                div().p_3().flex().flex_col().gap_2().child(
                    Banner::new(
                        "result-empty",
                        Severity::Info,
                        "Nothing yet. Pick a target and press Optimize; the run happens in this process, so it can be thorough.",
                    ),
                ),
            )
            .into_any_element();
    };

    let gain = result.gain_pct();
    let tone = if gain > 0.0 {
        Tone::Success
    } else {
        Tone::Neutral
    };
    let entity = cx.entity();
    let apply = entity.clone();
    let dismiss = entity.clone();

    let mut steps = div().flex().flex_col().gap_1();
    for (index, step) in result.steps.iter().enumerate() {
        steps = steps.child(
            div()
                .id(("step", index))
                .flex()
                .gap_2()
                .child(Caption::new(format!("{}.", index + 1)))
                .child(Caption::new(step.clone()).text_color(colors.fg_muted)),
        );
    }

    Card::new()
        .header(
            CardHeader::new("Result")
                .description(format!(
                    "{} steps · {} ms",
                    result.steps.len(),
                    result.elapsed_ms
                ))
                .action(
                    div()
                        .id("result-gain")
                        .role(gpui::Role::Label)
                        .aria_label(format!("{gain:+.1}% EXP/hr"))
                        .child(Badge::new(format!("{gain:+.1}% EXP/hr")).tone(tone)),
                ),
        )
        .child(
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
                                format::hours(result.projection.hours),
                                result.projection.final_level
                            ),
                        ))
                        .child(metric(
                            "result-insight",
                            "Insight levels",
                            format!("{}", result.projection.insight_gained),
                        )),
                )
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(
                            Button::new("apply-result", "Apply layout")
                                .primary()
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
                .child(projection_chart(
                    &result.projection.samples,
                    (colors.accent, colors.fg_muted, colors.border, colors.sunken),
                ))
                .child(div().h(px(1.0)).bg(colors.border))
                .child(
                    ScrollArea::new("steps-scroll").child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(Caption::new("Steps"))
                            .child(steps),
                    ),
                ),
        )
        .into_any_element()
}

/// Rate at `hours`, interpolated between the projection's samples.
fn rate_at(samples: &[Sample], hours: f64) -> f64 {
    match samples {
        [] => 0.0,
        [only] => only.rate,
        _ => {
            let mut previous = &samples[0];
            for sample in &samples[1..] {
                if sample.hours >= hours {
                    let span = sample.hours - previous.hours;
                    if span <= 0.0 {
                        return sample.rate;
                    }
                    let share = (hours - previous.hours) / span;
                    return previous.rate + (sample.rate - previous.rate) * share;
                }
                previous = sample;
            }
            previous.rate
        }
    }
}

/// Bar heights for the projection: EXP/hour across the run, slowest at the bottom.
///
/// The run is resampled into `bins` even slices, so the chart reads as a timeline
/// however few level-ups the projection recorded. A flat projection sits level in
/// the middle instead of collapsing onto an edge.
fn bar_heights(samples: &[Sample], height: f32, bins: usize) -> Vec<f32> {
    const FLOOR: f32 = 2.0;
    if samples.is_empty() {
        return Vec::new();
    }
    let height = height.max(FLOOR + 1.0);
    let bins = bins.max(2);
    let last = samples.last().map(|sample| sample.hours).unwrap_or(0.0);
    let span = if last > 0.0 { last } else { 1.0 };
    let rates: Vec<f64> = (0..bins)
        .map(|index| rate_at(samples, span * index as f64 / (bins - 1) as f64))
        .collect();
    let slowest = rates.iter().copied().fold(f64::INFINITY, f64::min);
    let fastest = rates.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let range = fastest - slowest;
    rates
        .iter()
        .map(|rate| {
            if !range.is_finite() || range <= 0.0 {
                return height / 2.0;
            }
            let share = ((rate - slowest) / range) as f32;
            FLOOR + share * (height - FLOOR)
        })
        .collect()
}

fn projection_chart(samples: &[Sample], palette: (Hsla, Hsla, Hsla, Hsla)) -> impl IntoElement {
    const PLOT_HEIGHT: f32 = 96.0;
    let (accent, muted, border, sunken) = palette;
    let start = samples.first().copied().unwrap_or_default();
    let end = samples.last().copied().unwrap_or_default();
    let described = format!(
        "Rate over hours: starts at {} EXP/hr, ends at {} EXP/hr after {}",
        format::rate(start.rate),
        format::rate(end.rate),
        format::hours(end.hours)
    );
    let heights = bar_heights(samples, PLOT_HEIGHT, 48);
    let mut bars = div()
        .id("projection-bars")
        .flex()
        .items_end()
        .w_full()
        .h(px(PLOT_HEIGHT));
    for weight in &heights {
        bars = bars.child(
            div()
                .flex_1()
                .min_w(px(1.0))
                .h(px(*weight))
                .bg(accent.opacity(0.75)),
        );
    }
    div()
        .id("projection-chart")
        .role(gpui::Role::Label)
        .aria_label(described)
        .flex()
        .flex_col()
        .gap_1()
        .w_full()
        .child(
            div()
                .w_full()
                .rounded(px(6.0))
                .border_1()
                .border_color(border)
                .bg(sunken)
                .px(px(3.0))
                .py(px(3.0))
                .child(bars),
        )
        .child(
            div()
                .flex()
                .justify_between()
                .text_size(px(11.0))
                .text_color(muted)
                .child(format!(
                    "start {} · {}",
                    format::hours(start.hours),
                    format::rate(start.rate)
                ))
                .child(format!(
                    "end {} · {}",
                    format::hours(end.hours),
                    format::rate(end.rate)
                )),
        )
}

fn metric(id: &'static str, label: &'static str, value: String) -> impl IntoElement {
    div()
        .id(id)
        .role(gpui::Role::Label)
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

    #[test]
    fn bars_grow_with_the_rate_and_keep_a_floor() {
        let samples = [
            Sample {
                hours: 0.0,
                level: 100,
                rate: 100.0,
            },
            Sample {
                hours: 5.0,
                level: 101,
                rate: 200.0,
            },
        ];
        let heights = bar_heights(&samples, 100.0, 4);
        assert_eq!(heights.len(), 4);
        assert_eq!(heights[0], 2.0, "the slowest end sits on the floor");
        assert_eq!(heights[3], 100.0, "the fastest end fills the plot");
        assert!(
            heights.windows(2).all(|pair| pair[0] <= pair[1]),
            "rising: {heights:?}"
        );
    }

    #[test]
    fn bars_cope_with_an_empty_or_flat_projection() {
        assert!(bar_heights(&[], 100.0, 8).is_empty());
        let flat = [
            Sample {
                hours: 0.0,
                level: 1,
                rate: 42.0,
            },
            Sample {
                hours: 24.0,
                level: 1,
                rate: 42.0,
            },
        ];
        assert_eq!(bar_heights(&flat, 100.0, 4), vec![50.0; 4]);
        let single = [Sample {
            hours: 0.0,
            level: 1,
            rate: 7.0,
        }];
        assert_eq!(bar_heights(&single, 100.0, 3), vec![50.0; 3]);
    }
}
