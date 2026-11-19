//! The simulator tab's controls, sandbox board and board-section toggles.

use super::result;
use super::state::{fever_modes, revive_label};
use crate::sections::jelly_operator::grid;
use crate::sections::jelly_operator::parts::{
    field_row, info, number_field, run_button, select_field,
};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::component::button::{Button, ButtonVariants as _, Toggle, ToggleGroup};
use gpui_kit::component::{ActiveTheme as _, Sizable as _};
use idleon_jelly::operation::RevivePolicy;
use idleon_jelly::{Jelly, Trial, data, fever_mode_bonus, fever_mode_name};
use idleon_ui::run_chrome;
use idleon_ui::runner::TaskState;
use idleon_ui::widgets::{Caption, Card, CardHeader, Choice, PANEL_GAP};
use std::rc::Rc;

/// The simulator tab: the controls, the sandbox board and the last run.
pub(crate) fn pane(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_sim_task(cx);
    let running = task.running;
    let controls = controls(state, window, cx, jelly, &task);
    let simulator = &state.jelly_state.simulator;
    let figures = &state.jelly_state.sandbox;
    let stage = simulator.obstruction;
    let selected = simulator.selected;
    let actions = actions(state, cx);
    let palette = palette(state, cx, jelly, selected);
    let board = grid::board(
        state,
        grid::BoardView {
            prefix: "jelly-sim",
            layout: &simulator.layout,
            unlocked: &figures.unlocked,
            highlight: &[],
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
        .gap(PANEL_GAP)
        .child(
            Card::new()
                .header(
                    CardHeader::new("Interactive operation simulator").description(
                        "A local sandbox: it starts from the save and never writes back to it.",
                    ),
                )
                .child(controls),
        )
        .child(
            Card::new()
                .header(CardHeader::new("Sandbox board").description(format!(
                    "{} cells · {} slot(s) unlocked · stage {}",
                    simulator.layout.len(),
                    figures.unlocked_count,
                    stage + 1
                )))
                .child(actions)
                .child(palette)
                .child(board)
                .child(message(state, cx)),
        )
        .child(sections(state, cx, jelly));

    let Some(result) = task.result else {
        if !running && !task.cancelled {
            body = body.child(info(
                "jelly-sim-empty",
                "Simulate one operation to see its outcome, cell contribution and critical target timeline.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-sim-stale"));
    }
    body.child(result::card(&result, task.stale))
        .into_any_element()
}

/// Stage, Fever, Stronkroid, revive policy, seed and the run button.
fn controls(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    task: &TaskState<Rc<Trial>>,
) -> AnyElement {
    let running = task.running;
    let simulator = &state.jelly_state.simulator;
    let controls = &state.jelly_controls;
    let modes = fever_modes(jelly);
    controls.sim_stage.show(
        controls.stage_choices.clone(),
        Some(&simulator.obstruction),
        window,
        cx,
    );
    let fever_choices: Vec<Choice<u32>> = if modes > 0 {
        (0..modes)
            .map(|mode| {
                Choice::new(
                    mode,
                    format!(
                        "{} — {}",
                        fever_mode_name(mode),
                        fever_mode_bonus(mode).unwrap_or("")
                    ),
                )
            })
            .collect()
    } else {
        vec![Choice::new(0, "Feverizer locked").disabled(true)]
    };
    controls
        .sim_fever
        .show(fever_choices, Some(&simulator.fever), window, cx);
    controls
        .sim_revive
        .show(revive_choices(), Some(&simulator.revive), window, cx);

    let mut row = div()
        .id("jelly-sim-controls")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            field_row()
                .child(select_field(
                    &controls.sim_stage,
                    "jelly-sim-stage",
                    "Stage",
                    false,
                    window,
                    cx,
                ))
                .child(select_field(
                    &controls.sim_fever,
                    "jelly-sim-fever",
                    "Fever",
                    modes == 0,
                    window,
                    cx,
                ))
                .when(owns_roid(jelly), |row| {
                    row.child(number_field(
                        "jelly-sim-roid",
                        "Stronkroid activation second",
                        &controls.sim_roid,
                        simulator.roid_timing,
                        window,
                        cx,
                    ))
                })
                .when(owns_revives(jelly), |row| {
                    row.child(select_field(
                        &controls.sim_revive,
                        "jelly-sim-revive",
                        "Automatic revival strategy",
                        false,
                        window,
                        cx,
                    ))
                })
                .child(number_field(
                    "jelly-sim-seed",
                    "Seed",
                    &controls.sim_seed,
                    f64::from(simulator.seed),
                    window,
                    cx,
                ))
                .child(run_button(
                    "jelly-sim-run",
                    "Simulate one operation",
                    running,
                    |state, cx| state.run_jelly_simulation(cx),
                    cx,
                )),
        )
        .child(Caption::new(
            "One seeded 60 FPS operation. Stage, Fever and the board sections come from this sandbox.",
        ));
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

/// The revive policies the select offers.
pub(crate) fn revive_choices() -> Vec<Choice<RevivePolicy>> {
    [
        RevivePolicy::None,
        RevivePolicy::ImmunoidFirst,
        RevivePolicy::AnchorFirst,
        RevivePolicy::DpsFirst,
    ]
    .into_iter()
    .map(|policy| Choice::new(policy, revive_label(policy)))
    .collect()
}

/// Reset from save, clear, remove mode and the three board-section actions.
fn actions(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let remove_mode = state.jelly_state.simulator.selected.is_none();
    div()
        .id("jelly-sim-actions")
        .flex()
        .flex_wrap()
        .gap(PANEL_GAP)
        .children(
            [
                ("jelly-sim-reset", "Reset from save", SimulatorAction::Reset),
                ("jelly-sim-clear", "Clear cells", SimulatorAction::Clear),
                ("jelly-sim-remove", "Remove mode", SimulatorAction::Remove),
                (
                    "jelly-sim-save-sections",
                    "Use save sections",
                    SimulatorAction::SaveSections,
                ),
                (
                    "jelly-sim-lock-sections",
                    "Lock all purchased sections",
                    SimulatorAction::LockSections,
                ),
                (
                    "jelly-sim-unlock-sections",
                    "Unlock all sections",
                    SimulatorAction::UnlockSections,
                ),
            ]
            .map(|(id, label, action)| {
                let entity = entity.clone();
                let button = Button::new(id).label(label).small();
                let button = if action == SimulatorAction::Remove && remove_mode {
                    button.primary()
                } else {
                    button.secondary()
                };
                button.on_click(move |_event, _window, cx| {
                    entity.update(cx, |state, cx| match action {
                        SimulatorAction::Reset => state.reset_jelly_simulator(cx),
                        SimulatorAction::Clear => state.clear_jelly_simulator(cx),
                        SimulatorAction::Remove => state.jelly_sim_remove_mode(cx),
                        SimulatorAction::SaveSections => state.jelly_sim_use_save_sections(cx),
                        SimulatorAction::LockSections => state.jelly_sim_lock_sections(cx),
                        SimulatorAction::UnlockSections => state.jelly_sim_unlock_all_sections(cx),
                    });
                })
            }),
        )
        .into_any_element()
}

#[derive(Clone, Copy, PartialEq)]
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
        .child(Caption::new(
            "Cell palette · pick a cell, then click a slot. Remove mode removes it.",
        ))
        .child(buttons)
        .into_any_element()
}

/// The hypothetical board sections (`Research[18]`).
fn sections(state: &AppState, cx: &mut Context<AppState>, jelly: &Jelly) -> AnyElement {
    let plots = &state.jelly_state.simulator.plots;
    let ids: Vec<usize> = data().plots.iter().map(|plot| plot.plot_id).collect();
    let entity = cx.entity();
    let group = ToggleGroup::new("jelly-sim-plots")
        .small()
        .children(data().plots.iter().map(|plot| {
            Toggle::new(format!("jelly-sim-plots-toggle-{}", plot.plot_id))
                .label(format!(
                    "Section {} · {} slots",
                    plot.plot_id + 1,
                    plot.width * plot.height
                ))
                .checked(plots.contains(&plot.plot_id))
        }))
        .on_click(move |checked, _window, cx| {
            let picked: Vec<usize> = ids
                .iter()
                .zip(checked)
                .filter_map(|(id, on)| on.then_some(*id))
                .collect();
            entity.update(cx, |state, cx| state.jelly_sim_set_plots(&picked, cx));
        });
    Card::new()
        .header(CardHeader::new("Board sections").description(
            "Section toggles are hypothetical; they change the sandbox's unlocked slots.",
        ))
        .child(div().flex().flex_wrap().child(group))
        .child(Caption::new(format!(
            "{} of {} section(s) unlocked · the save purchased {}",
            plots.len(),
            data().plots.len(),
            jelly.plots.len()
        )))
        .into_any_element()
}

/// The last action's answer, red when the action was refused.
fn message(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let simulator = &state.jelly_state.simulator;
    if simulator.message.is_empty() {
        return div().into_any_element();
    }
    let caption = Caption::new(simulator.message.clone());
    div()
        .id("jelly-sim-message")
        .role(Role::Label)
        .aria_label(simulator.message.clone())
        .child(if simulator.message_bad {
            caption.text_color(cx.theme().danger)
        } else {
            caption
        })
        .into_any_element()
}
