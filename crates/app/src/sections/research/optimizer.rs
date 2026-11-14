//! Optimize pane: the target, the effort and the controls that start a run.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use ely_gpui_component::buttons::{Button, SegmentedControl, ToggleGroup, ToggleItem};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Checkbox, Stepper};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::primitives::Severity;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_research::game;
use idleon_research::model::{Effort, SimOptions, Target};

pub(crate) fn optimizer_pane(
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
            .child(super::result::card(state, cx)),
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

    let current_level = state.simulation.research_level;
    let problem = state.target_problem();
    let (value, min, max, step) = match state.target {
        Target::Level(level) => (
            level as f64,
            f64::from(current_level.saturating_add(1)),
            (f64::from(current_level) + 500.0).max(500.0),
            1.0,
        ),
        Target::Hours(hours) => (hours, 1.0, 10_000.0, 1.0),
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

    let options = state.options;
    let option_entity = entity.clone();
    let option =
        |id: &'static str, label: &'static str, checked: bool, set: fn(&mut SimOptions, bool)| {
            let option_entity = option_entity.clone();
            Checkbox::new(id, checked)
                .label(label)
                .on_change(move |on, _window, cx| {
                    let option_entity = option_entity.clone();
                    option_entity.update(cx, |state, cx| {
                        let mut options = state.options;
                        set(&mut options, on);
                        state.set_options(options, cx);
                    });
                })
        };
    let assume = option(
        "option-assume-observations",
        "Assume observations unlocked",
        options.assume_observations,
        |options, on| options.assume_observations = on,
    );
    let extended = option(
        "option-extended-insight",
        "Extended insight lookahead",
        options.extended_insight,
        |options, on| options.extended_insight = on,
    );
    let tournament = option(
        "option-tournament",
        "Tournament +12h",
        options.include_tournament,
        |options, on| options.include_tournament = on,
    );
    let registrant = state.context.event_shop(46) > 0.0;

    let mut presets = ToggleGroup::new("preset").size(ControlSize::Sm);
    for preset in &game().presets {
        presets = presets.item(ToggleItem::new(preset.id.clone()).label(preset.name.clone()));
    }
    let preset_entity = entity.clone();
    let presets = presets
        .selected([state.simulation.preset.clone()])
        .on_change(move |selected, _window, cx| {
            let Some(id) = selected.first() else { return };
            preset_entity.update(cx, |state, cx| state.set_preset(id, cx));
        });

    let run_entity = entity.clone();
    let running = state.optimize_running(cx);
    let blocked = running || problem.is_some();
    let run = Button::new(
        "run-optimize",
        if running { "Optimizing…" } else { "Optimize" },
    )
    .primary()
    .icon(IconName::Play)
    .full_width()
    .disabled(blocked)
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
                        .gap_2()
                        .child(Caption::new("Options"))
                        .child(assume)
                        .child(Caption::new("Unlocks each observation at its research requirement as the run levels."))
                        .child(extended)
                        .child(Caption::new("Values insight by the rate it permanently buys."))
                        .child(tournament)
                        .child(Caption::new(if registrant {
                            "Research Registrant owned; adds 12h at every daily reset."
                        } else {
                            "Research Registrant not detected in the save."
                        })),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(Caption::new("Preset priority"))
                        .child(presets),
                )
                .child(run)
                .when_some(problem.clone(), |card, problem| {
                    card.child(Banner::new("target-problem", Severity::Danger, problem))
                })
                .child(Caption::new(state.status.clone()).text_color(colors.fg_muted)),
        )
}
