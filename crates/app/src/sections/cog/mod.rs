//! The Cog Board Optimizer section: the save board, the objective controls,
//! the optimized placement, its insight, roll ranges and the read-only shelf.
//!
//! CG-01 covers the three single-objective runs (build speed, construction EXP
//! and Flaggy rate), the shared background runner and reset; CG-02 adds the
//! custom weights and the perfect board; CG-03 adds the board insights, the
//! roll ranges and the shelf; CG-04 adds the step-by-step swap walkthrough:
//! Next applies the run's swap list in order, Previous restores the prior board
//! and Done keeps the optimized board.

mod actions;

mod board;
mod controls;
mod insights;
mod table;
mod walkthrough;

pub(crate) use controls::Controls;

use crate::state::AppState;
use crate::ui::segmented::{Segmented, segmented};
use controls::WEIGHTS;
use gpui::prelude::*;
use gpui::{
    AnyElement, App, Context, Entity, IntoElement, ParentElement, Role, SharedString, Styled,
    Window, div, px,
};
use gpui_kit::assets::IconName;
use gpui_kit::component::alert::Alert;
use gpui_kit::component::button::{Button, ButtonVariants as _};
use gpui_kit::component::checkbox::Checkbox;
use gpui_kit::component::empty::{
    Empty, EmptyContent, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle,
};
use gpui_kit::component::scroll::ScrollableElement as _;
use gpui_kit::component::table::DataTable;
use gpui_kit::component::tag::Tag;
use gpui_kit::component::{ActiveTheme as _, Disableable as _, Icon, Sizable as _};
use idleon_cog::{
    BOARD_SIZE, Board, BoardTotals, Cog, CogResult, CogSave, Goal, Objective, PerfectBoard,
    PerfectStats, SwapWalkthrough, WeightError, WeightInputs, empty_board, perfect_stats,
    player_slot, slot_to_pos,
};
use idleon_ui::format;
use idleon_ui::widgets::{
    Caption, Card, CardHeader, KpiStrip, Statistic, TEXT_BODY, Tone, show_number,
};
use table::{Col, table, text};

/// The inputs a stored optimizer result was produced from, so a later edit
/// marks it stale.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RunInputs {
    pub(crate) board: Board,
    pub(crate) shelf: Vec<Cog>,
    pub(crate) objective: Objective,
}

/// The inputs a stored perfect board was produced from. The perfect board reads
/// the player and the Yin/Yang inventory from the save board, not from the
/// placement currently on screen.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PerfectRunInputs {
    pub(crate) board: Board,
    pub(crate) shelf: Vec<Cog>,
    pub(crate) construction_level: u32,
    pub(crate) goal: Goal,
}

/// The section's own state: the save board, the board on screen, the objective,
/// the swap walkthrough while one is open and the perfect-board preview.
#[derive(Clone, Debug)]
pub(crate) struct State {
    pub(crate) save: CogSave,
    pub(crate) board: Board,
    pub(crate) goal: Goal,
    pub(crate) walkthrough: Option<SwapWalkthrough>,
    /// Whether the custom weights replace the single goal for Optimize.
    pub(crate) weighted: bool,
    /// The weight fields as entered, kept even while they are invalid.
    pub(crate) weights: WeightInputs,
    /// Whether the board grid shows the perfect board instead of the board on
    /// screen. The optimizer's result is shown by its walkthrough instead.
    pub(crate) show_perfect: bool,
}

impl Default for State {
    fn default() -> Self {
        Self {
            save: CogSave::default(),
            board: empty_board(),
            goal: Goal::ConExp,
            walkthrough: None,
            weighted: false,
            weights: WeightInputs::default(),
            show_perfect: false,
        }
    }
}

impl State {
    /// Take a newly imported save: reset the board to it and keep the controls.
    pub(crate) fn load(&mut self, save: CogSave) {
        self.board = save.board.clone();
        self.save = save;
        self.walkthrough = None;
        self.show_perfect = false;
    }

    /// Restore the save board, dropping any optimized or perfect placement.
    pub(crate) fn reset(&mut self) {
        self.board = self.save.board.clone();
        self.walkthrough = None;
        self.show_perfect = false;
    }

    /// The validated weights, or the reason they cannot run.
    pub(crate) fn weight_problem(&self) -> Option<WeightError> {
        self.weighted
            .then(|| self.weights.weights().err())
            .flatten()
    }

    /// The objective the next Optimize run uses.
    ///
    /// Invalid weights fall back to the selected single goal so a stored result
    /// can still be compared; the controls refuse to start a run until the
    /// weights are valid.
    pub(crate) fn objective(&self) -> Objective {
        if self.weighted
            && let Ok(weights) = self.weights.weights()
        {
            return Objective::weighted(weights, &self.save.board);
        }
        Objective::goal(self.goal)
    }

    /// The inputs a run started now would use.
    pub(crate) fn inputs(&self) -> RunInputs {
        RunInputs {
            board: self.board.clone(),
            shelf: self.save.shelf.clone(),
            objective: self.objective(),
        }
    }

    /// The inputs the perfect board would use.
    ///
    /// The perfect board reads the active stat overlay, not the custom
    /// weights, so it always maximises the selected single goal. Its stats use
    /// the effective construction level the save's own rolls reveal, as the
    /// roll ranges do.
    pub(crate) fn perfect_inputs(&self) -> PerfectRunInputs {
        PerfectRunInputs {
            board: self.save.board.clone(),
            shelf: self.save.shelf.clone(),
            construction_level: self.save.max_construction_level,
            goal: self.goal,
        }
    }

    /// Start replaying a finished run from the board the run started on. A run
    /// with no swaps has nothing to walk through, so its board is kept at once.
    pub(crate) fn begin_walkthrough(&mut self, result: &CogResult) {
        self.show_perfect = false;
        if result.swaps.is_empty() {
            self.board = result.board.clone();
            self.walkthrough = None;
            return;
        }
        let start = self.board.clone();
        self.walkthrough = Some(SwapWalkthrough::new(
            &start,
            &result.board,
            &self.save.shelf_slots,
            &result.swaps,
        ));
    }

    /// Apply the next swap step.
    pub(crate) fn next_swap(&mut self) -> bool {
        self.walkthrough
            .as_mut()
            .is_some_and(SwapWalkthrough::next_step)
    }

    /// Restore the board before the last applied swap step.
    pub(crate) fn previous_swap(&mut self) -> bool {
        self.walkthrough
            .as_mut()
            .is_some_and(SwapWalkthrough::previous)
    }

    /// Keep the optimized board and close the walkthrough.
    pub(crate) fn finish_walkthrough(&mut self) -> bool {
        let Some(mut walkthrough) = self.walkthrough.take() else {
            return false;
        };
        self.board = walkthrough.finish();
        true
    }

    /// Drop a walkthrough without keeping its result, as a new run or reset does.
    pub(crate) fn dismiss_walkthrough(&mut self) {
        self.walkthrough = None;
    }
}

/// The section view: controls, summaries, the board and the swap list.
pub(crate) fn view(
    state: &mut AppState,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> AnyElement {
    if !state.cog.save.has_board() {
        return empty_state(cx);
    }
    let task = state.cog_task(cx);
    let perfect = state.cog_perfect_task(cx);
    let perfect_board = if state.cog.show_perfect {
        perfect.result.as_ref().map(|perfect| &perfect.board)
    } else {
        None
    };
    let preview = state.cog.walkthrough.as_ref().map(SwapWalkthrough::board);
    let board: &Board = perfect_board.or(preview).unwrap_or(&state.cog.board);
    let after = match perfect_board {
        Some(board) => Some(board),
        None => task.result.as_ref().map(|result| &result.board),
    };
    let swaps = task.result.as_ref().map(|result| result.swaps.as_slice());
    state
        .cog_controls
        .sync(state.save_revision, &state.cog, board, swaps, after, cx);
    for goal in WEIGHTS {
        let field = state.cog_controls.weight(goal);
        show_number(field, state.cog.weights.get(goal), 0, window, cx);
    }

    let state: &AppState = state;
    let entity = cx.entity();
    let cx: &App = cx;
    let figures = state.cog_controls.figures();
    let board_changed = *board != state.cog.save.board;
    let has_result = task.result.is_some()
        || perfect.result.is_some()
        || state.cog.walkthrough.is_some()
        || (task.cancelled && board_changed);
    let running = task.running || perfect.running;

    let mut body = div()
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(state, figures))
        .child(controls(state, &entity, has_result, running))
        .child(board::card(state, board, figures.insights.as_ref(), cx))
        .child(insights::insights_card(
            figures.insights.as_ref(),
            state.cog.goal,
        ))
        .child(insights::roll_ranges_card(
            &state.cog_controls.ranges,
            state.cog.save.max_construction_level,
            state.cog.save.construction_level,
            cx,
        ))
        .child(insights::shelf_card(state));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let entity = entity.clone();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "cog",
            "Optimizing the cog board",
            "Greedy construction, simulated annealing and the greedy finish.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_cog(cx));
            },
        ));
    }
    if perfect.running
        && let Some(progress) = perfect.progress.clone()
    {
        let entity = entity.clone();
        body = body.child(idleon_ui::run_chrome::progress_card(
            "cog-perfect",
            "Building the perfect cog board",
            "Perfect cogs, directionals, crystals, Excogia and Yang, one player slot at a time.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_cog_perfect(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "cog-cancelled",
            board_changed,
        ));
    }
    if perfect.cancelled {
        body = body.child(idleon_ui::run_chrome::cancelled_banner(
            "cog-perfect-cancelled",
            board_changed,
        ));
    }
    let before = &figures.before;
    let after = &state.cog_controls.after;
    if state.cog.show_perfect {
        if let Some(result) = perfect.result.as_ref() {
            body = body.child(perfect_card(result, before, after, perfect.stale, cx));
        }
    } else if let Some(result) = task.result.as_ref() {
        body = body.child(result_card(result, before, after, task.stale));
        body = body.child(match state.cog.walkthrough.as_ref() {
            Some(walkthrough) => walkthrough::card(state, walkthrough, &entity),
            None => swaps_card(state, result.swaps.is_empty()),
        });
    }

    div()
        .id("cog-body")
        .flex()
        .flex_col()
        .flex_1()
        .min_h_0()
        .child(body)
        .overflow_y_scrollbar()
        .id("cog-scroll")
        .into_any_element()
}

/// `value` with `decimals` places and its whole part grouped in thousands,
/// as in `30,808.96`.
pub(super) fn grouped(value: f64, decimals: usize) -> String {
    let text = format!("{value:.decimals$}");
    match text.split_once('.') {
        Some((whole, fraction)) => {
            format!("{}.{fraction}", format::exact(whole.parse().unwrap_or(0.0)))
        }
        None => format::exact(value),
    }
}

/// A small outlined tag.
pub(super) fn tag(label: impl Into<SharedString>, tone: Tone) -> Tag {
    Tag::new()
        .with_variant(tone.into())
        .outline()
        .small()
        .child(label.into())
}

/// No save has been imported yet.
fn empty_state(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    let muted = cx.theme().muted_foreground;
    Card::new()
        .header(
            CardHeader::new("Cog board").description(
                "Placement optimizer for build speed, construction EXP and Flaggy rate",
            ),
        )
        .child(
            div().id("cog-empty").child(
                Empty::new()
                    .header(
                        EmptyHeader::new()
                            .media(
                                EmptyMedia::new().child(Icon::new(IconName::Map).text_color(muted)),
                            )
                            .title(EmptyTitle::new().child("Import a save to read the cog board"))
                            .description(EmptyDescription::new().child(
                                "The section reads the board, the shelf and the construction \
                                 level from the decoded save. Without one there is nothing to place.",
                            )),
                    )
                    .content(
                        EmptyContent::new().child(
                            Button::new("cog-import")
                                .label("Import a save")
                                .icon(IconName::Upload)
                                .on_click(move |_event, window, cx| {
                                    entity.update(cx, |state, cx| state.open_import(window, cx));
                                }),
                        ),
                    ),
            ),
        )
        .into_any_element()
}

/// The three saved totals, the construction level and the occupied slots.
fn summary(state: &AppState, figures: &controls::Figures) -> AnyElement {
    let (now, before, filled) = (&figures.now, &figures.before, figures.filled);
    let level = state.cog.save.construction_level;
    let objective = if state.cog.weighted {
        None
    } else {
        Some(state.cog.goal)
    };
    div()
        .id("cog-summary")
        .role(Role::Label)
        .aria_label(format!(
            "Build {} · Construction EXP {} · Flaggy {} · level {level} · {filled} of {BOARD_SIZE} slots",
            format::fmt_val(now.total_build),
            format::fmt_val(now.player_con_exp_hr),
            format::fmt_val(now.total_flaggy)
        ))
        .child(
            KpiStrip::new()
                .child(kpi(
                    objective,
                    "cog-kpi-build",
                    "Build Speed",
                    now.total_build,
                    before.total_build,
                    "Base build × board surrounds",
                ))
                .child(kpi(
                    objective,
                    "cog-kpi-conexp",
                    "Construction EXP/hr",
                    now.player_con_exp_hr,
                    before.player_con_exp_hr,
                    "Best player × surround × totalD",
                ))
                .child(kpi(
                    objective,
                    "cog-kpi-flaggy",
                    "Flaggy Rate",
                    now.total_flaggy,
                    before.total_flaggy,
                    "Base Flaggy × board surrounds",
                ))
                .child(kpi(
                    None,
                    "cog-kpi-level",
                    "Construction Level",
                    f64::from(level),
                    0.0,
                    "Highest saved Lv0 construction",
                ))
                .child(kpi(
                    None,
                    "cog-kpi-cogs",
                    "Board Cogs",
                    filled as f64,
                    0.0,
                    "Filled of 96 slots",
                )),
        )
        .into_any_element()
}

/// One summary figure: label, scaled figure and its change from the save.
fn kpi(
    objective: Option<Goal>,
    id: &'static str,
    label: &'static str,
    value: f64,
    before: f64,
    caption: &'static str,
) -> Statistic {
    let (scaled, suffix) = format::scale(value);
    let mut statistic = Statistic::new(id, label, grouped(scaled, 2));
    if !suffix.is_empty() {
        statistic = statistic.suffix(suffix);
    }
    let change = if before > 0.0 && value != before {
        format!("{:+.2}% · ", (value - before) / before * 100.0)
    } else {
        String::new()
    };
    let goal_note = if objective == Some(Goal::ConExp) && id == "cog-kpi-conexp" {
        " · objective"
    } else {
        ""
    };
    statistic.caption(format!("{change}{caption}{goal_note}"))
}

/// The goal switch: one segment per goal. Segment `n` is
/// `cog-goal.segment-<n>`.
fn goal_switch(current: Goal, entity: &Entity<AppState>) -> Segmented {
    const GOALS: [Goal; 3] = [Goal::Build, Goal::ConExp, Goal::Flaggy];
    let entity = entity.clone();
    segmented(
        "cog-goal",
        GOALS.iter().enumerate().map(|(index, goal)| {
            (
                *goal,
                SharedString::from(format!("cog-goal.segment-{index}")),
                goal.label(),
            )
        }),
        current,
        move |goal, _window, cx| entity.update(cx, |state, cx| state.set_cog_goal(goal, cx)),
    )
}

/// The objective controls: single goal or custom weights, Optimize, Perfect
/// Board and Reset.
fn controls(
    state: &AppState,
    entity: &Entity<AppState>,
    has_result: bool,
    running: bool,
) -> AnyElement {
    let problem = state.cog.weight_problem();
    let weighted = state.cog.weighted;
    let weighted_entity = entity.clone();
    let custom = Checkbox::new("cog-weighted")
        .label("Custom weights")
        .checked(weighted)
        .on_click(move |on, _window, cx| {
            let on = *on;
            weighted_entity.update(cx, |state, cx| state.set_cog_weighted(on, cx));
        });
    let optimize_entity = entity.clone();
    let optimize = Button::new("cog-optimize")
        .label("Optimize")
        .primary()
        .small()
        .icon(IconName::Play)
        .disabled(running || problem.is_some() || !state.cog.save.has_board())
        .on_click(move |_event, _window, cx| {
            optimize_entity.update(cx, |state, cx| state.optimize_cog(cx));
        });
    let perfect_entity = entity.clone();
    let can_perfect = player_slot(&state.cog.save.board).is_some();
    let perfect = Button::new("cog-perfect")
        .label("Perfect Board")
        .small()
        .icon(IconName::Sparkles)
        .disabled(running || !can_perfect)
        .on_click(move |_event, _window, cx| {
            perfect_entity.update(cx, |state, cx| state.perfect_cog(cx));
        });
    let reset_entity = entity.clone();
    let reset = Button::new("cog-reset")
        .label("Reset")
        .secondary()
        .small()
        .icon(IconName::RotateCcw)
        .disabled(!has_result && state.cog.board == state.cog.save.board)
        .on_click(move |_event, _window, cx| {
            reset_entity.update(cx, |state, cx| state.reset_cog(cx));
        });

    let weights = state.cog.weights;
    let weight_field = |id: &'static str, label: &'static str, goal: Goal| {
        div()
            .flex()
            .items_center()
            .gap_2()
            .child(Caption::new(label))
            .child(crate::ui::number_field(
                id,
                state.cog_controls.weight(goal),
                label,
            ))
    };
    let weight_row = div()
        .id("cog-weights")
        .flex()
        .items_center()
        .gap_3()
        .child(weight_field(
            "cog-weight-build",
            "Build weight",
            Goal::Build,
        ))
        .child(weight_field(
            "cog-weight-conexp",
            "Construction EXP weight",
            Goal::ConExp,
        ))
        .child(weight_field(
            "cog-weight-flaggy",
            "Flaggy weight",
            Goal::Flaggy,
        ));
    let normalised = weights.weights().ok().map(|weights| {
        format!(
            "Normalised: Build {:.0}% · Construction EXP {:.0}% · Flaggy {:.0}%",
            weights.get(Goal::Build) * 100.0,
            weights.get(Goal::ConExp) * 100.0,
            weights.get(Goal::Flaggy) * 100.0
        )
    });
    let mode_note = if weighted {
        format!(
            "Custom weights drive Optimize; the board cards and Perfect Board use {}.",
            state.cog.goal.label()
        )
    } else {
        format!(
            "Construction level {} · {} shelf cogs",
            state.cog.save.construction_level,
            state.cog.save.shelf.len()
        )
    };

    div()
        .id("cog-controls")
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .child(goal_switch(state.cog.goal, entity))
                .child(custom)
                .child(optimize)
                .child(perfect)
                .child(reset),
        )
        .when(weighted, |controls| controls.child(weight_row))
        .when_some(problem, |controls, problem| {
            // The wrapper carries the message as its label, which the harness reads.
            controls.child(
                div()
                    .id("cog-weight-error")
                    .role(Role::Label)
                    .aria_label(problem.message())
                    .child(
                        Alert::new("cog-weight-alert", problem.message())
                            .with_variant(Tone::Danger.into())
                            .banner(),
                    ),
            )
        })
        .when_some(normalised, |controls, text| {
            controls.child(
                div()
                    .id("cog-weight-normalised")
                    .role(Role::Label)
                    .aria_label(text.clone())
                    .child(Caption::new(text)),
            )
        })
        .child(Caption::new(mode_note))
        .into_any_element()
}

/// The objective as the result card names it.
fn objective_label(objective: &Objective) -> String {
    match objective {
        Objective::Goal(goal) => goal.label().to_string(),
        Objective::Weighted { weights, .. } => format!(
            "Custom weights · Build {:.0}% · Construction EXP {:.0}% · Flaggy {:.0}%",
            weights.get(Goal::Build) * 100.0,
            weights.get(Goal::ConExp) * 100.0,
            weights.get(Goal::Flaggy) * 100.0
        ),
    }
}

/// The stale badge, or nothing.
fn stale_action(stale: bool, id: &'static str) -> AnyElement {
    if stale {
        idleon_ui::run_chrome::stale_badge(id).into_any_element()
    } else {
        div().into_any_element()
    }
}

/// The result banner: every category before and after, plus the swap count.
fn result_card(
    result: &CogResult,
    before: &BoardTotals,
    after: &BoardTotals,
    stale: bool,
) -> AnyElement {
    let objective = objective_label(&result.objective);
    let weighted = matches!(result.objective, Objective::Weighted { .. });
    let summary = format!(
        "Optimized board: {} swaps, {:+.2}% on the objective · {objective}",
        result.swaps.len(),
        result.improvement_pct
    );
    let row = div()
        .id("cog-result")
        .role(Role::Label)
        .aria_label(summary.clone())
        .flex()
        .gap_4()
        .child(delta("Build Speed", before.total_build, after.total_build))
        .child(delta(
            "Construction EXP/hr",
            before.player_con_exp_hr,
            after.player_con_exp_hr,
        ))
        .child(delta(
            "Flaggy Rate",
            before.total_flaggy,
            after.total_flaggy,
        ))
        .child(delta(
            if weighted {
                "Objective score"
            } else {
                "Board score"
            },
            result.initial_score,
            result.score,
        ));
    Card::new()
        .header(
            CardHeader::new("Result")
                .description(format!(
                    "{} swaps · {:+.2}% improvement",
                    result.swaps.len(),
                    result.improvement_pct
                ))
                .action(stale_action(stale, "cog")),
        )
        .child(
            div()
                .id("cog-objective")
                .role(Role::Label)
                .aria_label(format!("Objective: {objective}"))
                .child(Caption::new(format!("Objective: {objective}"))),
        )
        .child(row)
        .when(stale, |card| {
            card.child(idleon_ui::run_chrome::stale_banner("cog"))
        })
        .into_any_element()
}

/// The perfect board's explanation: the layer table and the perfect stats.
fn perfect_card(
    perfect: &PerfectBoard,
    before: &BoardTotals,
    after: &BoardTotals,
    stale: bool,
    cx: &App,
) -> AnyElement {
    let (col, row) = slot_to_pos(perfect.player_slot);
    let summary = format!(
        "Perfect {} board: player at ({col},{row}), {} Excogia blocks, {} Yang cogs, score {}",
        perfect.goal.label(),
        perfect.excogia_blocks,
        perfect.yang_placed,
        format::fmt_val(perfect.score)
    );
    let layers = table(
        "cog-perfect-table",
        &[
            Col::Text("Layer"),
            Col::Text("What it placed"),
            Col::Number("Slots"),
        ],
        perfect.layers.iter().map(|layer| {
            vec![
                div()
                    .id(format!("cog-perfect-layer-{}", layer.id))
                    .child(layer.label)
                    .into_any_element(),
                text(layer.detail.clone()),
                text(layer.slots.to_string()),
            ]
        }),
        cx,
    );
    let t3 = perfect_stats(3, perfect.construction_level);
    let t4 = perfect_stats(4, perfect.construction_level);
    let base = |stats: &PerfectStats| match perfect.goal {
        Goal::Build => stats.a,
        Goal::ConExp => stats.d,
        Goal::Flaggy => stats.c,
    };
    let stats_line = format!(
        "Perfect T3 {} {} · T4 {} {} · max surround +{} / +{} at construction level {}",
        perfect.goal.label(),
        format::fmt_val(base(&t3)),
        perfect.goal.label(),
        format::fmt_val(base(&t4)),
        format::fmt_val(t3.surround),
        format::fmt_val(t4.surround),
        perfect.construction_level
    );
    Card::new()
        .header(
            CardHeader::new("Perfect board")
                .description(
                    "Theoretical placement from perfect cogs; the layers are applied in order and only kept when they raise the score.",
                )
                .action(stale_action(stale, "cog-perfect")),
        )
        .child(
            div()
                .id("cog-perfect-result")
                .role(Role::Label)
                .aria_label(summary.clone())
                .flex()
                .gap_4()
                .child(delta("Build Speed", before.total_build, after.total_build))
                .child(delta(
                    "Construction EXP/hr",
                    before.player_con_exp_hr,
                    after.player_con_exp_hr,
                ))
                .child(delta(
                    "Flaggy Rate",
                    before.total_flaggy,
                    after.total_flaggy,
                )),
        )
        .child(Caption::new(summary))
        .child(
            div()
                .h(px(240.0))
                .child(layers)
                .overflow_y_scrollbar()
                .id("cog-perfect-layers"),
        )
        .child(Caption::new(stats_line))
        .child(Caption::new(
            "The tiny-cog board is not part of this section yet; this perfect board covers the 12×8 board.",
        ))
        .when(stale, |card| {
            card.child(idleon_ui::run_chrome::stale_banner("cog-perfect"))
        })
        .into_any_element()
}

fn delta(label: &'static str, before: f64, after: f64) -> AnyElement {
    let change = after - before;
    let tone = if change > 0.0 {
        Tone::Success
    } else {
        Tone::Neutral
    };
    div()
        .flex()
        .flex_col()
        .items_start()
        .gap_0p5()
        .child(Caption::new(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_size(TEXT_BODY).child(format::fmt_val(after)))
                .child(tag(
                    if change == 0.0 {
                        "no change".to_string()
                    } else {
                        format!(
                            "{:+.2}%",
                            if before > 0.0 {
                                change / before * 100.0
                            } else {
                                0.0
                            }
                        )
                    },
                    tone,
                )),
        )
        .child(Caption::new(format!("was {}", format::fmt_val(before))))
        .into_any_element()
}

/// The swap list from the save board to the optimized one.
fn swaps_card(state: &AppState, empty: bool) -> AnyElement {
    if empty {
        return Card::new()
            .header(CardHeader::new("Swap list"))
            .child(div().id("cog-swaps-empty").child(Caption::new(
                "No swaps needed: the save board is already the best placement found.",
            )))
            .into_any_element();
    }
    Card::new()
        .header(CardHeader::new("Swap list").description(
            "Each row names the cog leaving the slot, the cog arriving and where it comes from.",
        ))
        .child(
            div()
                .id("cog-swaps")
                .h(px(260.0))
                .child(DataTable::new(&state.cog_controls.swaps)),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_cog::{SwapStep, compute_totals};

    fn sample_save() -> CogSave {
        let mut board = empty_board();
        let mut player = Cog::named("Player_Ana", 14);
        player.b = 100.0;
        player.d = 20.0;
        board[14] = Some(player);
        let mut filler = Cog::named("Cog3A00", 13);
        filler.a = 50.0;
        board[13] = Some(filler);
        CogSave {
            board,
            shelf: vec![Cog::named("CogCry2", 108)],
            shelf_slots: Vec::new(),
            construction_level: 42,
            max_construction_level: 42,
            loaded: true,
        }
    }

    #[test]
    fn reset_restores_the_save_board_and_keeps_the_goal() {
        let mut state = State {
            goal: Goal::Flaggy,
            ..State::default()
        };
        state.load(sample_save());
        assert_eq!(state.board, state.save.board);
        state.board[0] = Some(Cog::named("Cog3le", 0));
        state.show_perfect = true;
        state.reset();
        assert_eq!(state.board, state.save.board);
        assert_eq!(state.goal, Goal::Flaggy);
        assert!(!state.show_perfect);
    }

    #[test]
    fn the_shelf_is_read_only_across_load_reset_and_a_run_snapshot() {
        let mut state = State::default();
        state.load(sample_save());
        let saved = state.save.shelf.clone();
        // The board changes, the save board comes back, and a run clones the
        // shelf: the shelf itself is never edited.
        state.board[0] = Some(Cog::named("Cog3le", 0));
        state.reset();
        let inputs = state.inputs();
        assert_eq!(state.save.shelf, saved);
        assert_eq!(inputs.shelf, saved);
    }

    #[test]
    fn the_run_inputs_follow_the_board_shelf_and_goal() {
        let mut state = State::default();
        state.load(sample_save());
        let inputs = state.inputs();
        assert_eq!(inputs.board, state.board);
        assert_eq!(inputs.shelf.len(), 1);
        assert_eq!(inputs.objective, Objective::goal(Goal::ConExp));
    }

    #[test]
    fn the_default_objective_is_construction_exp() {
        let state = State::default();
        assert_eq!(state.objective(), Objective::goal(Goal::ConExp));
        assert!(state.weight_problem().is_none());
    }

    #[test]
    fn custom_weights_normalise_against_the_save_board() {
        let mut state = State {
            weighted: true,
            weights: WeightInputs {
                build: 60.0,
                conexp: 20.0,
                flaggy: 20.0,
            },
            ..State::default()
        };
        state.load(sample_save());
        let Objective::Weighted { weights, reference } = state.objective() else {
            panic!("custom weights are a weighted objective");
        };
        assert!((weights.get(Goal::Build) - 0.6).abs() < 1e-12);
        assert!((weights.get(Goal::ConExp) - 0.2).abs() < 1e-12);
        let totals = compute_totals(&state.save.board);
        assert_eq!(
            reference,
            [
                totals.total_build,
                totals.player_con_exp_hr,
                totals.total_flaggy,
            ]
        );
    }

    #[test]
    fn invalid_weights_are_reported_and_fall_back_to_the_goal() {
        let mut state = State {
            goal: Goal::Flaggy,
            weighted: true,
            weights: WeightInputs {
                build: 0.0,
                conexp: 0.0,
                flaggy: 0.0,
            },
            ..State::default()
        };
        state.load(sample_save());
        assert_eq!(state.weight_problem(), Some(WeightError::Empty));
        assert_eq!(state.objective(), Objective::goal(Goal::Flaggy));
    }

    #[test]
    fn the_perfect_inputs_read_the_save_board_and_the_selected_goal() {
        let mut state = State {
            goal: Goal::Build,
            weighted: true,
            weights: WeightInputs {
                build: 10.0,
                conexp: 80.0,
                flaggy: 10.0,
            },
            ..State::default()
        };
        state.load(sample_save());
        let inputs = state.perfect_inputs();
        assert_eq!(inputs.board, state.save.board);
        assert_eq!(inputs.shelf, state.save.shelf);
        assert_eq!(inputs.construction_level, 42);
        assert_eq!(
            inputs.goal,
            Goal::Build,
            "the perfect board uses the selected goal, not the weights"
        );
    }

    fn swap_result() -> CogResult {
        let mut target = empty_board();
        let mut player = Cog::named("Player_Ana", 14);
        player.b = 100.0;
        player.d = 20.0;
        let mut filler = Cog::named("Cog3A00", 13);
        filler.a = 50.0;
        target[13] = Some(player);
        target[14] = Some(filler);
        CogResult {
            board: target,
            objective: Objective::goal(Goal::ConExp),
            score: 2.0,
            initial_score: 1.0,
            improvement: 1.0,
            improvement_pct: 100.0,
            swaps: vec![SwapStep {
                slot: 14,
                from: "Player_Ana".into(),
                to: "Cog3A00".into(),
                src_slot: Some(13),
                col: 2,
                row: 1,
            }],
        }
    }

    #[test]
    fn the_walkthrough_steps_in_order_and_done_keeps_the_final_board() {
        let mut state = State::default();
        state.load(sample_save());
        let save_board = state.save.board.clone();
        let result = swap_result();
        state.begin_walkthrough(&result);
        assert_eq!(
            state.walkthrough.as_ref().map(SwapWalkthrough::applied),
            Some(0)
        );
        assert_eq!(
            state.walkthrough.as_ref().map(SwapWalkthrough::board),
            Some(&save_board)
        );

        assert!(state.next_swap());
        assert_eq!(
            state.walkthrough.as_ref().map(SwapWalkthrough::board),
            Some(&result.board)
        );
        assert_eq!(state.board, save_board, "the board is only kept after Done");
        assert!(state.previous_swap());
        assert_eq!(
            state.walkthrough.as_ref().map(SwapWalkthrough::board),
            Some(&save_board)
        );
        assert!(!state.previous_swap(), "there is no step before the first");

        assert!(state.next_swap());
        assert!(state.finish_walkthrough());
        assert_eq!(state.board, result.board);
        assert!(state.walkthrough.is_none());
        assert!(
            !state.finish_walkthrough(),
            "a closed walkthrough has nothing to finish"
        );
    }

    #[test]
    fn a_run_without_swaps_keeps_its_board_without_a_walkthrough() {
        let mut state = State::default();
        state.load(sample_save());
        let result = CogResult {
            swaps: Vec::new(),
            ..swap_result()
        };
        state.begin_walkthrough(&result);
        assert!(state.walkthrough.is_none());
        assert_eq!(state.board, result.board);
    }
}
