//! The Minehead strategy search: the knob ranges, the background search and the
//! top strategies.

use super::super::controls::stepper;
use super::{OptimizeInputs, Params};
use crate::sections::research::sushi::columns::{self, Column};
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{
    AnyElement, Context, Entity, IntoElement, ParentElement, SharedString, Styled, Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::Sizable as _;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::input::InputState;
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::{Table, TableBody, TableRow};
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _};
use idleon_minehead::{MonteCarlo, Optimized, Options};
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, Tone, show_number};

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
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if state.research.minehead.inputs.is_none() {
        return crate::ui::section::no_save(cx);
    }
    let entity = cx.entity();
    let running = state.minehead_optimize.read(cx).running();
    let run_inputs = state.minehead_optimize_inputs();
    let task = run_inputs
        .as_ref()
        .map(|inputs| state.minehead_optimize.read(cx).state(inputs));
    let minehead = &state.research.minehead;
    let combos = minehead.combo_count();
    for (range, [low, high]) in minehead.ranges.iter().zip(&minehead.knob_fields) {
        show_number(low, range.lo, range.decimals(), window, cx);
        show_number(high, range.hi, range.decimals(), window, cx);
    }
    let muted = cx.theme().muted_foreground;

    // The knob ranges: one low and one high field per knob, each stepping
    // through the setter that keeps low at most high.
    let field = |id: String,
                 label: String,
                 state: &Entity<InputState>,
                 value: (f64, f64, usize),
                 set: fn(&mut super::State, usize, f64, &mut Context<AppState>),
                 index: usize| {
        let knob = entity.clone();
        div()
            .id(SharedString::from(id.clone()))
            .w(px(120.0))
            .flex_none()
            .child(
                stepper(&id, state, value, move |value, _window, cx| {
                    knob.update(cx, |state, cx| {
                        set(&mut state.research.minehead, index, value, cx)
                    });
                })
                .accessibility_label(label)
                .small(),
            )
    };
    let knobs = div().flex().flex_col().gap_1().children(
        minehead
            .ranges
            .iter()
            .zip(&minehead.knob_fields)
            .enumerate()
            .map(|(index, (range, [low, high]))| {
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
                    .child(field(
                        format!("mh-lo-{}", range.key),
                        format!("{} from", range.label),
                        low,
                        (range.lo, range.step, range.decimals()),
                        super::State::set_knob_low,
                        index,
                    ))
                    .child(Caption::new("to").text_color(muted))
                    .child(field(
                        format!("mh-hi-{}", range.key),
                        format!("{} to", range.label),
                        high,
                        (range.hi, range.step, range.decimals()),
                        super::State::set_knob_high,
                        index,
                    ))
            }),
    );

    let reset = entity.clone();
    let run = entity.clone();
    let cancel = entity.clone();
    let controls = Card::new()
        .header(
            CardHeader::new("Search ranges")
                .description(format!(
                    "{combos} combos · 200 screening, 2,000 final trials"
                ))
                .action(Button::new("mh-opt-reset").label("Reset").small().on_click(
                    move |_event, _window, cx| {
                        reset.update(cx, |state, cx| {
                            state.research.minehead.ranges = super::default_ranges();
                            cx.notify();
                        });
                    },
                )),
        )
        .child(
            div().h(px(430.0)).child(
                div()
                    .overflow_y_scrollbar()
                    .id("mh-knobs-scroll")
                    .size_full()
                    .child(knobs),
            ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("mh-opt-run")
                        .label(if running {
                            "Optimizing…"
                        } else {
                            "Optimize Strategy"
                        })
                        .primary()
                        .icon(IconName::Play)
                        .disabled(running)
                        .on_click(move |_event, _window, cx| {
                            run.update(cx, |state, cx| state.run_minehead_optimize(cx));
                        }),
                )
                .when(running, |row| {
                    row.child(Button::new("mh-opt-cancel").label("Cancel").on_click(
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
    if let Some(result) = task.as_ref().and_then(|task| task.result.as_ref()) {
        body = body.child(results(result, cx));
    }
    if let Some(inferred) = minehead.inferred.as_ref() {
        body = body.child(inferred_card(
            inferred,
            minehead.use_inferred,
            minehead.games,
            minehead.decisions.len(),
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

/// The strategy table's columns.
const COLUMNS: [Column; 10] = [
    ("#", Some(36.0), true),
    ("Win%", None, true),
    ("Avg Dmg", None, true),
    ("T1 Dmg", None, true),
    ("Turns", None, true),
    ("Dmg/Cmt", None, true),
    ("EV", None, true),
    ("Min", None, true),
    ("Cap", None, true),
    ("HP", None, true),
];

fn results(result: &Optimized, cx: &mut Context<AppState>) -> AnyElement {
    let muted = cx.theme().muted_foreground;
    let best = &result.best;
    let cell = |index| columns::cell(&COLUMNS, index).min_w(px(56.0));
    let body =
        TableBody::new().children(result.top.iter().enumerate().map(|(index, candidate)| {
            let params = &candidate.params;
            let result = &candidate.result;
            TableRow::new()
                .child(cell(0).child(format!("{}", index + 1)))
                .child(
                    cell(1)
                        .text_color(win_tone(result.win_rate).color(cx))
                        .child(format!("{:.1}%", result.win_rate * 100.0)),
                )
                .child(cell(2).child(format::value(result.avg_dmg)))
                .child(cell(3).child(format::value(result.avg_first_turn_dmg)))
                .child(cell(4).child(format!("{:.1}", result.avg_turns)))
                .child(cell(5).child(format::value(result.avg_dmg_per_commit)))
                .child(cell(6).child(format!("{:.1}", params.ev_multiplier)))
                .child(cell(7).child(format!("{:.0}", params.min_reveal)))
                .child(cell(8).child(format!("{:.0}%", params.mine_cap_pct * 100.0)))
                .child(cell(9).child(format!("{:.0}%", params.hp_threshold * 100.0)))
        }));
    let table = div().h(px(330.0)).child(
        div()
            .overflow_y_scrollbar()
            .id("mh-opt-table")
            .size_full()
            .child(
                Table::new()
                    .small()
                    .child(columns::header(&COLUMNS))
                    .child(body),
            ),
    );
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
                    Tag::new()
                        .with_variant(win_tone(best.result.win_rate).into())
                        .outline()
                        .small()
                        .child(format!("{:.1}% win", best.result.win_rate * 100.0)),
                ),
        )
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
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
        .child(table)
        .into_any_element()
}

fn inferred_card(
    inferred: &idleon_minehead::Inferred,
    checked: bool,
    games: u32,
    decisions: usize,
    cx: &mut Context<AppState>,
) -> AnyElement {
    let Some(params) = inferred.params else {
        return div().into_any_element();
    };
    let muted = cx.theme().muted_foreground;
    let entity = cx.entity();
    let use_entity = entity.clone();
    let reset = entity.clone();
    let toggle = Checkbox::new("mh-use-inferred")
        .label("Use this strategy for Rank and Path")
        .checked(checked)
        .on_click(move |on, _window, cx| {
            let on = *on;
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
                .action(
                    Tag::new()
                        .with_variant(Tone::Info.into())
                        .outline()
                        .small()
                        .child(inferred.profile.clone()),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
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
                    div().flex().child(
                        Button::new("mh-reset-inferred")
                            .label("Reset play data")
                            .small()
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

fn summary(result: &MonteCarlo) -> String {
    format!(
        "{} avg dmg · {} turns · {} dmg/commit",
        format::value(result.avg_dmg),
        format::value(result.avg_turns),
        format::value(result.avg_dmg_per_commit)
    )
}

/// How a win rate reads: good above 80%, fair above 40%.
fn win_tone(win_rate: f64) -> Tone {
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
