//! The encounter workspace (CS-03): the simulator options and the run.
//!
//! Ported from the Simulator options and the Run/Cancel pair in
//! `combat-sim-calc.html` (`startRun`, `cancelRun`, `setProgress`) and run
//! through the shared [`idleon_ui::runner::TaskRunner`], so the progress card,
//! the cancel action and the stale marker are the ones every other section
//! uses. The run itself is [`idleon_combat::encounter`]; its outcome is the
//! spawn/respawn/kill totals of one map, and the mechanics it cannot yet model
//! are listed beside it instead of being dropped.

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, div, px};
use idleon_combat::encounter::{self, Outcome};
use idleon_ui::format;

/// The upstream duration presets, in seconds.
const DURATIONS: [u64; 8] = [60, 300, 600, 1800, 3600, 21_600, 43_200, 86_400];

/// The encounter pane: options, the run in flight and its result.
pub(crate) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let task = state.combat_encounter(cx);
    let mut body = div()
        .id("combat-encounter-panel")
        .flex()
        .flex_col()
        .gap_3()
        .p_3()
        .child(options_card(state, &task, cx))
        .child(
            Caption::new(
                "The run replays the game's 10 ms tick over the map's spawn roster: monsters \
                 respawn on their saved timers and the stand-in kills one live monster per \
                 interval. The modelled paths draw no random numbers, so the map, the player \
                 and the duration fully describe the run.",
            )
            .text_color(muted),
        );
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let cancel = cx.entity();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "combat-encounter-run",
            "Running the encounter",
            "Runs off the UI thread; cancelling keeps the previous result.",
            progress,
            move |_window, cx| {
                cancel.update(cx, |state, cx| state.cancel_combat_encounter(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "combat-encounter-cancelled",
            task.result.is_some(),
        ));
    }
    match task.result.clone() {
        Some(result) => body = body.child(results_card(&result, task.stale, cx)),
        None if !task.running && !task.cancelled => {
            body = body.child(
                Banner::new(
                    "combat-encounter-empty",
                    Severity::Info,
                    "No encounter has run yet. Pick a map, a player and a duration, then Run.",
                )
                .into_any_element(),
            );
        }
        None => {}
    }
    body = body.child(unsupported_card(cx));
    ScrollArea::new("combat-encounter-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// The simulator options: encounter, map, player, kill interval and duration.
fn options_card(
    state: &AppState,
    task: &idleon_ui::runner::TaskState<Outcome>,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let options = state.combat_encounter_options();
    let running = task.running;
    let invalid = encounter::validate(&options).err();
    let valid = invalid.is_none();
    let run = entity.clone();
    let cancel = entity;
    let selected_map = options.map.to_string();
    let stand_in = matches!(options.player, encounter::Player::StandIn { .. });
    let kill_every = match options.player {
        encounter::Player::StandIn { kill_every_ms } => kill_every_ms as f64,
        encounter::Player::Idle => 1500.0,
    };

    let mut map_choices = Vec::with_capacity(encounter::maps().len());
    for map in encounter::maps() {
        map_choices.push(Choice::new(map.index.to_string(), encounter::map_name(map)));
    }
    let map_entity = cx.entity();
    let map_select = Select::new("combat-encounter-map", map_choices)
        .label("Map")
        .selected(SharedString::from(selected_map))
        .on_change(move |value, _window, cx| {
            if let Ok(index) = value.parse::<usize>() {
                map_entity.update(cx, |state, cx| state.set_combat_encounter_map(index, cx));
            }
        });

    let player_entity = cx.entity();
    let player_select = Select::new(
        "combat-encounter-player",
        vec![
            Choice::new("standIn", "Fixed kill interval (stand-in)"),
            Choice::new("idle", "Idle (no attacks)"),
        ],
    )
    .label("Player")
    .selected(SharedString::from(if stand_in {
        "standIn"
    } else {
        "idle"
    }))
    .on_change(move |value, _window, cx| {
        let idle = value.as_ref() == "idle";
        player_entity.update(cx, |state, cx| state.set_combat_encounter_idle(idle, cx));
    });

    let kill_entity = cx.entity();
    let kill_input = NumberInput::new("combat-encounter-kill", kill_every)
        .range(10.0, 60_000.0)
        .step(10.0)
        .precision(0)
        .size(ControlSize::Sm)
        .label("Kill every (ms)")
        .on_commit(move |value, _window, cx| {
            kill_entity.update(cx, |state, cx| {
                state.set_combat_encounter_kill_every(value, cx);
            });
        });

    let duration_entity = cx.entity();
    let duration = options.duration_ms / 1000;
    let duration_select = Select::new(
        "combat-encounter-duration",
        DURATIONS
            .iter()
            .map(|seconds| Choice::new(seconds.to_string(), duration_label(*seconds)))
            .collect::<Vec<Choice>>(),
    )
    .label("Duration")
    .selected(SharedString::from(duration.to_string()))
    .on_change(move |value, _window, cx| {
        if let Ok(seconds) = value.parse::<u64>() {
            duration_entity.update(cx, |state, cx| {
                state.set_combat_encounter_duration(seconds, cx);
            });
        }
    });

    Card::new()
        .header(
            CardHeader::new("Encounter")
                .description(
                    "Normal combat on one map. Killroy practice arrives with CS-05; the auto-mode \
                     player AI is not ported yet, so a run cannot measure character DPS.",
                )
                .action(Badge::new("Normal combat").tone(Tone::Accent)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(
                    div()
                        .id("combat-encounter-controls")
                        .flex()
                        .flex_wrap()
                        .items_end()
                        .gap_2()
                        .child(map_select)
                        .child(player_select)
                        .when(stand_in, |row| row.child(kill_input))
                        .child(duration_select)
                        .child(div().flex_1())
                        .child(
                            Button::new("combat-encounter-run", "Run")
                                .variant(ButtonVariant::Primary)
                                .size(ControlSize::Sm)
                                .icon(IconName::Play)
                                .disabled(running || !valid)
                                .on_click(move |_event, _window, cx| {
                                    run.update(cx, |state, cx| {
                                        state.run_combat_encounter(cx);
                                    });
                                }),
                        )
                        .child(
                            Button::new("combat-encounter-cancel", "Cancel")
                                .size(ControlSize::Sm)
                                .icon(IconName::X)
                                .disabled(!running)
                                .on_click(move |_event, _window, cx| {
                                    cancel.update(cx, |state, cx| {
                                        state.cancel_combat_encounter(cx);
                                    });
                                }),
                        ),
                )
                .child(Caption::new(match options.player {
                    encounter::Player::StandIn { .. } => {
                        "Stand-in player: stands on the spawn platform and, every interval, kills \
                         a live monster for exactly its remaining HP. Spawns and respawns follow \
                         the source; movement, attacks and skills do not."
                    }
                    encounter::Player::Idle => {
                        "Idle player: never attacks. Useful for checking the spawn and respawn \
                         roster without the stand-in's kills."
                    }
                }))
                .when_some(invalid, |card, error| {
                    card.child(Caption::new(error).text_color(muted))
                }),
        )
        .into_any_element()
}

/// The stored run: its totals, the kills by monster and how far it got.
fn results_card(result: &Outcome, stale: bool, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(map) = encounter::map(result.map) else {
        return div().into_any_element();
    };
    let mut header = CardHeader::new(format!("Run result · {}", encounter::map_name(map)))
        .description(format!(
            "{} · {} · {:.0}s of {:.0}s simulated · {} steps · {} frames",
            result.scene,
            player_label(result.player),
            result.simulated_ms as f64 / 1000.0,
            result.duration_ms as f64 / 1000.0,
            result.steps,
            result.frames,
        ));
    if stale {
        header = header.action(idleon_ui::run_chrome::stale_badge("combat-encounter"));
    }
    let mut card = Card::new().header(header);
    if stale {
        card = card.child(idleon_ui::run_chrome::stale_banner("combat-encounter"));
    }

    let kpis = div()
        .id("combat-encounter-kpis")
        .flex()
        .flex_wrap()
        .items_stretch()
        .gap_2()
        .child(kpi(
            "combat-encounter-kills",
            "Kills",
            &format::count(result.kills),
            &format!(
                "{} kill ticks, {} found nothing",
                format::count(result.kill_ticks()),
                format::count(result.idle_ticks)
            ),
            muted,
        ))
        .child(kpi(
            "combat-encounter-damage",
            "Damage dealt",
            &format::value(result.damage),
            &format!(
                "{}/s over the run",
                format::value(result.damage_per_second())
            ),
            muted,
        ))
        .child(kpi(
            "combat-encounter-rate",
            "Kills / hour",
            &format::value(result.kills_per_hour()),
            "Kills × 3,600 ÷ simulated seconds",
            muted,
        ))
        .child(kpi(
            "combat-encounter-interval",
            "Mean kill interval",
            &result.kill_interval_ms().map_or_else(
                || "—".to_string(),
                |interval| format!("{:.2}s", interval / 1000.0),
            ),
            "Simulated time ÷ kills",
            muted,
        ));

    let mut table = Table::new(["Monster", "Kills", "Share"]);
    let total = result.kills.max(1);
    for (kind, kills) in &result.kills_by_type {
        table = table.row([
            div()
                .id(SharedString::from(format!("combat-encounter-type-{kind}")))
                .role(Role::Label)
                .aria_label(format!("{kind}: {kills} kills"))
                .text_size(px(12.0))
                .child(SharedString::from(kind.clone()))
                .into_any_element(),
            div().child(format::count(*kills)).into_any_element(),
            div()
                .child(format!("{:.1}%", *kills as f64 * 100.0 / total as f64))
                .into_any_element(),
        ]);
    }
    let table = if result.kills_by_type.is_empty() {
        div()
            .id("combat-encounter-no-kills")
            .p_3()
            .text_size(px(12.0))
            .text_color(muted)
            .child("The run killed nothing: the idle player never attacks.")
            .into_any_element()
    } else {
        div()
            .id("combat-encounter-by-type")
            .p_3()
            .child(table)
            .into_any_element()
    };

    card.child(div().p_3().child(kpis))
        .child(table)
        .into_any_element()
}

/// The mechanics this build does not model, named rather than dropped.
fn unsupported_card(cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    Card::new()
        .header(
            CardHeader::new("Not simulated")
                .description(
                    "A run reports only what this build models. These parts of the upstream \
                     simulator are left out on purpose and listed here, not silently ignored.",
                )
                .action(
                    Badge::new(format!("{} items", encounter::unsupported().len()))
                        .tone(Tone::Warning),
                ),
        )
        .child(
            div()
                .id("combat-encounter-unsupported")
                .role(Role::Label)
                .aria_label(encounter::unsupported().join(" · "))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .children(encounter::unsupported().iter().map(|item| {
                    div()
                        .text_size(px(11.0))
                        .text_color(muted)
                        .child(SharedString::from(format!("• {item}")))
                })),
        )
        .into_any_element()
}

fn kpi(
    id: &'static str,
    label: &'static str,
    value: &str,
    caption: &str,
    muted: gpui::Hsla,
) -> AnyElement {
    Card::new()
        .child(
            div()
                .id(id)
                .role(Role::Label)
                .aria_label(format!("{label} {value}"))
                .flex()
                .flex_col()
                .gap_1()
                .p_3()
                .child(Caption::new(label).text_color(muted))
                .child(
                    div()
                        .text_size(px(20.0))
                        .child(SharedString::from(value.to_string())),
                )
                .child(Caption::new(caption.to_string()).text_color(muted)),
        )
        .into_any_element()
}

fn player_label(player: &str) -> &str {
    match player {
        "idle" => "Idle player",
        _ => "Fixed kill interval",
    }
}

fn duration_label(seconds: u64) -> String {
    match seconds {
        60 => "1 minute".into(),
        300 => "5 minutes".into(),
        600 => "10 minutes".into(),
        1800 => "30 minutes".into(),
        3600 => "1 hour".into(),
        21_600 => "6 hours".into(),
        43_200 => "12 hours".into(),
        _ => "24 hours".into(),
    }
}
