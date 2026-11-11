//! Worship Tower Defense: the live simulator, its replay, plan and model panes.
//!
//! The simulator core is `idleon_core::worship`; this module owns the live run,
//! the tick pump, the map and the Plan and Model panes. Tower placement is
//! direct: pick a type from the palette, then a highlighted platform slot.
//! Clicking a standing tower selects it for a one-level upgrade or a 60% refund
//! sale. A finished run loads its replay: the map shows recorded frames and the
//! sidebar scrubs, plays and changes their speed. The Plan pane runs a build
//! plan through the same simulator engine, and the Optimizer pane searches
//! strategies with that engine before recommending one.

mod map;
mod model;
mod optimizer;
mod plan;
mod state;

pub use state::{OptimizeConfig, OptimizeRunInputs, PlanRunInputs, PlanSource, State};

use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::data_display::Tone;
use ely_gpui_component::forms::{Choice, Select, Slider};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::shell::{TabBar, WindowTab};
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use idleon_core::worship;

/// The section's four panes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Tab {
    /// The live run, its palette, map and HUD.
    #[default]
    Simulator,
    /// The build plan, its stages and the plan run's final towers.
    Plan,
    /// The modelling assumptions and missing inputs.
    Model,
    /// The strategy search and its recommended plan.
    Optimizer,
}

impl Tab {
    pub const ALL: [Tab; 4] = [Tab::Simulator, Tab::Plan, Tab::Model, Tab::Optimizer];

    pub fn id(self) -> &'static str {
        match self {
            Tab::Simulator => "worship-simulator-tab",
            Tab::Plan => "worship-plan-tab",
            Tab::Model => "worship-model-tab",
            Tab::Optimizer => "worship-optimizer-tab",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Tab::Simulator => "Simulator",
            Tab::Plan => "Plan",
            Tab::Model => "Model",
            Tab::Optimizer => "Optimizer",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|tab| tab.id() == id)
    }
}

/// The section's view.
pub fn view(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    state.worship_ensure();
    state.worship_ensure_plan();
    state.worship_pump(cx);
    state.worship_replay_pump(cx);
    let muted = cx.theme().colors.fg_muted;
    let tab = state.worship.tab;
    div()
        .id("worship-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header(state, muted))
        .child(tabs(state, cx))
        .child(match tab {
            Tab::Simulator => simulator(state, window, cx),
            Tab::Plan => plan::pane(state, cx),
            Tab::Model => model::pane(state, cx),
            Tab::Optimizer => optimizer::pane(state, cx),
        })
        .into_any_element()
}

fn tabs(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let mut tabs = TabBar::new("worship-tabs").selected(SharedString::from(state.worship.tab.id()));
    for tab in Tab::ALL {
        tabs = tabs.tab(WindowTab::new(tab.id(), tab.label()));
    }
    tabs.on_select(move |id, _window, cx| {
        if let Some(tab) = Tab::from_id(id.as_ref()) {
            entity.update(cx, |state, cx| state.worship_set_tab(tab, cx));
        }
    })
}

fn simulator(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    div()
        .id("worship-simulator-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(controls(state, cx))
        .child(
            div()
                .id("worship-body")
                .flex()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(map::map(state, window, cx))
                .child(sidebar(state, cx)),
        )
        .into_any_element()
}

fn header(state: &AppState, muted: gpui::Hsla) -> AnyElement {
    let trial = worship::trial(state.worship.trial);
    let name = trial
        .map(|trial| trial.mob_def.name.clone())
        .unwrap_or_default();
    let map = trial.map(|trial| trial.map).unwrap_or(0);
    let best = state
        .worship_inputs
        .best_waves
        .get(state.worship.trial)
        .copied()
        .unwrap_or(0);
    let summary = format!(
        "{name} · map {map} · best wave {best} · seed {}",
        state.worship.seed
    );
    div()
        .id("worship-header")
        .flex()
        .flex_none()
        .items_center()
        .gap_3()
        .child(Badge::new("Worship TD").tone(Tone::Accent))
        .child(
            Caption::new(
                "A seeded 100 ms run; tower placement, ranges and points follow the game.",
            )
            .text_color(muted),
        )
        .child(div().flex_1())
        .child(
            div()
                .id("worship-summary")
                .role(gpui::Role::Label)
                .aria_label(summary.clone())
                .child(Caption::new(summary).text_color(muted)),
        )
        .into_any_element()
}

fn controls(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let trials: Vec<Choice> = worship::data()
        .trials
        .iter()
        .map(|trial| {
            Choice::new(
                trial.trial.to_string(),
                format!("{} · map {}", trial.mob_def.name, trial.map),
            )
            .note(format!(
                "{} charge · requires {} efficiency",
                trial.charge, trial.eff_req
            ))
        })
        .collect();
    let trial_entity = entity.clone();
    let trial_select = Select::new("worship-trial", trials)
        .label("Trial")
        .selected(state.worship.trial.to_string())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Ok(trial) = value.parse::<usize>() {
                trial_entity.update(cx, |state, cx| state.worship_set_trial(trial, cx));
            }
        });

    let replaying = state.worship.replaying();
    let done = state.worship.run.as_ref().is_some_and(|run| run.done());
    let has_replay = state.worship.replay.is_some();
    let mut run_controls = div()
        .id("worship-run-controls")
        .flex()
        .items_center()
        .flex_wrap()
        .gap_1();
    if replaying {
        let live = entity.clone();
        let restart = entity.clone();
        run_controls = run_controls
            .child(Badge::new("Replay").tone(Tone::Info))
            .child(
                Button::new("worship-live-view", "Live view")
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        live.update(cx, |state, cx| state.worship_replay_close(cx));
                    }),
            )
            .child(
                Button::new("worship-restart", "Restart")
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        restart.update(cx, |state, cx| state.worship_restart(cx));
                    }),
            );
    } else {
        let playing = state.worship.playing;
        let play = entity.clone();
        let step = entity.clone();
        let skip = entity.clone();
        let restart = entity.clone();
        let speed = state.worship.speed;
        let speeds: Vec<AnyElement> = [0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
            .into_iter()
            .map(|value| {
                let picked = entity.clone();
                Button::new(
                    ("worship-speed", (value * 10.0) as u32),
                    format!("{value}×"),
                )
                .variant(if (speed - value).abs() < f64::EPSILON {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    picked.update(cx, |state, cx| state.worship_set_speed(value, cx));
                })
                .into_any_element()
            })
            .collect();
        let ranges = entity.clone();
        let ranges_on = state.worship.ranges;
        run_controls = run_controls
            .child(
                Button::new("worship-play", if playing { "Pause" } else { "Play" })
                    .variant(ButtonVariant::Primary)
                    .size(ControlSize::Sm)
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        play.update(cx, |state, cx| {
                            let next = !state.worship.playing;
                            state.worship_set_playing(next, cx);
                        });
                    }),
            )
            .child(
                Button::new("worship-step", "Step")
                    .size(ControlSize::Sm)
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        step.update(cx, |state, cx| state.worship_step(cx));
                    }),
            )
            .child(
                Button::new("worship-skip", "Next wave")
                    .size(ControlSize::Sm)
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        skip.update(cx, |state, cx| state.worship_skip(cx));
                    }),
            )
            .child(
                Button::new("worship-restart", "Restart")
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        restart.update(cx, |state, cx| state.worship_restart(cx));
                    }),
            )
            .child(div().w(px(8.0)));
        for button in speeds {
            run_controls = run_controls.child(button);
        }
        run_controls = run_controls.child(div().w(px(8.0))).child(
            Button::new("worship-ranges", "Ranges")
                .variant(if ranges_on {
                    ButtonVariant::Primary
                } else {
                    ButtonVariant::Subtle
                })
                .size(ControlSize::Sm)
                .on_click(move |_event, _window, cx| {
                    ranges.update(cx, |state, cx| {
                        state.worship.ranges = !state.worship.ranges;
                        cx.notify();
                    });
                }),
        );
        if has_replay {
            let open = entity.clone();
            run_controls = run_controls.child(
                Button::new("worship-open-replay", "Replay")
                    .variant(ButtonVariant::Subtle)
                    .size(ControlSize::Sm)
                    .on_click(move |_event, _window, cx| {
                        open.update(cx, |state, cx| state.worship_replay_open(cx));
                    }),
            );
        }
    }

    let tower = state.worship.tower;
    let levels = state.worship_inputs.tower_levels;
    let placed = state
        .worship
        .view
        .as_ref()
        .map(|view| view.towers.len())
        .unwrap_or(0);
    let palette: Vec<AnyElement> = worship::data()
        .towers
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let selected = index == tower;
            let unlocked = levels[index] >= 1;
            let cost = worship::new_tower_cost(index, placed) as u32;
            let picked = entity.clone();
            Button::new(
                ("worship-tower", index),
                if unlocked {
                    format!("{} · {cost}", def.name)
                } else {
                    format!("{} · locked", def.name)
                },
            )
            .variant(if selected {
                ButtonVariant::Primary
            } else {
                ButtonVariant::Subtle
            })
            .size(ControlSize::Sm)
            .disabled(!unlocked)
            .on_click(move |_event, _window, cx| {
                picked.update(cx, |state, cx| state.worship_set_tower(index, cx));
            })
            .into_any_element()
        })
        .collect();

    let selected = state.worship.selected.and_then(|id| {
        state
            .worship
            .view
            .as_ref()?
            .towers
            .iter()
            .find(|tower| tower.id == id)
            .cloned()
    });
    let mut card = Card::new()
        .header(CardHeader::new("Simulator").description(if replaying {
            "A recorded run plays back here. Live view returns to the field it ended on."
        } else {
            "Pick a tower, then a slot on the map. A standing tower can be upgraded or sold."
        }))
        .child(
            div()
                .id("worship-controls-row")
                .flex()
                .flex_wrap()
                .items_end()
                .gap_3()
                .p_3()
                .child(div().w(px(200.0)).child(trial_select))
                .child(run_controls),
        );
    if replaying {
        card =
            card.child(div().id("worship-palette").px_3().pb_3().child(
                Caption::new("Replay loaded · restart to build another run").text_color(muted),
            ));
    } else {
        card = card.child(
            div()
                .id("worship-palette")
                .flex()
                .flex_wrap()
                .gap_1()
                .px_3()
                .pb_3()
                .child(
                    Caption::new(if levels.iter().any(|level| *level >= 1) {
                        format!(
                            "Tower palette · {} types unlocked",
                            levels.iter().filter(|level| **level >= 1).count()
                        )
                    } else {
                        "Import a save to unlock tower types".to_string()
                    })
                    .text_color(muted),
                )
                .children(palette),
        );
    }
    if let Some(tower) = selected.filter(|_| !replaying) {
        let upgrade = entity.clone();
        let sell = entity.clone();
        let clear = entity.clone();
        let cost = state
            .worship
            .run
            .as_ref()
            .and_then(|run| run.upgrade_cost(tower.id))
            .map(|cost| format!("Upgrade · {cost:.0}"))
            .unwrap_or_else(|| "Upgrade".into());
        let refund = state
            .worship
            .run
            .as_ref()
            .map(|run| run.refund(tower.id))
            .unwrap_or(0.0);
        card = card.child(
            div()
                .id("worship-selected")
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pb_3()
                .child(
                    Caption::new(format!(
                        "{} · level {} · platform {} · x {:.0} · {}",
                        worship::data().towers[tower.type_index].name,
                        tower.level,
                        tower.node,
                        tower.x,
                        if tower.traits.is_empty() {
                            "no traits".to_string()
                        } else {
                            format!("traits {}", tower.traits)
                        }
                    ))
                    .text_color(muted),
                )
                .child(div().flex_1())
                .child(
                    Button::new("worship-upgrade", cost)
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            upgrade.update(cx, |state, cx| state.worship_upgrade(cx));
                        }),
                )
                .child(
                    Button::new("worship-sell", format!("Sell · {refund:.0}"))
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            sell.update(cx, |state, cx| state.worship_sell(cx));
                        }),
                )
                .child(
                    Button::new("worship-deselect", "Clear")
                        .variant(ButtonVariant::Subtle)
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            clear.update(cx, |state, cx| state.worship_select(None, cx));
                        }),
                ),
        );
    }
    if let Some(message) = &state.worship.message {
        card = card.child(
            div()
                .id("worship-message")
                .role(gpui::Role::Label)
                .aria_label(message.clone())
                .px_3()
                .pb_3()
                .child(Caption::new(message.clone()).text_color(cx.theme().colors.danger)),
        );
    }
    card.into_any_element()
}

fn hud(state: &AppState, muted: gpui::Hsla) -> AnyElement {
    let view = state.worship.display_view();
    let best = state
        .worship_inputs
        .best_waves
        .get(state.worship.trial)
        .copied()
        .unwrap_or(0);
    let total = state
        .worship
        .view
        .as_ref()
        .map(|view| view.towers.len())
        .unwrap_or(0);
    let countdown = if view.countdown > 999.0 {
        "boss hold".to_string()
    } else {
        format!("{:.0}s", view.countdown.max(0.0))
    };
    let summary = format!(
        "time {:.1}s · wave {} · alive {} · points {:.0} · totem {:.0} · towers {}",
        view.t,
        view.wave,
        view.mobs.len(),
        view.pts,
        view.totem,
        total,
    );
    div()
        .id("worship-hud")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .flex()
        .flex_wrap()
        .items_center()
        .gap_4()
        .px_3()
        .py_2()
        .child(hud_value(
            "worship-time",
            "Time",
            format_time(view.t),
            muted,
        ))
        .child(hud_value(
            "worship-wave",
            "Wave",
            format!("{}", view.wave),
            muted,
        ))
        .child(hud_value(
            "worship-countdown",
            "Next wave",
            countdown,
            muted,
        ))
        .child(hud_value(
            "worship-alive",
            "Alive",
            format!("{}", view.mobs.len()),
            muted,
        ))
        .child(hud_value(
            "worship-points",
            "Points",
            format!("{:.0}", view.pts),
            muted,
        ))
        .child(hud_value(
            "worship-totem",
            "Totem",
            format!("{:.0}", view.totem),
            muted,
        ))
        .child(hud_value(
            "worship-towers",
            "Towers",
            format!("{total}"),
            muted,
        ))
        .child(hud_value("worship-best", "Best", format!("{best}"), muted))
        .when_some(view.end_reason, |row, reason| {
            row.child(hud_value(
                "worship-ended",
                "Ended",
                reason.label().to_string(),
                muted,
            ))
        })
        .into_any_element()
}

fn hud_value(
    id: &'static str,
    label: &'static str,
    value: String,
    muted: gpui::Hsla,
) -> AnyElement {
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label} {value}"))
        .flex()
        .items_center()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().text_size(px(13.0)).child(value))
        .into_any_element()
}

fn sidebar(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let view = state.worship.display_view();
    let mut column = div()
        .flex()
        .flex_col()
        .w(px(320.0))
        .gap_3()
        .child(hud(state, muted));
    let done = state.worship.run.as_ref().is_some_and(|run| run.done());
    let body = if done {
        let result = state
            .worship
            .run
            .as_mut()
            .map(|run| run.result())
            .expect("a done run exists");
        div()
            .id("worship-result")
            .role(gpui::Role::Label)
            .aria_label(format!(
                "ended {} · wave {} · best wave {} · kills {} · exp {:.0} · leaks {}",
                result.end_reason.label(),
                result.highest_wave,
                result.best_wave,
                result.kills,
                result.exp,
                result.leaks
            ))
            .flex()
            .flex_wrap()
            .gap_3()
            .p_3()
            .child(kv("Ended", result.end_reason.label().to_string(), muted))
            .child(kv("Time", format_time(result.time_sec), muted))
            .child(kv("Waves", result.highest_wave.to_string(), muted))
            .child(kv("Best", result.best_wave.to_string(), muted))
            .child(kv("Kills", result.kills.to_string(), muted))
            .child(kv("EXP", format!("{:.0}", result.exp), muted))
            .child(kv("Leaks", result.leaks.to_string(), muted))
            .child(kv("Spent", format!("{:.0}", result.spent), muted))
            .child(kv(
                "Totem",
                format!(
                    "{:.0} / {:.0}",
                    result.totem_left, state.worship_inputs.totem_hp
                ),
                muted,
            ))
    } else {
        div()
            .id("worship-result")
            .role(gpui::Role::Label)
            .aria_label(format!(
                "running · wave {} · alive {} · towers {} · points {:.0}",
                view.wave,
                view.mobs.len(),
                view.towers.len(),
                view.pts
            ))
            .flex()
            .flex_wrap()
            .gap_3()
            .p_3()
            .child(kv("Time", format_time(view.t), muted))
            .child(kv("Wave", view.wave.to_string(), muted))
            .child(kv("Alive", view.mobs.len().to_string(), muted))
            .child(kv("Towers", view.towers.len().to_string(), muted))
            .child(kv("Points", format!("{:.0}", view.pts), muted))
            .child(kv("Totem", format!("{:.0}", view.totem), muted))
    };
    column = column.child(
        Card::new()
            .header(CardHeader::new(if done {
                "Run totals"
            } else {
                "Run so far"
            }))
            .child(body),
    );
    if state.worship.replay.is_some() {
        column = column.child(replay_card(state, cx));
    }
    column = column.child(
        div()
            .id("worship-notes")
            .flex()
            .flex_col()
            .gap_1()
            .px_1()
            .child(
                Caption::new(
                    "The Plan tab runs a build plan through this engine; the Model tab lists every assumption and missing input. A finished run opens its replay in the sidebar.",
                )
                .text_color(muted),
            ),
    );
    div()
        .id("worship-sidebar")
        .flex()
        .flex_col()
        .flex_none()
        .w(px(320.0))
        .min_h_0()
        .gap_3()
        .child(column)
        .into_any_element()
}

fn replay_card(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(replay) = state.worship.replay.as_ref() else {
        return div().into_any_element();
    };
    let entity = cx.entity();
    let muted = cx.theme().colors.fg_muted;
    let len = replay.len();
    let last = len.saturating_sub(1);
    let index = state.worship.replay_index();
    let time = replay.frame(index).map(|frame| frame.t).unwrap_or(0.0);
    let wave = replay.frame(index).map(|frame| frame.wave).unwrap_or(0);
    let interval = replay.interval_sec();
    let playing = state.worship.replay_playing;
    let speed = state.worship.replay_speed;
    let position = format!("frame {index}/{last} · {} · wave {wave}", format_time(time));
    let play = entity.clone();
    let mut card = Card::new()
        .header(
            CardHeader::new("Replay").description(format!(
                "Recorded every {interval:.1} s of game time · {len} frames · speeds are game seconds per second."
            )),
        )
        .child(
            div()
                .id("worship-replay")
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Replay frame {index} of {last} at {} · wave {wave}",
                    format_time(time)
                ))
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new(
                                "worship-replay-play",
                                if playing { "Pause" } else { "Play" },
                            )
                            .variant(ButtonVariant::Primary)
                            .size(ControlSize::Sm)
                            .disabled(len < 2)
                            .on_click(move |_event, _window, cx| {
                                play.update(cx, |state, cx| {
                                    let next = !state.worship.replay_playing;
                                    state.worship_replay_set_playing(next, cx);
                                });
                            }),
                        )
                        .child(Caption::new(position).text_color(muted)),
                )
                .child(
                    div()
                        .id("worship-replay-speeds")
                        .flex()
                        .items_center()
                        .gap_1()
                        .child(Caption::new("Speed").text_color(muted))
                        .children([1.0, 2.0, 5.0, 10.0, 30.0].into_iter().map(|value| {
                            let picked = entity.clone();
                            Button::new(
                                ("worship-replay-speed", (value * 10.0) as u32),
                                format!("{value}×"),
                            )
                            .variant(if (speed - value).abs() < f64::EPSILON {
                                ButtonVariant::Primary
                            } else {
                                ButtonVariant::Subtle
                            })
                            .size(ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                picked.update(cx, |state, cx| {
                                    state.worship_replay_set_speed(value, cx);
                                });
                            })
                            .into_any_element()
                        })),
                )
                .when(last > 0, |card| {
                    let scrub = entity.clone();
                    card.child(
                        Slider::new("worship-replay-scrub", index as f64)
                            .range(0.0, last as f64)
                            .step(1.0)
                            .label("Replay position")
                            .on_change(move |value, _window, cx| {
                                scrub.update(cx, |state, cx| state.worship_replay_seek(value, cx));
                            }),
                    )
                }),
        );
    card = card.child(
        div().px_3().pb_3().child(
            Caption::new(format!(
                "Scrub through {len} recorded frames; a frame is the live field at that tick."
            ))
            .text_color(muted),
        ),
    );
    card.into_any_element()
}

fn kv(label: &'static str, value: String, muted: gpui::Hsla) -> AnyElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().child(value))
        .into_any_element()
}

fn format_time(seconds: f64) -> String {
    let seconds = seconds.max(0.0);
    let hours = (seconds / 3600.0).floor() as u32;
    let minutes = ((seconds - f64::from(hours) * 3600.0) / 60.0).floor() as u32;
    let rest = (seconds - f64::from(hours) * 3600.0 - f64::from(minutes) * 60.0).floor() as u32;
    if hours > 0 {
        format!("{hours}:{minutes:02}:{rest:02}")
    } else {
        format!("{minutes}:{rest:02}")
    }
}
