//! JO-05: the Optimizer tab, the coordinator over the full board.
//!
//! The objective, EXP target, search breadth, beam, rounds, finalists, trials
//! and seed, the run through the shared task runner, and the ranked finalists
//! with the recommended board and its critical target timeline. The search
//! itself lives in `idleon_jelly::optimizer`; this module owns the controls,
//! the results and the button that puts the recommendation on the layout.

use super::controls::{OPTIMIZER_COUNTS, optimizer_count};
use super::grid;
use super::outlook::timeline_card;
use super::parts::{
    SELECT_WIDTH, boxed, figure, info, number_field, run_button, select, strips, text_kpi,
    text_table,
};
use super::planner::signed_value;
use crate::state::AppState;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Role, Styled, Window, div};
use gpui_kit::assets::IconName;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::{Disableable as _, Sizable as _};
use idleon_jelly::Jelly;
use idleon_jelly::board::Layout;
use idleon_jelly::optimizer::{self, Optimization};
use idleon_jelly::planner::Objective;
use idleon_ui::format;
use idleon_ui::run_chrome;
use idleon_ui::widgets::{Caption, Card, CardHeader, PANEL_GAP};

/// The optimizer controls, with the cheaper
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
    /// The controls as the layout search reads them.
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
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
) -> AnyElement {
    let task = state.jelly_optimize_task(cx);
    let running = task.running;
    let mut body = div()
        .id("jelly-optimizer")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(
            Card::new()
                .header(CardHeader::new("Coordinator optimizer").description(
                    "Independent starts trade focus and elite boards, then the strongest finalists are simulated on the current board's own seeds.",
                ))
                .child(controls(state, window, cx, jelly, running)),
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
            body = body.child(info(
                "jelly-optimizer-empty",
                "Optimize the board to rank full layouts against the one it starts from and see the winner's critical target timeline.",
            ));
        }
        return body.into_any_element();
    };
    if task.stale {
        body = body.child(run_chrome::stale_banner("jelly-optimizer-stale"));
    }
    body.child(result_view(state, cx, &result))
        .into_any_element()
}

/// The objective, EXP target, search breadth and the advanced rounds.
fn controls(
    state: &AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
    jelly: &Jelly,
    running: bool,
) -> AnyElement {
    let optimizer = &state.jelly_state.optimizer;
    let controls = &state.jelly_controls;
    controls.opt_objective.show(
        crate::sections::jelly_operator::controls::objective_choices(jelly),
        Some(&optimizer.objective),
        window,
        cx,
    );
    let mut row = div()
        .id("jelly-optimizer-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap(PANEL_GAP)
        .child(boxed(
            SELECT_WIDTH,
            select(
                &controls.opt_objective,
                "jelly-optimizer-objective",
                "Objective",
            ),
        ));
    if optimizer.objective == Objective::Exp {
        controls.opt_exp.show(
            crate::sections::jelly_operator::controls::exp_choices(jelly),
            Some(&optimizer.exp_cell_type),
            window,
            cx,
        );
        row = row.child(boxed(
            SELECT_WIDTH,
            select(
                &controls.opt_exp,
                "jelly-optimizer-exp-cell-type",
                "EXP target",
            ),
        ));
    }
    let mut counts = optimizer.clone();
    for (index, (id, label, _, _)) in OPTIMIZER_COUNTS.into_iter().enumerate() {
        let value = *optimizer_count(&mut counts, index) as f64;
        row = row.child(number_field(
            id,
            label,
            &controls.opt_counts[index],
            value,
            window,
            cx,
        ));
    }
    row.child(number_field(
        "jelly-optimizer-trials",
        "Trials",
        &controls.opt_trials,
        f64::from(optimizer.trials),
        window,
        cx,
    ))
    .child(number_field(
        "jelly-optimizer-seed",
        "Seed",
        &controls.opt_seed,
        f64::from(optimizer.seed),
        window,
        cx,
    ))
    .child(run_button(
        "jelly-optimizer-run",
        "Optimize board",
        running,
        |state, cx| state.run_jelly_optimize(cx),
        cx,
    ))
    .into_any_element()
}

fn result_view(state: &AppState, cx: &mut Context<AppState>, result: &Optimization) -> AnyElement {
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
    let cards = strips(
        "jelly-optimizer-summary",
        vec![
            text_kpi(
                "jelly-optimizer-score",
                "Board score",
                format!(
                    "{} → {}",
                    score_text(result.baseline_score),
                    score_text(recommended.proxy_score)
                ),
                format!("{gain:+.2}% over the board the run started from"),
            ),
            text_kpi(
                "jelly-optimizer-measured",
                "Measured gain",
                signed_value(recommended.objective_value - result.baseline_objective_value),
                format!("{chance} chance to beat the baseline"),
            ),
            figure(
                "jelly-optimizer-moves",
                "Anchors changed",
                result.moves.len() as f64,
                0,
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
            text_kpi(
                "jelly-optimizer-finalists-card",
                "Finalists",
                format::count(result.finalists.len() as u64),
                format!(
                    "{} start(s) · beam {} · {} trial(s) each",
                    result.starts.len(),
                    result.beam_width,
                    recommended.operation.estimate.trials
                ),
            ),
        ],
    );

    let unlocked = &state.jelly_state.save.unlocked;
    let highlight: Vec<usize> = result
        .moves
        .iter()
        .filter_map(|movement| movement.after.map(|_| movement.anchor))
        .collect();
    let baseline = grid::panel(
        state,
        "Current board",
        grid::BoardView {
            prefix: "jelly-optimizer-baseline",
            layout: &result.baseline_layout,
            unlocked,
            highlight: &[],
            obstruction: result.obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
        cx,
    );
    let optimized = grid::panel(
        state,
        "Optimized board",
        grid::BoardView {
            prefix: "jelly-optimizer-final",
            layout: &recommended.layout,
            unlocked,
            highlight: &highlight,
            obstruction: result.obstruction,
            selected: None,
            action: grid::BoardAction::ReadOnly,
        },
        cx,
    );
    div()
        .id("jelly-optimizer-result")
        .flex()
        .flex_col()
        .gap(PANEL_GAP)
        .child(cards)
        .child(apply_row(state, cx, result))
        .when(recommended.layout == result.baseline_layout, |body| {
            body.child(info(
                "jelly-optimizer-tie",
                "No finalist measured better than the current board, so the recommendation keeps it.",
            ))
        })
        .child(
            div()
                .flex()
                .flex_wrap()
                .gap(PANEL_GAP)
                .child(baseline)
                .child(optimized),
        )
        .child(finalists_table(result))
        .child(timeline_card(&recommended.operation.estimate, cx))
        .into_any_element()
}

fn apply_row(state: &AppState, cx: &mut Context<AppState>, result: &Optimization) -> AnyElement {
    let message = &state.jelly_state.optimizer_message;
    let entity = cx.entity();
    let apply = Button::new("jelly-optimizer-apply")
        .label("Use this board")
        .primary()
        .small()
        .icon(IconName::Check)
        .disabled(result.recommended.layout == result.baseline_layout)
        .on_click(move |_event, _window, cx| {
            entity.update(cx, |state, cx| state.apply_jelly_optimized_layout(cx));
        });
    div()
        .id("jelly-optimizer-apply-row")
        .flex()
        .items_center()
        .gap(PANEL_GAP)
        .child(apply)
        .when(!message.is_empty(), |row| {
            row.child(
                div()
                    .id("jelly-optimizer-message")
                    .role(Role::Label)
                    .aria_label(message.clone())
                    .child(Caption::new(message.clone())),
            )
        })
        .into_any_element()
}

/// The finalists the run kept: at most the 16 the control allows.
fn finalists_table(result: &Optimization) -> AnyElement {
    let table = text_table(
        &[
            ("#", true),
            ("Source", false),
            ("Cells", true),
            ("Board score", true),
            ("Measured", true),
            ("Beats baseline", true),
        ],
        result.finalists.iter().enumerate().map(|(index, row)| {
            vec![
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
            ]
        }),
    );
    Card::new()
        .header(CardHeader::new("Ranked finalists").description(
            "Every finalist with its shared-seed comparison against the board the run started from.",
        ))
        .child(div().id("jelly-optimizer-ranked").child(table))
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
