//! The Play tab: the interactive Depth Charge floor, as `renderPlayfield` in
//! `js/ui/minehead-play.js`.
//!
//! Every click is recorded as a decision, and after three finished games the
//! tab infers the knobs that best explain the player's choices.

use crate::sections::research::minehead::State as MineheadState;
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::Badge;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, Hsla, IntoElement, ParentElement, Styled, div, px};
use idleon_minehead::{Decision, Game, Inputs, Options, Recorded, Reveal, Rng, Spatial};
use idleon_ui::format;

/// The game on screen and its log.
pub(crate) struct Play {
    pub(crate) game: Game,
    pub(crate) rng: Rng,
    pub(crate) log: Vec<String>,
}

impl Play {
    /// A fresh game on the save's floor.
    pub(crate) fn new(inputs: &Inputs) -> Self {
        let mut options = Options::from_inputs(inputs);
        // The Play tab never times out; only the search caps its turns.
        options.max_turns = 0;
        let mut game = Game::new(&options);
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.subsec_nanos() ^ elapsed.as_secs() as u32)
            .unwrap_or(1);
        let mut rng = Rng::new(seed);
        game.start_turn(&mut rng);
        Self {
            log: vec!["Game started! Click tiles to reveal them.".into()],
            game,
            rng,
        }
    }

    /// The context and spatial position of the click about to happen.
    fn record(&self, position: usize, choice: Decision) -> Recorded {
        Recorded {
            ctx: self.game.context(),
            choice,
            spatial: Some(spatial(&self.game, position)),
        }
    }

    /// Start the next turn after the current one ended, or finish the game.
    fn advance(&mut self, minehead: &mut MineheadState, state_status: &mut String) {
        if self.game.finished() {
            minehead.finish_game();
            *state_status = if self.game.result().won {
                format!(
                    "Boss defeated: {} damage.",
                    format::value(self.game.result().total_dmg)
                )
            } else {
                "Game over — all lives lost.".into()
            };
            self.log.push(if self.game.result().won {
                format!(
                    "BOSS DEFEATED! Total damage: {}",
                    format::value(self.game.result().total_dmg)
                )
            } else {
                "GAME OVER — all lives lost!".into()
            });
            return;
        }
        if !self.game.turret {
            self.game.start_turn(&mut self.rng);
            self.log
                .push(format!("—— Turn {} ——", self.game.turns_played));
        }
    }

    /// Log the result of one reveal.
    fn log_reveal(&mut self, position: usize, result: Reveal) {
        let value = self.game.grid[position];
        match result {
            Reveal::MineBlocked => self.log.push("Block absorbed a mine hit.".into()),
            Reveal::MineHit => {
                self.log
                    .push(format!("MINE HIT! Lives: {:.0}", self.game.lives.max(0.0)));
            }
            Reveal::Jackpot => self.log.push("JACKPOT! Cascade-revealing tiles…".into()),
            Reveal::Safe if self.game.crowns[position] => {
                self.log.push(format!(
                    "Crown {} / 3 ({} sets)",
                    self.game.crown_progress, self.game.crown_sets
                ));
            }
            _ => {
                if (40..50).contains(&value) {
                    self.log.push(format!("Currency tile +{}", value - 39));
                }
            }
        }
    }
}

impl AppState {
    /// Replace the game on screen with a fresh one.
    pub(crate) fn minehead_new_game(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = self.research.minehead.inputs.clone() else {
            return;
        };
        self.research.minehead.play = Some(Play::new(&inputs));
        self.status = "New Depth Charge game.".into();
        cx.notify();
    }

    /// Reveal one tile.
    pub(crate) fn minehead_click(&mut self, position: usize, cx: &mut Context<Self>) {
        let Some(inputs) = self.research.minehead.inputs.clone() else {
            return;
        };
        if self.research.minehead.play.is_none() {
            self.research.minehead.play = Some(Play::new(&inputs));
        }
        let Some(mut play) = self.research.minehead.play.take() else {
            return;
        };
        if play.game.finished() || play.game.revealed.get(position).copied().unwrap_or(true) {
            self.research.minehead.play = Some(play);
            return;
        }
        let choice = if play.game.golden.contains(&position) {
            Decision::Golden
        } else {
            Decision::Reveal
        };
        let decision = play.record(position, choice);
        let result = play.game.reveal(position, false, &mut play.rng);
        play.log_reveal(position, result);
        let minehead = &mut self.research.minehead;
        minehead.record(decision);
        // A cleared grid commits its damage by itself, as upstream.
        if play.game.remaining().0 + play.game.remaining().1 == 0.0 {
            play.game.clear_turn();
        }
        let mut status = String::new();
        play.advance(minehead, &mut status);
        if !status.is_empty() {
            self.status = status;
        }
        self.research.minehead.play = Some(play);
        cx.notify();
    }

    /// Commit this turn's damage.
    pub(crate) fn minehead_attack(&mut self, cx: &mut Context<Self>) {
        let Some(mut play) = self.research.minehead.play.take() else {
            return;
        };
        if !play.game.values.is_empty() {
            let ctx = play.game.context();
            let decision = Recorded {
                ctx,
                choice: Decision::Attack,
                spatial: None,
            };
            play.game.attack();
            play.log.push(format!(
                "Attacked for {} damage.",
                format::value(play.game.result().total_dmg - ctx.total_dmg_so_far)
            ));
            let minehead = &mut self.research.minehead;
            minehead.record(decision);
            let mut status = String::new();
            play.advance(minehead, &mut status);
            if !status.is_empty() {
                self.status = status;
            }
        }
        self.research.minehead.play = Some(play);
        cx.notify();
    }

    /// Spend one insta-reveal.
    pub(crate) fn minehead_insta(&mut self, cx: &mut Context<Self>) {
        let Some(mut play) = self.research.minehead.play.take() else {
            return;
        };
        if play.game.use_insta(&mut play.rng) {
            play.log.push("Insta-reveal found a mine safely.".into());
        }
        self.research.minehead.play = Some(play);
        cx.notify();
    }
}

pub(super) fn pane(state: &mut AppState, cx: &mut Context<AppState>) -> AnyElement {
    let Some(inputs) = state.research.minehead.inputs.clone() else {
        return crate::ui::section::no_save(cx);
    };
    let colors = cx.theme().colors.clone();
    if state.research.minehead.play.is_none() {
        state.research.minehead.play = Some(Play::new(&inputs));
    }
    let Some(play) = state.research.minehead.play.take() else {
        return div().into_any_element();
    };
    let entity = cx.entity();
    let (cols, _rows) = play.game.dims();
    let finished = play.game.finished();
    let lives = play.game.lives.max(0.0);
    let boss_hp = play.game.boss_hp();
    let total = play.game.total_dmg;
    let turn_damage = play.game.turn_damage();
    let (mines_left, safe_left, goldens_left) = play.game.remaining();
    let turn = play.game.turns_played;

    // The grid.
    let mut grid = div().flex().flex_wrap().gap_1().w(px(cols as f32 * 53.0));
    for position in 0..play.game.grid.len() {
        let revealed = play.game.revealed[position];
        let golden = play.game.golden.contains(&position);
        let value = play.game.grid[position];
        let (label, background, text) = if !revealed {
            if golden {
                ("★".to_string(), colors.warning, colors.warning)
            } else {
                ("?".to_string(), colors.sunken, colors.fg_muted)
            }
        } else if value == 0 {
            ("💣".to_string(), colors.danger, colors.danger)
        } else if value == 30 {
            ("🎰".to_string(), colors.warning, colors.warning)
        } else if value >= 40 {
            (format!("${}", value - 39), colors.info, colors.info)
        } else if value >= 20 {
            let multi = idleon_minehead::game()
                .tile_multipliers
                .get((value - 20) as usize)
                .copied()
                .unwrap_or(1.0);
            (format!("×{multi}"), colors.accent, colors.accent)
        } else {
            (value.to_string(), colors.success, colors.success)
        };
        let click = !revealed && !finished;
        let entity = entity.clone();
        grid = grid.child(
            div()
                .id(("mh-tile", position))
                .role(gpui::Role::Button)
                .aria_label(format!("Tile {}: {label}", position + 1))
                .flex_none()
                .w(px(50.0))
                .h(px(50.0))
                .flex()
                .items_center()
                .justify_center()
                .rounded_md()
                .border_1()
                .border_color(colors.border)
                .bg(background)
                .text_color(text)
                .text_size(px(15.0))
                .when(click, |tile| {
                    tile.cursor_pointer()
                        .hover(|tile| tile.opacity(0.8))
                        .on_click(move |_event, _window, cx| {
                            entity.update(cx, |state, cx| state.minehead_click(position, cx));
                        })
                })
                .child(label),
        );
    }

    let hud = |label: &str, value: String, color: Hsla| {
        div()
            .flex()
            .items_center()
            .gap_1()
            .child(Caption::new(label).text_color(colors.fg_muted))
            .child(div().text_color(color).text_size(px(13.0)).child(value))
    };
    let run = entity.clone();
    let attack = entity.clone();
    let insta = entity.clone();
    let hud_row = div()
        .flex()
        .flex_wrap()
        .gap_3()
        .child(hud(
            "Lives",
            format!("{lives:.0} / {:.0}", play.game.max_lives()),
            colors.danger,
        ))
        .when(play.game.max_blocks() > 0.0, |row| {
            row.child(hud(
                "Blocks",
                format!("{:.0}", play.game.blocks_left),
                colors.info,
            ))
        })
        .when(play.game.max_goldens() > 0.0, |row| {
            row.child(hud(
                "Goldens",
                format!("{:.0}", play.game.goldens_remaining),
                colors.warning,
            ))
        })
        .when(play.game.max_instas() > 0.0, |row| {
            row.child(hud(
                "Instas",
                format!("{:.0}", play.game.insta_left),
                colors.success,
            ))
        })
        .child(hud("Turn", format!("{turn}"), colors.fg))
        .child(hud(
            "Crowns",
            format!(
                "{} / 3 ({} sets)",
                play.game.crown_progress, play.game.crown_sets
            ),
            colors.accent,
        ));
    let mine_pct = if mines_left + safe_left > 0.0 {
        mines_left / (mines_left + safe_left) * 100.0
    } else {
        0.0
    };
    let info = format!(
        "Tiles: {} revealed · Mines left: ~{:.0}/{:.0} ({mine_pct:.0}%) · Turn dmg: {}",
        play.game.safe_revealed,
        mines_left,
        mines_left + safe_left,
        format::value(turn_damage)
    );
    let bar_fraction = if boss_hp > 0.0 {
        (total / boss_hp).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let damage_line = format!(
        "{} / {} {}{}",
        format::value(total),
        format::value(boss_hp),
        if turn_damage > 0.0 { "(+" } else { "" },
        if turn_damage > 0.0 {
            format!("{})", format::value(turn_damage))
        } else {
            String::new()
        }
    );
    let attack_disabled = finished || play.game.values.is_empty();
    let insta_disabled = finished || play.game.insta_left <= 0.0;
    let log: Vec<String> = play.log.iter().rev().take(12).cloned().collect();
    let mut log_list = div().flex().flex_col().gap_1();
    for (index, line) in log.iter().enumerate() {
        log_list = log_list.child(
            div()
                .id(("mh-log", index))
                .text_size(px(11.0))
                .text_color(colors.fg_muted)
                .child(line.clone()),
        );
    }
    let minehead = &state.research.minehead;
    let games = minehead.games;
    let decisions = minehead.decisions.len();
    let inferred = minehead.inferred.clone();
    let strategy_line = if games < 3 {
        format!(
            "Games: {games} · Decisions: {decisions} — play {} more game{} to start learning",
            3 - games,
            if 3 - games > 1 { "s" } else { "" }
        )
    } else {
        format!("Games: {games} · Decisions: {decisions} — auto-analyzing after each game")
    };
    let reset = entity.clone();
    let profile = inferred
        .as_ref()
        .map(|inferred| {
            format!(
                "{} · {:.0}% agreement",
                inferred.profile,
                inferred.agreement * 100.0
            )
        })
        .unwrap_or_default();
    state.research.minehead.play = Some(play);

    div()
        .id("minehead-play")
        .flex()
        .flex_col()
        .gap_3()
        .min_h_0()
        .child(
            Card::new()
                .header(
                    CardHeader::new(format!(
                        "Depth Charge — Floor {}: {}",
                        inputs.floor,
                        super::boss_name(inputs.floor)
                    ))
                    .description(info)
                    .action(Badge::new(damage_line).tone(
                        if total >= boss_hp {
                            ely_gpui_component::data_display::Tone::Success
                        } else {
                            ely_gpui_component::data_display::Tone::Neutral
                        },
                    )),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .px_3()
                        .pb_3()
                        .child(ely_gpui_component::motion::ProgressBar::new(
                            "mh-damage-bar",
                            bar_fraction,
                        ))
                        .child(hud_row)
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    Button::new("mh-play-attack", "Attack")
                                        .primary()
                                        .icon(IconName::Play)
                                        .disabled(attack_disabled)
                                        .on_click(move |_event, _window, cx| {
                                            attack.update(cx, |state, cx| {
                                                state.minehead_attack(cx);
                                            });
                                        }),
                                )
                                .child(
                                    Button::new("mh-play-insta", "Insta-Reveal")
                                        .icon(IconName::Zap)
                                        .disabled(insta_disabled)
                                        .on_click(move |_event, _window, cx| {
                                            insta.update(cx, |state, cx| {
                                                state.minehead_insta(cx);
                                            });
                                        }),
                                )
                                .child(Button::new("mh-play-new", "New Game").on_click(
                                    move |_event, _window, cx| {
                                        run.update(cx, |state, cx| {
                                            state.minehead_new_game(cx);
                                        });
                                    },
                                )),
                        ),
                ),
        )
        .child(
            div()
                .flex()
                .gap_3()
                .min_h_0()
                .child(
                    Card::new()
                        .header(
                            CardHeader::new("Playfield").description(format!(
                                "{:.0} goldens left on the grid",
                                goldens_left
                            )),
                        )
                        .child(div().p_3().child(grid)),
                )
                .child(
                    Card::new()
                        .header(
                            CardHeader::new("Log").description(format!("{turn} turn(s) played")),
                        )
                        .child(
                            ScrollArea::new("mh-play-log")
                                .vertical()
                                .h(px(240.0))
                                .child(div().p_3().child(log_list)),
                        ),
                ),
        )
        .child(
            Card::new()
                .header(
                    CardHeader::new("Your strategy")
                        .description(strategy_line)
                        .when(!profile.is_empty(), |header| {
                            header.action(
                                Badge::new(profile)
                                    .tone(ely_gpui_component::data_display::Tone::Info),
                            )
                        }),
                )
                .child(
                    div().flex().items_center().gap_2().px_3().pb_3().child(
                        Button::new("mh-play-reset", "Reset play data")
                            .size(ely_gpui_component::theme::ControlSize::Sm)
                            .on_click(move |_event, _window, cx| {
                                reset.update(cx, |state, cx| {
                                    state.research.minehead.reset_inference();
                                    cx.notify();
                                });
                            }),
                    ),
                ),
        )
        .into_any_element()
}

/// One click's position and its neighbourhood, for the inferrer.
fn spatial(game: &Game, position: usize) -> Spatial {
    let (cols, rows) = game.dims();
    let row = position / cols;
    let col = position % cols;
    let edge_row = row == 0 || row + 1 == rows;
    let edge_col = col == 0 || col + 1 == cols;
    let corner = edge_row && edge_col;
    let edge = (edge_row || edge_col) && !corner;
    let mut adj_mines = 0;
    let mut adj_safe = 0;
    let mut adj_unrevealed = 0;
    for dr in -1i32..=1 {
        for dc in -1i32..=1 {
            if dr == 0 && dc == 0 {
                continue;
            }
            let nr = row as i32 + dr;
            let nc = col as i32 + dc;
            if nr < 0 || nc < 0 || nr as usize >= rows || nc as usize >= cols {
                continue;
            }
            let neighbour = nr as usize * cols + nc as usize;
            if game.revealed[neighbour] {
                if game.grid[neighbour] == 0 {
                    adj_mines += 1;
                } else {
                    adj_safe += 1;
                }
            } else {
                adj_unrevealed += 1;
            }
        }
    }
    Spatial {
        idx: position,
        row,
        col,
        cols,
        rows,
        is_corner: corner,
        is_edge: edge,
        is_center: !edge_row && !edge_col,
        adj_mines,
        adj_safe,
        adj_unrevealed,
        click_order: game.safe_revealed,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_corner_click_reads_as_a_corner() {
        let save: serde_json::Value =
            serde_json::from_str(include_str!("../../../../../../fixtures/data.json"))
                .expect("save");
        let imported =
            idleon_import::import(&serde_json::to_string(&save).expect("json")).expect("import");
        let parsed = idleon_save::Save::new(save);
        let (inputs, _) = idleon_minehead::inputs(&parsed, &imported.simulation);
        let play = Play::new(&inputs);
        let spatial = spatial(&play.game, 0);
        assert!(spatial.is_corner);
        assert!(!spatial.is_edge);
        assert_eq!(spatial.cols, play.game.dims().0);
    }
}
