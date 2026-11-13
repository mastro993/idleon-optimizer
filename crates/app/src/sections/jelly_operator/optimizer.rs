//! JO-05: the Optimizer tab, the coordinator over the full board.
//!
//! Ports the upstream `#tab-optimizer` panel of `jelly-operator-calc.html`: the
//! objective, EXP target, search breadth, beam, rounds, finalists, trials and
//! seed, the run through the shared task runner, and the ranked finalists with
//! the recommended board and its critical target timeline. The search itself
//! lives in `idleon_jelly::optimizer`; this module owns the controls, the
//! results and the button that puts the recommendation on the layout.

use super::grid;
use super::outlook::timeline_card;
use crate::sections::jelly_operator::estimate::text_kpi;
use crate::state::AppState;
use ely_gpui_component::buttons::{Button, ButtonVariant};
use ely_gpui_component::data_display::{KpiCard, Statistic};
use ely_gpui_component::feedback::Banner;
use ely_gpui_component::forms::{Choice, NumberInput, Select};
use ely_gpui_component::layout::{Card, CardHeader};
use ely_gpui_component::primitives::{IconName, Severity};
use ely_gpui_component::tables::Table;
use ely_gpui_component::theme::{ActiveTheme as _, ControlSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use idleon_jelly::board::{self, Layout};
use idleon_jelly::optimizer::{self, Optimization};
use idleon_jelly::planner::Objective;
use idleon_jelly::{Jelly, data};
use idleon_ui::format;
use idleon_ui::run_chrome;

/// The page's optimizer controls (`optimizerControls`), with the cheaper
/// defaults this single-process search runs.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub(crate) struct Optimizer {
    pub(crate) objective: Objective,
    pub(crate) exp_cell_type: Option<usize>,
    pub(crate) starts: usize,
    pub(crate) beam_width: usize,
    pub(crate) iterations: usize,
    pub(crate) saved_refinement_rounds: usize,
    pub(crate) refinement_rounds: usize,
    pub(crate) finalists: usize,
    pub(crate) trials: u32,
    pub(crate) seed: u32,
}

impl Default for Optimizer {
    fn default() -> Self {
        let defaults = optimizer::Options::default();
        Self {
            objective: defaults.objective,
            exp_cell_type: defaults.exp_cell_type,
            starts: defaults.starts,
            beam_width: defaults.beam_width,
            iterations: defaults.iterations,
            saved_refinement_rounds: defaults.saved_refinement_rounds,
            refinement_rounds: defaults.refinement_rounds,
            finalists: defaults.finalists,
            trials: defaults.trials,
            seed: defaults.seed,
        }
    }
}

impl Optimizer {
    /// Take the save's visible objective and a cell type its board can own.
    pub(crate) fn adopt(&mut self, jelly: &Jelly) {
        if !self.objective.visible(jelly) {
            self.objective = Objective::Clear;
        }
        self.exp_cell_type = self
            .exp_cell_type
            .map(|cell_type| cell_type.min(jelly.units_owned().saturating_sub(1)));
    }

    /// The request a run started now would use.
    pub(crate) fn inputs(&self, jelly: Option<&Jelly>, layout: &Layout) -> RunInputs {
        RunInputs {
            jelly: jelly.cloned(),
            layout: layout.clone(),
            optimizer: self.clone(),
        }
    }
}

/// The inputs one optimizer run reads, so an edit marks its result stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) jelly: Option<Jelly>,
    pub(crate) layout: Layout,
    pub(crate) optimizer: Optimizer,
}

impl RunInputs {
    /// `optimizerControls` as the layout search reads it.
    pub(crate) fn options(&self) -> optimizer::Options {
        optimizer::Options {
            objective: self.optimizer.objective,
            exp_cell_type: self.optimizer.exp_cell_type,
            starts: self.optimizer.starts,
            beam_width: self.optimizer.beam_width,
            iterations: self.optimizer.iterations,
            saved_refinement_rounds: self.optimizer.saved_refinement_rounds,
            refinement_rounds: self.optimizer.refinement_rounds,
            finalists: self.optimizer.finalists,
            trials: self.optimizer.trials,
            seed: self.optimizer.seed,
            layout: Some(self.layout.clone()),
            ..optimizer::Options::default()
        }
    }
}

/// The Optimizer tab: the controls, the run and the recommendation.
pub(crate) fn pane(
    state: &mut AppState,
    _window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_optimize_task(cx);
    let running = task.running;
    let mut body = div()
        .id("jelly-optimizer")
        .flex()
        .flex_col()
        .gap_3()
        .child(
            Card::new()
                .header(CardHeader::new("Coordinator optimizer").description(
                    "Independent starts trade focus and elite boards, then the strongest finalists are simulated on the current board's own seeds.",
                ))
                .child(div().p_3().child(controls(state, cx, jelly, running))),
        );
    if running && let Some(progress) = task.progress.clone() {
        let entity = cx.entity();
        body = body.child(run_chrome::progress_card(
            "jelly-optimizer",
            "Optimizing the board",
            "Building seeds, keeping a diverse beam and simulating the finalists.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_jelly_optimize(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(run_chrome::cancelled_banner(
            "jelly-optimizer-cancelled",
            task.stale,
        ));
    }
    let Some(result) = task.result else {
        if !running && !task.cancelled {
            body = body.child(Banner::new(
                "jelly-optimizer-empty",
                Severity::Info,
                "Optimize the board to rank full layouts against the one it starts from and see the winner's critical target timeline.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-optimizer-stale"));
    }
    body.child(result_view(state, cx, jelly, &result))
        .into_any_element()
}

/// The objective, EXP target, search breadth and the advanced rounds.
fn controls(
    state: &AppState,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    running: bool,
) -> AnyElement {
    let entity = cx.entity();
    let controls = &state.jelly_state.optimizer;

    let objective_entity = entity.clone();
    let objectives: Vec<Choice> = [
        Objective::Clear,
        Objective::Bloodcells,
        Objective::Exp,
        Objective::Dps,
        Objective::Survival,
    ]
    .into_iter()
    .filter(|objective| objective.visible(jelly))
    .map(|objective| Choice::new(objective.id(), objective.label()))
    .collect();
    let objective = Select::new("jelly-optimizer-objective", objectives)
        .label("Objective")
        .selected(controls.objective.id())
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            if let Some(objective) = Objective::from_id(value) {
                let entity = objective_entity.clone();
                entity.update(cx, |state, cx| {
                    state.edit_jelly_optimizer(cx, |optimizer| {
                        optimizer.objective = objective;
                    })
                });
            }
        });

    let exp_entity = entity.clone();
    let mut exp_choices = vec![Choice::new("total", "Total EXP")];
    exp_choices.extend(
        (0..jelly.units_owned())
            .map(|cell_type| Choice::new(cell_type.to_string(), data().cell_name(cell_type))),
    );
    let exp_target = Select::new("jelly-optimizer-exp-cell-type", exp_choices)
        .label("EXP target")
        .selected(
            controls
                .exp_cell_type
                .map(|cell_type| cell_type.to_string())
                .unwrap_or_else(|| "total".to_string()),
        )
        .size(ControlSize::Sm)
        .on_change(move |value, _window, cx| {
            let cell_type = value.parse::<usize>().ok();
            let entity = exp_entity.clone();
            entity.update(cx, |state, cx| {
                state.edit_jelly_optimizer(cx, |optimizer| {
                    optimizer.exp_cell_type = cell_type;
                })
            });
        });

    let mut row = div()
        .id("jelly-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_end()
        .gap_3()
        .child(objective);
    if controls.objective == Objective::Exp {
        row = row.child(exp_target);
    }

    let number = |id: &'static str,
                  label: &'static str,
                  value: f64,
                  range: (f64, f64),
                  edit: fn(&mut Optimizer, usize),
                  entity: gpui::Entity<AppState>| {
        NumberInput::new(id, value)
            .label(label)
            .range(range.0, range.1)
            .step(1.0)
            .size(ControlSize::Sm)
            .on_change(move |value, _window, cx| {
                let value = value.clamp(range.0, range.1).round() as usize;
                let entity = entity.clone();
                entity.update(cx, |state, cx| {
                    state.edit_jelly_optimizer(cx, |optimizer| edit(optimizer, value))
                });
            })
    };
    row = row
        .child(number(
            "jelly-optimizer-starts",
            "Starts",
            controls.starts as f64,
            (1.0, 32.0),
            |optimizer, value| optimizer.starts = value,
            entity.clone(),
        ))
        .child(number(
            "jelly-optimizer-beam",
            "Beam",
            controls.beam_width as f64,
            (4.0, 64.0),
            |optimizer, value| optimizer.beam_width = value,
            entity.clone(),
        ))
        .child(number(
            "jelly-optimizer-iterations",
            "Build rounds",
            controls.iterations as f64,
            (1.0, 120.0),
            |optimizer, value| optimizer.iterations = value,
            entity.clone(),
        ))
        .child(number(
            "jelly-optimizer-saved-rounds",
            "Saved-board rounds",
            controls.saved_refinement_rounds as f64,
            (0.0, 30.0),
            |optimizer, value| optimizer.saved_refinement_rounds = value,
            entity.clone(),
        ))
        .child(number(
            "jelly-optimizer-refinement-rounds",
            "Refinement rounds",
            controls.refinement_rounds as f64,
            (0.0, 30.0),
            |optimizer, value| optimizer.refinement_rounds = value,
            entity.clone(),
        ))
        .child(number(
            "jelly-optimizer-finalists",
            "Finalists",
            controls.finalists as f64,
            (1.0, 16.0),
            |optimizer, value| optimizer.finalists = value,
            entity.clone(),
        ))
        .child(
            NumberInput::new("jelly-optimizer-trials", f64::from(controls.trials))
                .label("Trials")
                .range(4.0, 1024.0)
                .step(4.0)
                .size(ControlSize::Sm)
                .on_change({
                    let entity = entity.clone();
                    move |value, _window, cx| {
                        let trials = value.clamp(4.0, 1024.0).round() as u32;
                        let entity = entity.clone();
                        entity.update(cx, |state, cx| {
                            state.edit_jelly_optimizer(cx, |optimizer| {
                                optimizer.trials = trials;
                            })
                        });
                    }
                }),
        )
        .child(
            NumberInput::new("jelly-optimizer-seed", f64::from(controls.seed))
                .label("Seed")
                .range(0.0, f64::from(u32::MAX))
                .step(1.0)
                .size(ControlSize::Sm)
                .on_change({
                    let entity = entity.clone();
                    move |value, _window, cx| {
                        let seed = if value.is_finite() {
                            value.clamp(0.0, f64::from(u32::MAX)) as u32
                        } else {
                            1
                        };
                        let entity = entity.clone();
                        entity.update(cx, |state, cx| {
                            state.edit_jelly_optimizer(cx, |optimizer| optimizer.seed = seed)
                        });
                    }
                }),
        );

    let run_entity = entity;
    let run = Button::new("jelly-optimizer-run", "Optimize board")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Play)
        .disabled(running)
        .on_click(move |_event, _window, cx| {
            let run_entity = run_entity.clone();
            run_entity.update(cx, |state, cx| state.run_jelly_optimize(cx));
        });
    row.child(run).into_any_element()
}

fn result_view(
    state: &AppState,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    result: &Optimization,
) -> AnyElement {
    let recommended = &result.recommended;
    let chance = recommended
        .chance_beats_baseline
        .map(format::chance)
        .unwrap_or_else(|| "Not measured".to_string());
    let gain = if result.baseline_score.abs() > 0.0 {
        100.0 * (result.recommended.proxy_score - result.baseline_score)
            / result.baseline_score.abs()
    } else {
        0.0
    };
    let cards = div()
        .id("jelly-optimizer-summary")
        .flex()
        .flex_wrap()
        .gap_3()
        .child(text_kpi(
            "jelly-optimizer-score",
            "Board score",
            format!(
                "{} → {}",
                score_text(result.baseline_score),
                score_text(recommended.proxy_score)
            ),
            format!("{gain:+.2}% over the board the run started from"),
            cx,
        ))
        .child(text_kpi(
            "jelly-optimizer-measured",
            "Measured gain",
            signed(recommended.objective_value - result.baseline_objective_value),
            format!("{chance} chance to beat the baseline"),
            cx,
        ))
        .child(
            KpiCard::new(
                Statistic::new(
                    "jelly-optimizer-moves",
                    "Anchors changed",
                    result.moves.len() as f64,
                )
                .decimals(0),
            )
            .caption(format!(
                "{} cell(s) placed, {} removed",
                result
                    .moves
                    .iter()
                    .filter(|movement| movement.after.is_some())
                    .count(),
                result
                    .moves
                    .iter()
                    .filter(|movement| movement.before.is_some())
                    .count()
            )),
        )
        .child(text_kpi(
            "jelly-optimizer-finalists-card",
            "Finalists",
            format::count(result.finalists.len() as u64),
            format!(
                "{} start(s) · beam {} · {} trial(s) each",
                result.starts.len(),
                result.beam_width,
                recommended.operation.estimate.trials
            ),
            cx,
        ));

    let mut body = div()
        .id("jelly-optimizer-result")
        .flex()
        .flex_col()
        .gap_3()
        .child(cards)
        .child(apply_row(state, cx, result));
    if recommended.layout == result.baseline_layout {
        body = body.child(Banner::new(
            "jelly-optimizer-tie",
            Severity::Info,
            "No finalist measured better than the current board, so the recommendation keeps it.",
        ));
    }
    let unlocked = board::unlocked_slots(jelly);
    let highlight: Vec<usize> = result
        .moves
        .iter()
        .filter_map(|movement| movement.after.map(|_| movement.anchor))
        .collect();
    body = body
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap_3()
                .child(board_panel(
                    state,
                    cx,
                    "Current board",
                    grid::BoardView {
                        prefix: "jelly-optimizer-baseline",
                        layout: &result.baseline_layout,
                        unlocked: &unlocked,
                        highlight: &[],
                        obstruction: result.obstruction,
                        selected: None,
                        action: grid::BoardAction::ReadOnly,
                    },
                ))
                .child(board_panel(
                    state,
                    cx,
                    "Optimized board",
                    grid::BoardView {
                        prefix: "jelly-optimizer-final",
                        layout: &recommended.layout,
                        unlocked: &unlocked,
                        highlight: &highlight,
                        obstruction: result.obstruction,
                        selected: None,
                        action: grid::BoardAction::ReadOnly,
                    },
                )),
        )
        .child(finalists_table(result))
        .child(timeline_card(&recommended.operation.estimate, cx));
    body.into_any_element()
}

fn apply_row(state: &AppState, cx: &mut Context<AppState>, result: &Optimization) -> AnyElement {
    let message = &state.jelly_state.optimizer_message;
    let entity = cx.entity();
    let apply = Button::new("jelly-optimizer-apply", "Use this board")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Check)
        .disabled(result.recommended.layout == result.baseline_layout)
        .on_click(move |_event, _window, cx| {
            let entity = entity.clone();
            entity.update(cx, |state, cx| state.apply_jelly_optimized_layout(cx));
        });
    let mut row = div()
        .id("jelly-optimizer-apply-row")
        .flex()
        .items_center()
        .gap_3()
        .child(apply);
    if !message.is_empty() {
        row = row.child(
            div()
                .id("jelly-optimizer-message")
                .role(Role::Label)
                .aria_label(message.clone())
                .child(Caption::new(message.clone()))
                .text_color(cx.theme().colors.fg_muted),
        );
    }
    row.into_any_element()
}

fn board_panel(
    state: &AppState,
    cx: &mut Context<AppState>,
    title: &'static str,
    view: grid::BoardView<'_>,
) -> AnyElement {
    Card::new()
        .header(CardHeader::new(title))
        .child(div().p_3().child(grid::board(state, view, cx)))
        .into_any_element()
}

fn finalists_table(result: &Optimization) -> AnyElement {
    let mut table = Table::new([
        "#",
        "Source",
        "Cells",
        "Board score",
        "Measured",
        "Beats baseline",
    ]);
    for (index, row) in result.finalists.iter().enumerate() {
        table = table.row([
            (index + 1).to_string(),
            match (row.start, row.focus) {
                (Some(start), Some(focus)) => {
                    format!("Start {} · {}", start + 1, focus.label())
                }
                _ => "Saved board".to_string(),
            },
            row.layout.len().to_string(),
            score_text(row.proxy_score),
            value_of(row),
            if row.layout == result.baseline_layout {
                "—".to_string()
            } else {
                row.chance_beats_baseline
                    .map(format::chance)
                    .unwrap_or_else(|| "—".to_string())
            },
        ]);
    }
    Card::new()
        .header(CardHeader::new("Ranked finalists").description(
            "Every finalist with its shared-seed comparison against the board the run started from.",
        ))
        .child(div().id("jelly-optimizer-ranked").p_3().child(table))
        .into_any_element()
}

fn value_of(row: &optimizer::Row) -> String {
    if !row.objective_value.is_finite() {
        return "—".to_string();
    }
    objective_text(row.objective_value)
}

/// The objective value, in its own units: probabilities, damage or EXP.
fn objective_text(value: f64) -> String {
    if value != 0.0 && value.abs() < 1.0 {
        format!("{:.4}", value)
    } else {
        format::value(value)
    }
}

fn score_text(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    format::value(value)
}

fn signed(value: f64) -> String {
    if !value.is_finite() {
        return "—".to_string();
    }
    if value >= 0.0 {
        format!("+{}", format::value(value))
    } else {
        format!("-{}", format::value(-value))
    }
}
