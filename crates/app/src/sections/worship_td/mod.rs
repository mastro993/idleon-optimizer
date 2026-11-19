//! Worship Tower Defense: the live simulator, its replay, plan and model panes.
//!
//! The simulator core is `idleon_worship`; this module owns the live run,
//! the tick pump, the map and the Plan and Model panes. Tower placement is
//! direct: pick a type from the palette, then a highlighted platform slot.
//! Clicking a standing tower selects it for a one-level upgrade or a 60% refund
//! sale. A finished run loads its replay: the map shows recorded frames and the
//! sidebar scrubs, plays and changes their speed. The Plan pane runs a build
//! plan through the same simulator engine, and the Optimizer pane searches
//! strategies with that engine before recommending one.

mod actions;

mod controls;
mod map;
mod model;
mod optimizer;
mod plan;
mod state;
mod table;

pub(crate) use controls::Controls;
pub(crate) use state::{OptimizeConfig, OptimizeRunInputs, PlanRunInputs, PlanSource, State};

use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Styled, Window, div, px};
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::select::Select;
use gpui_kit::component::slider::Slider;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Sizable as _, h_flex, v_flex};
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP, TEXT_BODY, Tone};
use idleon_worship::{self as worship, RunView};

/// The section's four panes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Tab {
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
    pub(crate) const ALL: [Tab; 4] = [Tab::Simulator, Tab::Plan, Tab::Model, Tab::Optimizer];

    pub(crate) fn id(self) -> &'static str {
        match self {
            Tab::Simulator => "worship-simulator-tab",
            Tab::Plan => "worship-plan-tab",
            Tab::Model => "worship-model-tab",
            Tab::Optimizer => "worship-optimizer-tab",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Tab::Simulator => "Simulator",
            Tab::Plan => "Plan",
            Tab::Model => "Model",
            Tab::Optimizer => "Optimizer",
        }
    }
}

/// The section's view.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    state.worship_ensure();
    state.worship_ensure_plan();
    state.worship_pump(cx);
    state.worship_replay_pump(cx);
    let tab = state.worship.tab;
    div()
        .id("worship-section")
        .flex()
        .flex_col()
        .size_full()
        .min_h_0()
        .gap_3()
        .child(header(state, cx))
        .child(tabs(state, cx))
        .child(match tab {
            Tab::Simulator => simulator(state, window, cx),
            Tab::Plan => plan::pane(state, cx),
            Tab::Model => model::pane(state, cx),
            Tab::Optimizer => optimizer::pane(state, window, cx),
        })
        .into_any_element()
}

fn tabs(state: &AppState, cx: &mut Context<AppState>) -> impl IntoElement {
    let entity = cx.entity();
    let selected = Tab::ALL
        .iter()
        .position(|tab| *tab == state.worship.tab)
        .unwrap_or_default();
    let tabs = Tab::ALL.map(|tab| (tab.id().into(), tab.label().into()));
    crate::ui::layout::tab_bar("worship-tabs", tabs, selected, move |index, _window, cx| {
        entity.update(cx, |state, cx| state.worship_set_tab(Tab::ALL[index], cx));
    })
}

fn simulator(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    // One view per frame: the map, the HUD and the sidebar all read it, and a
    // replay rebuilds it from its recorded frame.
    let view = state.worship.display_view();
    div()
        .id("worship-simulator-pane")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .gap_3()
        .child(controls(state, window, cx))
        .child(
            div()
                .id("worship-body")
                .flex()
                .flex_1()
                .min_h_0()
                .gap_3()
                .child(map::map(state, &view, window, cx))
                .child(sidebar(state, &view, window, cx)),
        )
        .into_any_element()
}

fn header(state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
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
        .child(
            Tag::new()
                .with_variant(Tone::Accent.into())
                .outline()
                .small()
                .child("Worship TD"),
        )
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

/// A small speed or toggle button, primary while it is the current pick.
fn pick_button(id: impl Into<gpui::ElementId>, label: String, picked: bool) -> Button {
    let button = Button::new(id).label(label).small();
    if picked {
        button.primary()
    } else {
        button.secondary()
    }
}

fn controls(state: &mut AppState, window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
    state.worship_controls.trial.show(
        controls::trial_choices(),
        Some(&state.worship.trial),
        window,
        cx,
    );
    let trial_select = div().flex_none().w(px(200.0)).child(
        Select::new(state.worship_controls.trial.state())
            .id("worship-trial")
            .accessibility_label("Trial")
            .small(),
    );

    let replaying = state.worship.replaying();
    let done = state.worship.run.as_ref().is_some_and(|run| run.done());
    let has_replay = state.worship.replay.is_some();
    let mut run_controls = h_flex().id("worship-run-controls").gap_1();
    if replaying {
        let live = entity.clone();
        let restart = entity.clone();
        run_controls = run_controls
            .child(
                Tag::new()
                    .with_variant(Tone::Info.into())
                    .outline()
                    .small()
                    .child("Replay"),
            )
            .child(
                Button::new("worship-live-view")
                    .label("Live view")
                    .small()
                    .on_click(move |_event, _window, cx| {
                        live.update(cx, |state, cx| state.worship_replay_close(cx));
                    }),
            )
            .child(
                Button::new("worship-restart")
                    .label("Restart")
                    .small()
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
        let speeds = [0.5, 1.0, 2.0, 5.0, 10.0, 30.0].map(|value| {
            let picked = entity.clone();
            pick_button(
                ("worship-speed", (value * 10.0) as u32),
                format!("{value}×"),
                (speed - value).abs() < f64::EPSILON,
            )
            .on_click(move |_event, _window, cx| {
                picked.update(cx, |state, cx| state.worship_set_speed(value, cx));
            })
        });
        let ranges = entity.clone();
        run_controls = run_controls
            .child(
                Button::new("worship-play")
                    .label(if playing { "Pause" } else { "Play" })
                    .primary()
                    .small()
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        play.update(cx, |state, cx| {
                            let next = !state.worship.playing;
                            state.worship_set_playing(next, cx);
                        });
                    }),
            )
            .child(
                Button::new("worship-step")
                    .label("Step")
                    .small()
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        step.update(cx, |state, cx| state.worship_step(cx));
                    }),
            )
            .child(
                Button::new("worship-skip")
                    .label("Next wave")
                    .small()
                    .disabled(done)
                    .on_click(move |_event, _window, cx| {
                        skip.update(cx, |state, cx| state.worship_skip(cx));
                    }),
            )
            .child(
                Button::new("worship-restart")
                    .label("Restart")
                    .small()
                    .on_click(move |_event, _window, cx| {
                        restart.update(cx, |state, cx| state.worship_restart(cx));
                    }),
            )
            .child(div().w(px(8.0)))
            .children(speeds)
            .child(div().w(px(8.0)))
            .child(
                pick_button("worship-ranges", "Ranges".into(), state.worship.ranges).on_click(
                    move |_event, _window, cx| {
                        ranges.update(cx, |state, cx| {
                            state.worship.ranges = !state.worship.ranges;
                            cx.notify();
                        });
                    },
                ),
            );
        if has_replay {
            let open = entity.clone();
            run_controls = run_controls.child(
                Button::new("worship-open-replay")
                    .label("Replay")
                    .secondary()
                    .small()
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
    let mut palette = worship::data()
        .towers
        .iter()
        .enumerate()
        .map(|(index, def)| {
            let unlocked = levels[index] >= 1;
            let cost = worship::new_tower_cost(index, placed) as u32;
            let picked = entity.clone();
            pick_button(
                ("worship-tower", index),
                if unlocked {
                    format!("{} · {cost}", def.name)
                } else {
                    format!("{} · locked", def.name)
                },
                index == tower,
            )
            .disabled(!unlocked)
            .on_click(move |_event, _window, cx| {
                picked.update(cx, |state, cx| state.worship_set_tower(index, cx));
            })
        })
        .collect::<Vec<_>>()
        .into_iter();

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
            h_flex()
                .id("worship-controls-row")
                .gap_3()
                .child(trial_select)
                .child(run_controls),
        );
    if replaying {
        card =
            card.child(div().id("worship-palette").child(
                Caption::new("Replay loaded · restart to build another run").text_color(muted),
            ));
    } else {
        let unlocked = levels.iter().filter(|level| **level >= 1).count();
        // Two lines that never wrap: a wrapping row keeps one line's height
        // and draws its second line over what follows (`docs/ui-kit/display.md`).
        let first: Vec<Button> = palette.by_ref().take(5).collect();
        card = card.child(
            v_flex()
                .id("worship-palette")
                .gap_1()
                .child(
                    h_flex()
                        .gap_1()
                        .child(
                            Caption::new(if unlocked > 0 {
                                format!("Tower palette · {unlocked} types unlocked")
                            } else {
                                "Import a save to unlock tower types".to_string()
                            })
                            .text_color(muted),
                        )
                        .children(first),
                )
                .child(h_flex().gap_1().children(palette)),
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
            h_flex()
                .id("worship-selected")
                .gap_2()
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
                .child(Button::new("worship-upgrade").label(cost).small().on_click(
                    move |_event, _window, cx| {
                        upgrade.update(cx, |state, cx| state.worship_upgrade(cx));
                    },
                ))
                .child(
                    Button::new("worship-sell")
                        .label(format!("Sell · {refund:.0}"))
                        .small()
                        .on_click(move |_event, _window, cx| {
                            sell.update(cx, |state, cx| state.worship_sell(cx));
                        }),
                )
                .child(
                    Button::new("worship-deselect")
                        .label("Clear")
                        .secondary()
                        .small()
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
                .child(Caption::new(message.clone()).text_color(cx.theme().danger)),
        );
    }
    card.into_any_element()
}

fn hud(state: &AppState, view: &RunView, muted: Hsla) -> AnyElement {
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
    let value = |id, label, value| hud_value(id, label, value, muted);
    // Two rows of four that never wrap (`docs/ui-kit/display.md`).
    v_flex()
        .id("worship-hud")
        .role(gpui::Role::Label)
        .aria_label(summary)
        .gap_2()
        .px_3()
        .py_2()
        .child(
            h_flex()
                .gap_4()
                .child(value("worship-time", "Time", format_time(view.t)))
                .child(value("worship-wave", "Wave", view.wave.to_string()))
                .child(value("worship-countdown", "Next wave", countdown))
                .child(value("worship-alive", "Alive", view.mobs.len().to_string())),
        )
        .child(
            h_flex()
                .gap_4()
                .child(value(
                    "worship-points",
                    "Points",
                    format!("{:.0}", view.pts),
                ))
                .child(value(
                    "worship-totem",
                    "Totem",
                    format!("{:.0}", view.totem),
                ))
                .child(value("worship-towers", "Towers", total.to_string()))
                .child(value("worship-best", "Best", best.to_string())),
        )
        .when_some(view.end_reason, |hud, reason| {
            hud.child(value("worship-ended", "Ended", reason.label().to_string()))
        })
        .into_any_element()
}

fn hud_value(id: &'static str, label: &'static str, value: String, muted: Hsla) -> AnyElement {
    div()
        .id(id)
        .role(gpui::Role::Label)
        .aria_label(format!("{label} {value}"))
        .flex()
        .items_center()
        .gap_1()
        .child(Caption::new(label).text_color(muted))
        .child(div().text_size(TEXT_BODY).child(value))
        .into_any_element()
}

fn sidebar(
    state: &mut AppState,
    view: &RunView,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let done = state.worship.run.as_ref().is_some_and(|run| run.done());
    let body = if done {
        let result = state
            .worship
            .run
            .as_mut()
            .map(|run| run.result())
            .expect("a done run exists");
        let totals = [
            ("Ended", result.end_reason.label().to_string()),
            ("Time", format_time(result.time_sec)),
            ("Waves", result.highest_wave.to_string()),
            ("Best", result.best_wave.to_string()),
            ("Kills", result.kills.to_string()),
            ("EXP", format!("{:.0}", result.exp)),
            ("Leaks", result.leaks.to_string()),
            ("Spent", format!("{:.0}", result.spent)),
            (
                "Totem",
                format!(
                    "{:.0} / {:.0}",
                    result.totem_left, state.worship_inputs.totem_hp
                ),
            ),
        ];
        kv_grid(&totals, muted)
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
    } else {
        let totals = [
            ("Time", format_time(view.t)),
            ("Wave", view.wave.to_string()),
            ("Alive", view.mobs.len().to_string()),
            ("Towers", view.towers.len().to_string()),
            ("Points", format!("{:.0}", view.pts)),
            ("Totem", format!("{:.0}", view.totem)),
        ];
        kv_grid(&totals, muted)
            .id("worship-result")
            .role(gpui::Role::Label)
            .aria_label(format!(
                "running · wave {} · alive {} · towers {} · points {:.0}",
                view.wave,
                view.mobs.len(),
                view.towers.len(),
                view.pts
            ))
    };
    let mut column = v_flex()
        .w(px(320.0))
        .gap_3()
        .child(hud(state, view, muted))
        .child(
            Card::new()
                .header(CardHeader::new(if done {
                    "Run totals"
                } else {
                    "Run so far"
                }))
                .child(body),
        );
    if state.worship.replay.is_some() {
        column = column.child(replay_card(state, window, cx));
    }
    column = column.child(
        div().id("worship-notes").px_1().child(
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

fn replay_card(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(replay) = state.worship.replay.as_ref() else {
        return div().into_any_element();
    };
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
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
    let speeds = [1.0, 2.0, 5.0, 10.0, 30.0].map(|value| {
        let picked = entity.clone();
        pick_button(
            ("worship-replay-speed", (value * 10.0) as u32),
            format!("{value}×"),
            (speed - value).abs() < f64::EPSILON,
        )
        .on_click(move |_event, _window, cx| {
            picked.update(cx, |state, cx| state.worship_replay_set_speed(value, cx));
        })
    });
    let scrubber = (last > 0).then(|| state.worship_scrubber(last, index, window, cx));
    Card::new()
        .header(
            CardHeader::new("Replay").description(format!(
                "Recorded every {interval:.1} s of game time · {len} frames · speeds are game seconds per second."
            )),
        )
        .child(
            v_flex()
                .id("worship-replay")
                .role(gpui::Role::Label)
                .aria_label(format!(
                    "Replay frame {index} of {last} at {} · wave {wave}",
                    format_time(time)
                ))
                .gap_2()
                .child(
                    h_flex()
                        .gap_2()
                        .child(
                            Button::new("worship-replay-play")
                                .label(if playing { "Pause" } else { "Play" })
                                .primary()
                                .small()
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
                    h_flex()
                        .id("worship-replay-speeds")
                        .gap_1()
                        .child(Caption::new("Speed").text_color(muted))
                        .children(speeds),
                )
                .when_some(scrubber, |card, scrubber| {
                    card.child(
                        div()
                            .id("worship-replay-scrub")
                            .child(Slider::new(&scrubber).accessibility_label("Replay position")),
                    )
                }),
        )
        .child(
            Caption::new(format!(
                "Scrub through {len} recorded frames; a frame is the live field at that tick."
            ))
            .text_color(muted),
        )
        .into_any_element()
}

/// Label-over-value pairs, three to a row.
fn kv_grid(pairs: &[(&'static str, String)], muted: Hsla) -> gpui::Div {
    v_flex().gap(PANEL_GAP).children(pairs.chunks(3).map(|row| {
        h_flex().gap_3().children(row.iter().map(|(label, value)| {
            v_flex()
                .flex_1()
                .gap_1()
                .child(Caption::new(*label).text_color(muted))
                .child(div().child(value.clone()))
        }))
    }))
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
