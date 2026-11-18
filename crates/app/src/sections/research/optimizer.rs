//! Optimize pane: the target, the effort and the controls that start a run.

use crate::state::AppState;
use crate::ui::layout::{self, Breakpoint};
use gpui::prelude::*;
use gpui::{Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::NumberInput;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _};
use idleon_research::game;
use idleon_research::model::{Effort, SimOptions, Target};
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone, show_number};

use super::controls::segmented;

pub(crate) fn optimizer_pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    layout::split_pane_reversed(
        "optimizer",
        Breakpoint::of(window, cx),
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
            .child(controls(state, window, cx)),
        px(380.0),
    )
}

fn controls(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> impl IntoElement {
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();

    let target_entity = entity.clone();
    let kind_control = segmented(
        "target-kind",
        &[("level", "Research level"), ("hours", "Hours")],
        if matches!(state.target, Target::Hours(_)) {
            "hours"
        } else {
            "level"
        },
        move |kind, _window, cx| {
            target_entity.update(cx, |state, cx| state.set_target_kind(kind, cx));
        },
    );

    let problem = state.target_problem();
    let value = match state.target {
        Target::Level(level) => f64::from(level),
        Target::Hours(hours) => hours,
    };
    show_number(&state.research.target_value, value, 0, window, cx);
    // `NumberInput` has no `id`: the wrapper keeps the Ely control's.
    let value_control = div().id("target-value").w(px(160.0)).child(
        NumberInput::new(&state.research.target_value)
            .accessibility_label("Target")
            .small(),
    );

    let effort_entity = entity.clone();
    let effort_control = segmented(
        "effort",
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

    let options = state.options;
    let option_entity = entity.clone();
    let option =
        |id: &'static str, label: &'static str, checked: bool, set: fn(&mut SimOptions, bool)| {
            let option_entity = option_entity.clone();
            Checkbox::new(id)
                .label(label)
                .checked(checked)
                .on_click(move |on, _window, cx| {
                    let on = *on;
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

    let active = &state.simulation.preset;
    let preset_entity = entity.clone();
    let presets = ToggleGroup::new("preset")
        .single()
        .small()
        .children(game().presets.iter().map(|preset| {
            Toggle::new(SharedString::from(preset.id.clone()))
                .label(preset.name.clone())
                .checked(&preset.id == active)
        }))
        .on_click(move |checked, _window, cx| {
            let Some(index) = checked.iter().position(|checked| *checked) else {
                return;
            };
            let id = &game().presets[index].id;
            preset_entity.update(cx, |state, cx| state.set_preset(id, cx));
        });

    let run_entity = entity.clone();
    let running = state.optimize_running(cx);
    let blocked = running || problem.is_some();
    let run = Button::new("run-optimize")
        .label(if running { "Optimizing…" } else { "Optimize" })
        .primary()
        .icon(IconName::Play)
        .w_full()
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
                    card.child(
                        Alert::new("target-problem", problem)
                            .with_variant(Tone::Danger.into())
                            .banner(),
                    )
                })
                .child(Caption::new(state.status.clone()).text_color(muted)),
        )
}
