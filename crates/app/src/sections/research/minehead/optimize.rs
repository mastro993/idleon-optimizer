//! The Minehead strategy search: the knob ranges, the background search and the
//! top strategies, as `_renderOptimize` in `js/ui/minehead-ui.js`.

use super::{OptimizeInputs, Params};
use crate::state::AppState;
use ely_gpui_component::buttons::Button;
use ely_gpui_component::data_display::{Badge, Tone};
use ely_gpui_component::forms::Stepper;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::ActiveTheme as _;
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, SharedString, Styled, div, px};
use idleon_minehead::{MonteCarlo, Optimized, Options};
use idleon_ui::format;
use idleon_ui::run_chrome;

impl AppState {
    /// Start the strategy search over the current slider ranges.
    pub(crate) fn run_minehead_optimize(&mut self, cx: &mut Context<Self>) {
        let Some(inputs) = self.research.minehead.inputs.as_ref() else {
            return;
        };
        let run = OptimizeInputs {
            options: Options::from_inputs(inputs),
            grid: self.research.minehead.search_grid(),
            screen_trials: 200,
            final_trials: 2000,
            top_n: 20,
            seed: 42 + inputs.floor as u32,
        };
        self.minehead_optimize.update(cx, |runner, cx| {
            runner.start(cx, run, |inputs, report| {
                idleon_minehead::optimize(
                    &inputs.options,
                    &inputs.grid,
                    inputs.screen_trials,
                    inputs.final_trials,
                    inputs.top_n,
                    inputs.seed,
                    report,
                )
            });
        });
    }

    /// Cancel the strategy search; the last result stays.
    pub(crate) fn cancel_minehead_optimize(&mut self, cx: &mut Context<Self>) {
        self.cancel_run(
            self.minehead_optimize.clone(),
            "Cancelling the strategy search…",
            cx,
        );
    }

    fn minehead_optimize_inputs(&self) -> Option<OptimizeInputs> {
        let inputs = self.research.minehead.inputs.as_ref()?;
        Some(OptimizeInputs {
            options: Options::from_inputs(inputs),
            grid: self.research.minehead.search_grid(),
            screen_trials: 200,
            final_trials: 2000,
            top_n: 20,
            seed: 42 + inputs.floor as u32,
        })
    }
}

pub(super) fn pane(
    state: &mut AppState,
    _window: &mut gpui::Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if state.research.minehead.inputs.is_none() {
        return crate::ui::section::no_save(cx);
    }
    let muted = cx.theme().colors.fg_muted;
    let entity = cx.entity();
    let running = state.minehead_optimize.read(cx).running();
    let run_inputs = state.minehead_optimize_inputs();
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_optimize.read(cx).state(inputs));
    let combos = state.research.minehead.combo_count();

    // The knob ranges: one low and one high stepper per knob.
    let mut knobs = div().flex().flex_col().gap_1();
    for (index, range) in state.research.minehead.ranges.iter().enumerate() {
        let low_entity = entity.clone();
        let high_entity = entity.clone();
        let key = range.key;
        let precision = if range.step < 0.1 {
            2
        } else if range.step < 1.0 {
            1
        } else {
            0
        };
        knobs = knobs.child(
            div()
                .id(("mh-knob", index))
                .flex()
                .items_center()
                .gap_2()
                .h(px(28.0))
                .child(
                    div()
                        .w(px(130.0))
                        .flex_none()
                        .child(Caption::new(range.label).text_color(muted)),
                )
                .child(
                    Stepper::new(SharedString::from(format!("mh-lo-{key}")), range.lo)
                        .range(range.min, range.hi)
                        .step(range.step)
                        .precision(precision)
                        .on_change(move |value, _window, cx| {
                            low_entity.update(cx, |state, cx| {
                                if let Some(range) = state
                                    .research
                                    .minehead
                                    .ranges
                                    .iter_mut()
                                    .find(|range| range.key == key)
                                {
                                    range.lo = value;
                                }
                                cx.notify();
                            });
                        }),
                )
                .child(Caption::new("to").text_color(muted))
                .child(
                    Stepper::new(SharedString::from(format!("mh-hi-{key}")), range.hi)
                        .range(range.lo, range.max)
                        .step(range.step)
                        .precision(precision)
                        .on_change(move |value, _window, cx| {
                            high_entity.update(cx, |state, cx| {
                                if let Some(range) = state
                                    .research
                                    .minehead
                                    .ranges
                                    .iter_mut()
                                    .find(|range| range.key == key)
                                {
                                    range.hi = value;
                                }
                                cx.notify();
                            });
                        }),
                ),
        );
    }

    let reset = entity.clone();
    let run = entity.clone();
    let cancel = entity.clone();
    let controls = Card::new()
        .header(
            CardHeader::new("Search ranges")
                .description(format!(
                    "{combos} combos · 200 screening, 2,000 final trials"
                ))
                .action(
                    Button::new("mh-opt-reset", "Reset")
                        .size(ely_gpui_component::theme::ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            reset.update(cx, |state, cx| {
                                state.research.minehead.ranges = super::default_ranges();
                                cx.notify();
                            });
                        }),
                ),
        )
        .child(
            ScrollArea::new("mh-knobs-scroll")
                .vertical()
                .h(px(430.0))
                .child(div().p_3().min_h_0().child(knobs)),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .pb_3()
                .child(
                    Button::new(
                        "mh-opt-run",
                        if running {
                            "Optimizing…"
                        } else {
                            "Optimize Strategy"
                        },
                    )
                    .primary()
                    .icon(IconName::Play)
                    .disabled(running)
                    .on_click(move |_event, _window, cx| {
                        run.update(cx, |state, cx| state.run_minehead_optimize(cx));
                    }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-opt-cancel", "Cancel").on_click(
                        move |_event, _window, cx| {
                            cancel.update(cx, |state, cx| state.cancel_minehead_optimize(cx));
                        },
                    ))
                }),
        );

    let mut body = div().flex().flex_col().gap_3().min_h_0();
    if let Some(task) = task.as_ref() {
        if task.running {
            if let Some(progress) = task.progress.clone() {
                let cancel = entity.clone();
                body = body.child(run_chrome::progress_card(
                    "mh-optimize",
                    "Optimizing",
                    "Screens every combo, then re-runs the best twenty with 2,000 trials each.",
                    progress,
                    move |_window, cx| {
                        cancel.update(cx, |state, cx| state.cancel_minehead_optimize(cx));
                    },
                ));
            }
        } else if task.cancelled {
            body = body.child(run_chrome::cancelled_banner(
                "mh-optimize-cancelled",
                task.result.is_some(),
            ));
        }
    }
    if task.as_ref().is_some_and(|task| task.stale) {
        body = body.child(run_chrome::stale_banner("mh-optimize-stale"));
    }
    if let Some(result) = task.as_ref().and_then(|task| task.result.clone()) {
        body = body.child(results(&result, muted, cx));
    }
    let minehead = &state.research.minehead;
    if let Some(inferred) = minehead.inferred.as_ref() {
        body = body.child(inferred_card(
            inferred,
            minehead.use_inferred,
            minehead.games,
            minehead.decisions.len(),
            muted,
            cx,
        ));
    }

    div()
        .id("minehead-optimize")
        .flex()
        .gap_3()
        .min_h_0()
        .child(div().w(px(360.0)).flex_none().child(controls))
        .child(
            div()
                .id("minehead-optimize-results")
                .flex()
                .flex_col()
                .flex_1()
                .min_w_0()
                .min_h_0()
                .gap_3()
                .child(body),
        )
        .into_any_element()
}

fn results(result: &Optimized, muted: gpui::Hsla, cx: &mut Context<AppState>) -> AnyElement {
    let colors = cx.theme().colors.clone();
    let best = &result.best;
    let mut table = Table::new([
        "#", "Win%", "Avg Dmg", "T1 Dmg", "Turns", "Dmg/Cmt", "EV", "Min", "Cap", "HP",
    ]);
    for (index, candidate) in result.top.iter().enumerate() {
        let params = &candidate.params;
        let result = &candidate.result;
        table = table.row([
            div().child(format!("{}", index + 1)).into_any_element(),
            div()
                .text_color(win_tone(result.win_rate, &colors))
                .child(format!("{:.1}%", result.win_rate * 100.0))
                .into_any_element(),
            div()
                .child(format::value(result.avg_dmg))
                .into_any_element(),
            div()
                .child(format::value(result.avg_first_turn_dmg))
                .into_any_element(),
            div()
                .child(format!("{:.1}", result.avg_turns))
                .into_any_element(),
            div()
                .child(format::value(result.avg_dmg_per_commit))
                .into_any_element(),
            div()
                .child(format!("{:.1}", params.ev_multiplier))
                .into_any_element(),
            div()
                .child(format!("{:.0}", params.min_reveal))
                .into_any_element(),
            div()
                .child(format!("{:.0}%", params.mine_cap_pct * 100.0))
                .into_any_element(),
            div()
                .child(format!("{:.0}%", params.hp_threshold * 100.0))
                .into_any_element(),
        ]);
    }
    let table = ScrollArea::new("mh-opt-table")
        .vertical()
        .h(px(330.0))
        .child(div().min_w_0().child(table));
    let params = &best.params;
    Card::new()
        .header(
            CardHeader::new("Optimized strategies")
                .description(format!(
                    "{} combos · top {} re-run with 2,000 trials",
                    result.total_combos,
                    result.top.len()
                ))
                .action(
                    Badge::new(format!("{:.1}% win", best.result.win_rate * 100.0))
                        .tone(win_badge(best.result.win_rate)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .px_3()
                .py_2()
                .child(Caption::new(format!(
                    "Goldens {} · Cmt Min {:.0}% · Life {:.1} · T1 EV {:.1} · Crown {:.1} · Insta {:.0}%",
                    if params.golden_mine_pct < 1.0 {
                        format!("{:.0}%", params.golden_mine_pct * 100.0)
                    } else {
                        "Last".into()
                    },
                    params.commit_min * 100.0,
                    params.life_aggro,
                    params.turn1_ev_mul,
                    params.crown_chase,
                    params.insta_mode * 100.0,
                )))
                .child(Caption::new(format!(
                    "Blocks {} · Corner {:.1}× · Edge {:.1}× · Cluster {:.1}×",
                    if params.block_aggro {
                        "Aggro"
                    } else {
                        "Safe"
                    },
                    params.corner_bias,
                    params.edge_bias,
                    params.cluster_bias,
                )))
                .child(Caption::new(format!("Best: {}", summary(&best.result))).text_color(muted)),
        )
        .child(div().min_h_0().overflow_hidden().p_3().child(table))
        .into_any_element()
}

fn inferred_card(
    inferred: &idleon_minehead::Inferred,
    checked: bool,
    games: u32,
    decisions: usize,
    muted: gpui::Hsla,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(params) = inferred.params else {
        return div().into_any_element();
    };
    let entity = cx.entity();
    let use_entity = entity.clone();
    let reset = entity.clone();
    let toggle = ely_gpui_component::forms::Checkbox::new("mh-use-inferred", checked)
        .label("Use this strategy for Rank and Path")
        .on_change(move |on, _window, cx| {
            use_entity.update(cx, |state, cx| {
                state.research.minehead.use_inferred = on;
                cx.notify();
            });
        });
    Card::new()
        .header(
            CardHeader::new("Your strategy")
                .description(format!(
                    "{:.1}% agreement · {decisions} decisions over {games} games",
                    inferred.agreement * 100.0
                ))
                .action(Badge::new(inferred.profile.clone()).tone(Tone::Info)),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .px_3()
                .pb_3()
                .child(
                    Caption::new(format!(
                        "EV {:.1} · Min {:.0} · Cap {:.0}% · Goldens {} · Blocks {} · Life {:.1}",
                        params.ev_multiplier,
                        params.min_reveal,
                        params.mine_cap_pct * 100.0,
                        if params.golden_mine_pct < 1.0 {
                            format!("{:.0}%", params.golden_mine_pct * 100.0)
                        } else {
                            "Last".into()
                        },
                        if params.block_aggro { "Aggro" } else { "Safe" },
                        params.life_aggro,
                    ))
                    .text_color(muted),
                )
                .child(toggle)
                .child(
                    Button::new("mh-reset-inferred", "Reset play data")
                        .size(ely_gpui_component::theme::ControlSize::Sm)
                        .on_click(move |_event, _window, cx| {
                            reset.update(cx, |state, cx| {
                                state.research.minehead.reset_inference();
                                cx.notify();
                            });
                        }),
                ),
        )
        .into_any_element()
}

fn summary(result: &MonteCarlo) -> String {
    format!(
        "{} avg dmg · {} turns · {} dmg/commit",
        format::value(result.avg_dmg),
        format::value(result.avg_turns),
        format::value(result.avg_dmg_per_commit)
    )
}

fn win_tone(win_rate: f64, colors: &ely_gpui_component::theme::Palette) -> gpui::Hsla {
    if win_rate > 0.8 {
        colors.success
    } else if win_rate > 0.4 {
        colors.warning
    } else {
        colors.danger
    }
}

fn win_badge(win_rate: f64) -> Tone {
    if win_rate > 0.8 {
        Tone::Success
    } else if win_rate > 0.4 {
        Tone::Warning
    } else {
        Tone::Danger
    }
}

/// The badge the Rank and Path panes show for the chosen strategy.
pub(crate) fn strategy_label(params: &Params) -> String {
    format!(
        "EV {:.1} · Min {:.0}",
        params.ev_multiplier, params.min_reveal
    )
}
