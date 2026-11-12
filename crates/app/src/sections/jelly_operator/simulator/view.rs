//! The simulator tab's controls, sandbox board and board-section toggles.

use super::result;
use super::state::{fever_modes, revive_label};
use crate::sections::jelly_operator::grid;
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant, ToggleGroup, ToggleItem};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_jelly::board;
use idleon_jelly::operation::RevivePolicy;
use idleon_jelly::{Jelly, boss_hp, boss_time, data, fever_mode_bonus, fever_mode_name};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// The simulator tab: the controls, the sandbox board and the last run.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_sim_task(cx);
    let running = task.running;
    let simulator = &state.jelly_state.simulator;
    let sandbox = simulator.sandbox(jelly);
    let stage = simulator.obstruction;
    let selected = simulator.selected;
    let plots: Vec<usize> = simulator.plots.iter().copied().collect();
    let controls = controls(state, cx, jelly);
    let actions = actions(state, cx);
    let palette = palette(state, cx, jelly, selected);
    let board = grid::board(
        state,
        grid::BoardView {
            prefix: "jelly-sim",
            layout: &simulator.layout,
            unlocked: &board::unlocked_slots(&sandbox),
            obstruction: stage,
            selected,
            action: grid::BoardAction::Simulator,
        },
        cx,
    );

    let mut body = div()
        .id("jelly-simulator")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Card::new()
                .header(
                    CardHeader::new("Interactive operation simulator").description(
                        "A local sandbox: it starts from the save and never writes back to it.",
                    ),
                )
                .child(div().p_3().child(controls)),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Sandbox board").description(format!(
                    "{} cells · {} slot(s) unlocked · stage {}",
                    simulator.layout.len(),
                    board::unlocked_count(&sandbox),
                    stage + 1
                )))
                .child(
                    div()
                        .p_3()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(actions)
                        .child(palette)
                        .child(board)
                        .child(message(state, cx)),
                ),
        )
        .child(sections(cx, jelly, &plots));

    let Some(result) = task.result.clone() else {
        if !running && !task.cancelled {
            body = body.child(Banner::new(
                "jelly-sim-empty",
                Severity::Info,
                "Simulate one operation to see its outcome, cell contribution and critical target timeline.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-sim-stale"));
    }
    body.child(result::card(&result, task.stale, cx))
        .into_any_element()
}

/// Stage, Fever, Stronkroid, revive policy, seed and the run button.
fn controls(state: &AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let task = state.jelly_sim_task(cx);
    let running = task.running;
    let simulator = &state.jelly_state.simulator;
    let modes = fever_modes(jelly);
    let entity = cx.entity();

    let stage_choices: Vec<Choice> = (0..=super::MAX_STAGE)
        .map(|index| {
            let name = data()
                .reward(index)
                .map(|reward| reward.name.clone())
                .unwrap_or_else(|| format!("Obstruction {}", index + 1));
            Choice::new(index.to_string(), format!("{}. {name}", index + 1)).note(format!(
                "{} HP · {:.0}s normal phase",
                format::value(boss_hp(index)),
                boss_time(index)
            ))
        })
        .collect();
    let stage_entity = entity.clone();
    let stage_select = Select::new("jelly-sim-stage", stage_choices)
        .label("Stage")
        .selected(simulator.obstruction.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(stage) = value.parse::<usize>() {
                let entity = stage_entity.clone();
                entity.update(cx, |state, cx| state.set_jelly_sim_stage(stage, cx));
            }
        });

    let fever_choices: Vec<Choice> = if modes > 0 {
        (0..modes)
            .map(|mode| {
                Choice::new(
                    mode.to_string(),
                    format!(
                        "{} — {}",
                        fever_mode_name(mode),
                        fever_mode_bonus(mode).unwrap_or("")
                    ),
                )
            })
            .collect()
    } else {
        vec![Choice::new("0", "Feverizer locked").disabled()]
    };
    let fever_entity = entity.clone();
    let fever_select = Select::new("jelly-sim-fever", fever_choices)
        .label("Fever")
        .selected(simulator.fever.to_string())
        .size(ControlSize::Sm)
        .disabled(modes == 0)
        .on_change(move |value, _window, cx| {
            if let Ok(fever) = value.parse::<u32>() {
                let entity = fever_entity.clone();
                entity.update(cx, |state, cx| state.set_jelly_sim_fever(fever, cx));
            }
        });

    let roid_entity = entity.clone();
    let roid_input = NumberInput::new("jelly-sim-roid", simulator.roid_timing)
        .label("Stronkroid activation second")
        .range(-1.0, boss_time(simulator.obstruction))
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let entity = roid_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_sim_roid(value, cx));
        });

    let revive_entity = entity.clone();
    let revive_select = Select::new(
        "jelly-sim-revive",
        [
            RevivePolicy::None,
            RevivePolicy::ImmunoidFirst,
            RevivePolicy::AnchorFirst,
            RevivePolicy::DpsFirst,
        ]
        .map(|policy| Choice::new(revive_id(policy), revive_label(policy))),
    )
    .label("Automatic revival strategy")
    .selected(revive_id(simulator.revive))
    .size(ControlSize::Sm)
    .on_change(move |value, _window, cx| {
        let policy = revive_from_id(value.as_ref());
        let entity = revive_entity.clone();
        entity.update(cx, |state, cx| state.set_jelly_sim_revive(policy, cx));
    });

    let seed_entity = entity.clone();
    let seed_input = NumberInput::new("jelly-sim-seed", f64::from(simulator.seed))
        .label("Seed")
        .range(0.0, f64::from(u32::MAX))
        .step(1.0)
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let entity = seed_entity.clone();
            entity.update(cx, |state, cx| state.set_jelly_sim_seed(value, cx));
        });

    let run_entity = entity.clone();
    let run = Button::new("jelly-sim-run", "Simulate one operation")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            let entity = run_entity.clone();
            entity.update(cx, |state, cx| state.run_jelly_simulation(cx));
        });

    let mut row = div()
        .id("jelly-sim-controls")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            div()
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .child(stage_select)
                .child(fever_select)
                .children(owns_roid(jelly).then_some(roid_input))
                .children(owns_revives(jelly).then_some(revive_select))
                .child(seed_input)
                .child(run),
        )
        .child(
            Caption::new(
                "One seeded 60 FPS operation. Stage, Fever and the board sections come from this sandbox.",
            )
            .text_color(cx.theme().colors.fg_muted),
        );
    if running && let Some(progress) = task.progress.clone() {
        let entity = cx.entity();
        row = row.child(run_chrome::progress_card(
            "jelly-sim",
            "Simulating the operation",
            "One seeded 60 FPS attempt of the sandbox board.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_jelly_simulation(cx));
            },
        ));
    }
    if task.cancelled {
        row = row.child(run_chrome::cancelled_banner(
            "jelly-sim-cancelled",
            task.stale,
        ));
    }
    row.into_any_element()
}

fn owns_roid(jelly: &Jelly) -> bool {
    jelly.upgrade_level(29) > 0.0
}

fn owns_revives(jelly: &Jelly) -> bool {
    jelly.upgrade_level(35) > 0.0
}

fn revive_id(revive: RevivePolicy) -> &'static str {
    match revive {
        RevivePolicy::None => "none",
        RevivePolicy::ImmunoidFirst => "immunoid-first",
        RevivePolicy::AnchorFirst => "anchor-first",
        RevivePolicy::DpsFirst => "dps-first",
        RevivePolicy::SoakFirst => "soak-first",
    }
}

fn revive_from_id(id: &str) -> RevivePolicy {
    match id {
        "none" => RevivePolicy::None,
        "anchor-first" => RevivePolicy::AnchorFirst,
        "dps-first" => RevivePolicy::DpsFirst,
        "soak-first" => RevivePolicy::SoakFirst,
        _ => RevivePolicy::ImmunoidFirst,
    }
}

/// Reset from save, clear, remove mode and the three board-section actions.
fn actions(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let remove_mode = state.jelly_state.simulator.selected.is_none();
    let mut row = div().id("jelly-sim-actions").flex().flex_wrap().gap_2();
    for (id, label, variant, action) in [
        (
            "jelly-sim-reset",
            "Reset from save",
            ButtonVariant::Subtle,
            SimulatorAction::Reset,
        ),
        (
            "jelly-sim-clear",
            "Clear cells",
            ButtonVariant::Subtle,
            SimulatorAction::Clear,
        ),
        (
            "jelly-sim-remove",
            "Remove mode",
            if remove_mode {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Subtle
            },
            SimulatorAction::Remove,
        ),
        (
            "jelly-sim-save-sections",
            "Use save sections",
            ButtonVariant::Subtle,
            SimulatorAction::SaveSections,
        ),
        (
            "jelly-sim-lock-sections",
            "Lock all purchased sections",
            ButtonVariant::Subtle,
            SimulatorAction::LockSections,
        ),
        (
            "jelly-sim-unlock-sections",
            "Unlock all sections",
            ButtonVariant::Subtle,
            SimulatorAction::UnlockSections,
        ),
    ] {
        let entity = entity.clone();
        row = row.child(
            Button::new(id, label)
                .variant(variant)
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    let entity = entity.clone();
                    entity.update(cx, |state, cx| match action {
                        SimulatorAction::Reset => state.reset_jelly_simulator(cx),
                        SimulatorAction::Clear => state.clear_jelly_simulator(cx),
                        SimulatorAction::Remove => state.jelly_sim_remove_mode(cx),
                        SimulatorAction::SaveSections => state.jelly_sim_use_save_sections(cx),
                        SimulatorAction::LockSections => state.jelly_sim_lock_sections(cx),
                        SimulatorAction::UnlockSections => state.jelly_sim_unlock_all_sections(cx),
                    });
                }),
        );
    }
    row.into_any_element()
}

#[derive(Clone, Copy)]
enum SimulatorAction {
    Reset,
    Clear,
    Remove,
    SaveSections,
    LockSections,
    UnlockSections,
}

/// The cell palette, with the same buttons as the layout tab.
fn palette(
    state: &AppState,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    selected: Option<usize>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let buttons = grid::palette(
        state,
        jelly.units_owned(),
        selected,
        "jelly-sim-palette",
        cx,
        |cell_type, state, cx| state.jelly_sim_select(cell_type, cx),
    );
    div()
        .id("jelly-sim-palette-row")
        .flex()
        .flex_col()
        .gap_1()
        .child(
            Caption::new("Cell palette · pick a cell, then click a slot. Remove mode removes it.")
                .text_color(muted),
        )
        .child(buttons)
        .into_any_element()
}

/// The hypothetical board sections (`Research[18]`).
fn sections(cx: &mut Context<AppState>, jelly: &Jelly, plots: &[usize]) -> AnyElement {
    let mut group = ToggleGroup::new("jelly-sim-plots")
        .multiple()
        .size(ControlSize::Sm)
        .selected(plots.iter().map(usize::to_string));
    for plot in &data().plots {
        group = group.item(ToggleItem::new(plot.plot_id.to_string()).label(format!(
            "Section {} · {} slots",
            plot.plot_id + 1,
            plot.width * plot.height
        )));
    }
    let entity = cx.entity();
    let group = group.on_change(move |selected, _window, cx| {
        let ids: Vec<usize> = selected
            .iter()
            .filter_map(|value| value.parse::<usize>().ok())
            .collect();
        let entity = entity.clone();
        entity.update(cx, |state, cx| state.jelly_sim_set_plots(&ids, cx));
    });
    Card::new()
        .header(CardHeader::new("Board sections").description(
            "Section toggles are hypothetical; they change the sandbox's unlocked slots.",
        ))
        .child(
            div().p_3().flex().flex_col().gap_3().child(group).child(
                Caption::new(format!(
                    "{} of {} section(s) unlocked · the save purchased {}",
                    plots.len(),
                    data().plots.len(),
                    jelly.plots.len()
                ))
                .text_color(cx.theme().colors.fg_muted),
            ),
        )
        .into_any_element()
}

/// The last action's answer, red when the action was refused.
fn message(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let simulator = &state.jelly_state.simulator;
    if simulator.message.is_empty() {
        return div().into_any_element();
    }
    let color = if simulator.message_bad {
        cx.theme().colors.danger
    } else {
        cx.theme().colors.fg_muted
    };
    div()
        .id("jelly-sim-message")
        .role(Role::Label)
        .aria_label(simulator.message.clone())
        .child(Caption::new(simulator.message.clone()).text_color(color))
        .into_any_element()
}
