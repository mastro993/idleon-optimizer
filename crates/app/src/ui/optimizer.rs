//! Optimize pane: the target, the effort, and what the last run found.

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, SegmentedControl, ToggleGroup, ToggleItem};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::game;
use idleon_core::model::{Effort, Target};

pub fn optimizer_pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    div()
        .id("optimizer")
        .flex()
        .size_full()
        .gap_3()
        .min_h_0()
        .child(
            div()
                .id("optimizer-controls")
                .flex()
                .flex_col()
                .flex_none()
                .w(px(380.0))
                .gap_3()
                .min_h_0()
                .child(controls(state, cx)),
        )
        .child(
            div()
                .id("optimizer-result")
                .flex_1()
                .min_w_0()
                .min_h_0()
                .child(result(state, cx)),
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
    let run = Button::new("run-optimize", if running { "Optimizing…" } else { "Optimize" })
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
                .child(
                    Caption::new(state.status.clone()).text_color(colors.fg_muted),
                ),
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
    let tone = if gain > 0.0 { Tone::Success } else { Tone::Neutral };
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
                .description(format!("{} steps · {} ms", result.steps.len(), result.elapsed_ms))
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
                        .gap_4()
                        .child(metric("result-before", "Before", format::rate(result.before_rate)))
                        .child(metric("result-after", "After", format::rate(result.after_rate)))
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
                        .child(
                            Button::new("dismiss-result", "Dismiss").on_click(
                                move |_event, _window, cx| {
                                    dismiss.update(cx, |state, cx| state.dismiss_result(cx));
                                },
                            ),
                        ),
                )
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
