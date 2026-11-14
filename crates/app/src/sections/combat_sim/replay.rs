//! The replay tab (CS-07): the run's recorded frames, scrubbed and played.
//!
//! The frames come from [`idleon_combat::encounter`]'s recorder and the
//! delta codec in [`idleon_combat::replay`]; the scene art, monster sheets and
//! platform come from the extracted map table and resolve through the shared
//! asset manifest, so a missing file draws a labelled placeholder instead of
//! breaking the replay. The controls are the upstream ones: play/pause, the
//! `REPLAY_SPEEDS` buttons, a scrubber over the frames and, for a run whose
//! Den was projected, the wave boundaries to step between.

use super::RunResult;
use crate::assets::{Artwork, Manifest};
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::Slider;
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize, Palette};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, IntoElement, ParentElement, Role, SharedString, Styled, Window, div, img,
    px,
};
use idleon_combat::encounter as sim;
use idleon_combat::encounter::Art;
use idleon_combat::replay::Recording;
use idleon_ui::format;
use serde_json::Value as Json;

/// The upstream playback speeds (`REPLAY_SPEEDS`).
const SPEEDS: [f64; 7] = [0.5, 1.0, 2.0, 5.0, 20.0, 60.0, 300.0];

/// The replay pane: the scene, its controls and what the run cannot draw.
pub(crate) fn pane(
    state: &AppState,
    result: &RunResult,
    window: &Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let Some(replay) = result.run.replay.as_ref() else {
        return Card::new()
            .header(
                CardHeader::new("Replay")
                    .description("A replay is recorded by the run that produced it."),
            )
            .child(super::encounter::empty_panel(
                "combat-replay-empty",
                "This result carries no replay frames. Run the encounter again to record one.",
                muted,
            ))
            .into_any_element();
    };
    let len = replay.len();
    if len < 2 {
        return super::encounter::empty_panel(
            "combat-replay-short",
            "The replay has fewer than two frames: run for at least one frame interval.",
            muted,
        );
    }
    let last = len - 1;
    let index = state.combat.replay_pos.floor().clamp(0.0, last as f64) as usize;
    let time = replay.time_ms(index).unwrap_or(0.0);
    let interval = replay.interval_ms();
    let bytes = replay.bytes();
    let playing = state.combat.replay_playing;
    let speed = state.combat.replay_speed;
    let entity = cx.entity();
    let map = sim::map(result.run.map);
    let art = map.map(|map| &map.art);
    let scale = art.map_or(1.0, |art| scale_for(window, art));
    let colors = cx.theme().colors.clone();

    let play = entity.clone();
    let scrub = entity.clone();
    let mut card = Card::new().header(
        CardHeader::new("Replay")
            .description(format!(
                "{len} frames every {interval} ms · {} · {} · {}",
                format::duration(time / 1000.0),
                format::duration(result.run.simulated_ms as f64 / 1000.0),
                format_size(bytes),
            ))
            .action(source_badge(result, muted)),
    );
    card = card.child(
        div()
            .id("combat-replay-controls")
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .px_3()
            .pt_3()
            .child(
                Button::new("combat-replay-play", if playing { "Pause" } else { "Play" })
                    .variant(ButtonVariant::Primary)
                    .size(ControlSize::Sm)
                    .icon(if playing {
                        IconName::Pause
                    } else {
                        IconName::Play
                    })
                    .on_click(move |_event, _window, cx| {
                        play.update(cx, |state, cx| {
                            state.combat_replay_set_playing(!playing, cx);
                        });
                    }),
            )
            .child(
                div()
                    .id("combat-replay-speeds")
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(Caption::new("Speed").text_color(muted))
                    .children(SPEEDS.into_iter().map(|value| {
                        let picked = entity.clone();
                        Button::new(
                            format!("combat-replay-speed-{}", (value * 10.0) as u32),
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
                                state.combat_replay_set_speed(value, cx);
                            });
                        })
                        .into_any_element()
                    })),
            )
            .child(div().flex_1())
            .child(
                div()
                    .id("combat-replay-position")
                    .role(Role::Label)
                    .aria_label(format!(
                        "Replay frame {index} of {last} at {}",
                        format::duration(time / 1000.0)
                    ))
                    .text_size(px(12.0))
                    .text_color(muted)
                    .child(format!(
                        "frame {index}/{last} · {}/{} · {}",
                        format::duration(time / 1000.0),
                        format::duration(result.run.simulated_ms as f64 / 1000.0),
                        format_size(bytes)
                    )),
            ),
    );
    card = card.child(
        div().px_3().pt_2().child(
            Slider::new("combat-replay-scrub", index as f64)
                .range(0.0, last as f64)
                .step(1.0)
                .label("Replay position")
                .on_change(move |value, _window, cx| {
                    scrub.update(cx, |state, cx| state.combat_replay_seek(value, cx));
                }),
        ),
    );
    card = card.child(wave_controls(result, time, &entity, muted));
    if let (Some(art), Some(map)) = (art, map) {
        card = card.child(
            div().p_3().child(
                div()
                    .id("combat-replay-scene-frame")
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(scene(state, art, map, replay, index, scale, &colors)),
            ),
        );
    }
    card = card.child(
        div()
            .px_3()
            .pb_3()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                Caption::new(
                    "The replay draws the run itself: the map's background and platform, each \
                     monster on its spawn node and the stand-in on the scene's player node. \
                     Monsters do not move, so the spawn x within a platform is the platform's \
                     centre. Positions come from the map table, art and monster sheets through the \
                     asset manifest, and a missing file draws its placeholder.",
                )
                .text_color(muted),
            )
            .child(
                Caption::new(format!(
                    "{} · recorded every {interval} ms · {} of {} simulated. The player's attacks, \
                     projectiles and damage numbers are the auto-mode AI this build does not run, \
                     so the replay shows spawns, kills and respawns.",
                    if result.source.is_some() {
                        "Killroy replay"
                    } else {
                        "Encounter run"
                    },
                    format::duration(time / 1000.0),
                    format::duration(result.run.simulated_ms as f64 / 1000.0),
                ))
                .text_color(muted),
            ),
    );
    card.child(capture_row(result, state, cx))
        .into_any_element()
}

/// The source line: whose replay this is.
fn source_badge(result: &RunResult, muted: gpui::Hsla) -> AnyElement {
    if let Some(source) = &result.source {
        return div()
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .id("combat-replay-source")
                    .role(Role::Label)
                    .aria_label(format!("Killroy replay {}", source.label))
                    .child(
                        Badge::new(format!("Killroy replay · {}", source.label)).tone(Tone::Info),
                    ),
            )
            .child(
                Caption::new(format!(
                    "Map {} · {} s · seed {}",
                    source.map_idx,
                    format::value(source.seconds),
                    source.seed
                ))
                .text_color(muted),
            )
            .into_any_element();
    }
    div()
        .id("combat-replay-source")
        .role(Role::Label)
        .aria_label("Encounter run")
        .child(Badge::new("Encounter run").tone(Tone::Accent))
        .into_any_element()
}

/// Previous and next wave, and the wave the playhead is in.
///
/// The boundaries come from the Den projection, which runs on the same clock
/// and the same kill interval as the stream. A run without a Den says so.
fn wave_controls(
    result: &RunResult,
    time: f64,
    entity: &gpui::Entity<AppState>,
    muted: gpui::Hsla,
) -> AnyElement {
    let Some(den) = result.den.as_ref() else {
        return div()
            .id("combat-replay-waves")
            .px_3()
            .pt_2()
            .child(
                Caption::new(
                    "No wave controls: waves are The Den's, and this save has no Den projection \
                     for this run.",
                )
                .text_color(muted),
            )
            .into_any_element();
    };
    let waves = &den.waves;
    let next = waves
        .iter()
        .find(|wave| wave.t_ms as f64 > time + 0.5)
        .map(|wave| wave.t_ms);
    let previous = waves
        .iter()
        .rev()
        .find(|wave| (wave.t_ms as f64) < time - 0.5)
        .map(|wave| wave.t_ms);
    let current = waves
        .iter()
        .rev()
        .find(|wave| wave.t_ms as f64 <= time)
        .map(|wave| wave.wave)
        .unwrap_or(0);
    let back = entity.clone();
    let forward = entity.clone();
    div()
        .id("combat-replay-waves")
        .flex()
        .items_center()
        .gap_2()
        .px_3()
        .pt_2()
        .child(
            Button::new("combat-replay-wave-prev", "Wave")
                .size(ControlSize::Sm)
                .icon(IconName::SkipBack)
                .disabled(previous.is_none())
                .on_click(move |_event, _window, cx| {
                    let target = previous.unwrap_or(0);
                    back.update(cx, |state, cx| state.combat_replay_seek_time(target, cx));
                }),
        )
        .child(
            Button::new("combat-replay-wave-next", "Wave")
                .size(ControlSize::Sm)
                .icon(IconName::SkipForward)
                .disabled(next.is_none())
                .on_click(move |_event, _window, cx| {
                    if let Some(target) = next {
                        forward.update(cx, |state, cx| {
                            state.combat_replay_seek_time(target, cx);
                        });
                    }
                }),
        )
        .child(
            div()
                .id("combat-replay-wave")
                .role(Role::Label)
                .aria_label(format!(
                    "Den wave {current} of {}",
                    den.snapshot.wave as u32
                ))
                .text_size(px(12.0))
                .text_color(muted)
                .child(format!(
                    "Den wave {current} of {} · {} waves recorded",
                    den.snapshot.wave as u32,
                    format::count(waves.len() as u64),
                )),
        )
        .into_any_element()
}

/// The captured Killroy replay: capture one from a finished practice, or open
/// the stored one, which re-runs its map, duration and seed.
fn capture_row(result: &RunResult, state: &AppState, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().colors.fg_muted;
    let capture = cx.entity();
    let open = cx.entity();
    let forget = cx.entity();
    let captured = state.combat.killroy_replay.clone();
    let can_capture = result.killroy.is_some() && state.save.is_some();
    let mut row = div()
        .id("combat-replay-killroy")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_2()
        .border_t_1()
        .border_color(cx.theme().colors.border)
        .px_3()
        .py_3()
        .child(
            Button::new("combat-replay-capture", "Capture Killroy replay")
                .size(ControlSize::Sm)
                .icon(IconName::Save)
                .disabled(!can_capture || captured.is_some())
                .on_click(move |_event, _window, cx| {
                    capture.update(cx, |state, cx| state.capture_combat_killroy_replay(cx));
                }),
        );
    match captured {
        Some(replay) => {
            row = row
                .child(
                    div()
                        .id("combat-replay-captured")
                        .role(Role::Label)
                        .aria_label(format!(
                            "Captured Killroy replay {} on map {}",
                            replay.label, replay.map_idx
                        ))
                        .text_size(px(12.0))
                        .child(format!(
                            "Captured: {} · map {} · {} s · seed {}",
                            replay.label,
                            replay.map_idx,
                            format::value(replay.seconds),
                            replay.seed
                        )),
                )
                .child(
                    Button::new("combat-replay-open", "Open and run")
                        .variant(ButtonVariant::Primary)
                        .size(ControlSize::Sm)
                        .icon(IconName::Play)
                        .on_click(move |_event, _window, cx| {
                            open.update(cx, |state, cx| state.open_combat_killroy_replay(cx));
                        }),
                )
                .child(
                    Button::new("combat-replay-forget", "Forget")
                        .size(ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            forget.update(cx, |state, cx| {
                                state.forget_combat_killroy_replay(cx);
                            });
                        }),
                );
        }
        None => {
            row = row.child(
                Caption::new(
                    "A Killroy practice run can be captured as a replay: it stores the map, \
                     duration and seed, and opening it re-runs them. The capture is checked with \
                     the upstream rules and lives in this session, like the upstream page's \
                     handoff token.",
                )
                .text_color(muted),
            );
        }
    }
    row.into_any_element()
}

/// The scene board: the map art, the platforms and the frame's monsters.
fn scene(
    state: &AppState,
    art: &Art,
    map: &idleon_combat::encounter::Map,
    replay: &Recording,
    index: usize,
    scale: f32,
    colors: &Palette,
) -> AnyElement {
    let manifest = state.assets.clone();
    let width = art.width as f32 * scale;
    let height = art.height as f32 * scale;
    let frame = replay.frame(index).unwrap_or(Json::Null);
    let time = replay.time_ms(index).unwrap_or(0.0);
    let monsters = frame["monsters"].as_array().cloned().unwrap_or_default();
    let mut board = div()
        .id("combat-replay-scene")
        .role(Role::Group)
        .aria_label(format!(
            "{} replay at {} with {} monsters",
            map.name,
            format::duration(time / 1000.0),
            format::count(monsters.len() as u64)
        ))
        .relative()
        .flex_none()
        .w(px(width))
        .h(px(height))
        .rounded(px(8.0))
        .overflow_hidden()
        .bg(colors.bg)
        .border_1()
        .border_color(colors.border);
    if let Some(path) = art
        .bg
        .as_deref()
        .and_then(|bg| artwork(&manifest, &format!("combat-sim/{bg}")))
    {
        // `6bg.png` is a 1200x675 cover image behind the platform art.
        let (bg_w, bg_h) = (1200.0, 675.0);
        let cover = (art.width / bg_w).max(art.height / bg_h);
        let draw_w = bg_w * cover;
        let draw_h = bg_h * cover;
        let left = (art.width - draw_w) / 2.0;
        let top = art.height - draw_h;
        board = board.child(
            img(path)
                .absolute()
                .left(px(left as f32 * scale))
                .top(px(top as f32 * scale))
                .w(px(draw_w as f32 * scale))
                .h(px(draw_h as f32 * scale)),
        );
    }
    if let Some(path) = art
        .plat
        .as_deref()
        .and_then(|plat| artwork(&manifest, &format!("combat-sim/{plat}")))
    {
        board = board.child(
            img(path)
                .absolute()
                .left_0()
                .top_0()
                .w(px(width))
                .h(px(height)),
        );
    } else {
        // No platform art: draw the collision geometry instead of nothing.
        for node in &art.nodes {
            board = board.child(
                div()
                    .absolute()
                    .left(px(node.x as f32 * scale))
                    .top(px(node.y as f32 * scale))
                    .w(px((node.length - node.x).max(0.0) as f32 * scale))
                    .h(px(2.0))
                    .bg(colors.fg_muted.opacity(0.4)),
            );
        }
    }
    if let Some(node) = art.nodes.get(art.player_node) {
        let (w, h) = (94.0, 50.0);
        let x = node.x + (node.length - node.x - w) / 2.0;
        let y = node.y - h;
        board = board.child(
            div()
                .id("combat-replay-player")
                .role(Role::Label)
                .aria_label("Stand-in player")
                .absolute()
                .left(px(x as f32 * scale))
                .top(px(y as f32 * scale))
                .w(px(w as f32 * scale))
                .h(px(h as f32 * scale))
                .rounded(px(4.0))
                .border_2()
                .border_color(colors.info)
                .child(
                    div()
                        .absolute()
                        .left_0()
                        .bottom_0()
                        .px(px(2.0))
                        .text_size(px(9.0))
                        .text_color(colors.fg)
                        .bg(colors.bg.opacity(0.6))
                        .child("stand-in"),
                ),
        );
    }
    let tick = (time / 100.0) as u64;
    for monster in &monsters {
        let id = monster["id"].as_u64().unwrap_or(0) as usize;
        let sprite = map
            .mobs
            .get(id.saturating_sub(1))
            .and_then(|mob| mob.sprite.as_ref());
        let alive = monster["alive"].as_f64().unwrap_or(0.0) == 1.0;
        let node = monster["node"].as_u64().unwrap_or(0) as usize;
        let name = map
            .mobs
            .get(id.saturating_sub(1))
            .map(|mob| idleon_combat::drops::monster_name(&mob.kind))
            .unwrap_or_else(|| format!("Monster {id}"));
        let x = monster["x"].as_f64().unwrap_or(0.0) as f32 * scale;
        let y = monster["y"].as_f64().unwrap_or(0.0) as f32 * scale;
        let hp = monster["hp"].as_f64().unwrap_or(0.0);
        let hp_tot = monster["hpTot"].as_f64().unwrap_or(0.0).max(1.0);
        let (fw, fh) = sprite.map_or((50.0, 50.0), |sprite| (sprite.fw, sprite.fh));
        let w = fw as f32 * scale;
        let h = fh as f32 * scale;
        let mut marker = div()
            .id(SharedString::from(format!("combat-replay-monster-{id}")))
            .role(Role::Label)
            .aria_label(format!(
                "{name} on platform {node}: {} hp, {}",
                format::value(hp.max(0.0)),
                if alive { "alive" } else { "respawning" }
            ))
            .absolute()
            .left(px(x))
            .top(px(y))
            .w(px(w.max(2.0)))
            .h(px(h.max(2.0)))
            .overflow_hidden()
            .when(!alive, |marker| marker.opacity(0.35));
        if let Some(sprite) = sprite {
            let (first, count) = if !alive && sprite.death_frame < sprite.frames {
                (sprite.death_frame, sprite.frames - sprite.death_frame)
            } else if sprite.moving_frame > 0 {
                (0, sprite.moving_frame)
            } else {
                (0, sprite.death_frame.max(1))
            };
            let frame = first + (tick as usize % count.max(1));
            let column = frame % sprite.across.max(1);
            let row = frame / sprite.across.max(1);
            if let Some(path) = artwork(&manifest, &format!("combat-sim/{}", sprite.file)) {
                marker = marker.child(
                    img(path)
                        .absolute()
                        .left(px(-(column as f32) * w))
                        .top(px(-(row as f32) * h))
                        .w(px(sprite.across as f32 * w))
                        .h(px(sprite.down as f32 * h)),
                );
            } else {
                marker = marker.child(
                    div()
                        .absolute()
                        .left_0()
                        .top_0()
                        .w(px(w))
                        .h(px(h))
                        .rounded(px(3.0))
                        .bg(colors.warning.opacity(0.75)),
                );
            }
        } else {
            marker = marker.child(
                div()
                    .absolute()
                    .left_0()
                    .top_0()
                    .w(px(w))
                    .h(px(h))
                    .rounded(px(3.0))
                    .bg(colors.danger.opacity(0.7)),
            );
        }
        if alive && hp < hp_tot {
            marker = marker.child(
                div()
                    .absolute()
                    .left_0()
                    .top(px(-(4.0)))
                    .w(px(w))
                    .h(px(3.0))
                    .bg(colors.bg.opacity(0.7))
                    .child(
                        div()
                            .w(px(w * (hp / hp_tot).clamp(0.0, 1.0) as f32))
                            .h(px(3.0))
                            .bg(colors.success),
                    ),
            );
        }
        board = board.child(marker);
    }
    if monsters.is_empty() {
        board = board.child(
            div()
                .absolute()
                .left_0()
                .top_0()
                .p_2()
                .text_size(px(11.0))
                .text_color(colors.fg_muted)
                .child("The roster is empty."),
        );
    }
    board.into_any_element()
}

fn artwork(manifest: &Manifest, reference: &str) -> Option<std::path::PathBuf> {
    match manifest.artwork(reference) {
        Artwork::Ready(path) => Some(path.to_path_buf()),
        Artwork::Missing => None,
    }
}

fn scale_for(window: &Window, art: &Art) -> f32 {
    let viewport = window.viewport_size();
    let available_w = (f32::from(viewport.width) - 420.0).max(320.0);
    let available_h = (f32::from(viewport.height) - 520.0).max(200.0);
    (available_w / art.width as f32)
        .min(available_h / art.height as f32)
        .clamp(0.2, 1.0)
}

fn format_size(bytes: usize) -> String {
    if bytes >= 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}
