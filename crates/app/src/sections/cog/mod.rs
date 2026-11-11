//! The Cog Board Optimizer section: the save board, the objective controls,
//! the optimized placement and its swap list.
//!
//! Ported from `cog-optimizer.html` and `js/cogs/cog-optimizer.js`. CG-01
//! covers the three single-objective runs (build speed, construction EXP and
//! Flaggy rate), the shared background runner and reset; weighted objectives,
//! the perfect board, insights, shelf browsing and the step-by-step walkthrough
//! are the later CG-02 to CG-04 features.

mod board;

use crate::state::AppState;
use crate::ui::format;
use ely_gpui_component::buttons::{Button, ButtonVariant, SegmentedControl};
use ely_gpui_component::data_display::{Badge, KpiCard, Statistic, Tone};
use ely_gpui_component::feedback::EmptyState;
use ely_gpui_component::layout::{Card, CardHeader, ScrollArea};
use ely_gpui_component::primitives::IconName;
use ely_gpui_component::theme::{ControlSize, TextSize};
use ely_gpui_component::typography::Caption;
use gpui::prelude::*;
use gpui::{AnyElement, Context, IntoElement, ParentElement, Styled, Window, div, px};
use idleon_core::cog::{
    BOARD_SIZE, Board, Cog, CogResult, CogSave, Goal, SwapStep, compute_totals, empty_board,
};

/// The inputs a stored result was produced from, so a later edit marks it stale.
#[derive(Clone, Debug, PartialEq)]
pub struct RunInputs {
    pub board: Board,
    pub shelf: Vec<Cog>,
    pub goal: Goal,
}

/// The section's own state: the save board, the board on screen and the goal.
#[derive(Clone, Debug)]
pub struct State {
    pub save: CogSave,
    pub board: Board,
    pub goal: Goal,
}

impl Default for State {
    fn default() -> Self {
        Self {
            save: CogSave::default(),
            board: empty_board(),
            goal: Goal::ConExp,
        }
    }
}

impl State {
    /// Take a newly imported save: reset the board to it and keep the goal.
    pub fn load(&mut self, save: CogSave) {
        self.board = save.board.clone();
        self.save = save;
    }

    /// Restore the save board, dropping any optimized placement.
    pub fn reset(&mut self) {
        self.board = self.save.board.clone();
    }

    /// The inputs a run started now would use.
    pub fn inputs(&self) -> RunInputs {
        RunInputs {
            board: self.board.clone(),
            shelf: self.save.shelf.clone(),
            goal: self.goal,
        }
    }
}

/// The section view: controls, summaries, the board and the swap list.
pub fn view(state: &mut AppState, _window: &mut Window, cx: &mut Context<AppState>) -> AnyElement {
    if !state.cog.save.has_board() {
        return empty_state(cx);
    }
    let task = state.cog_task(cx);
    let board = task
        .result
        .as_ref()
        .map_or(&state.cog.board, |result| &result.board);
    let now = compute_totals(board);
    let before = compute_totals(&state.cog.save.board);
    let filled = board.iter().flatten().count();
    let board_changed = board != &state.cog.save.board;
    let has_result = task.result.is_some() || (task.cancelled && board_changed);

    let mut body = div()
        .id("cog-body")
        .flex()
        .flex_col()
        .gap_3()
        .child(summary(state, &now, &before, filled))
        .child(controls(state, cx, has_result, task.running))
        .child(board::card(state, board, cx));
    if task.running
        && let Some(progress) = task.progress.clone()
    {
        let entity = cx.entity();
        body = body.child(crate::ui::task::progress_card(
            "cog",
            "Optimizing the cog board",
            "Greedy construction, simulated annealing and the greedy finish.",
            progress,
            move |_window, cx| {
                entity.update(cx, |state, cx| state.cancel_cog(cx));
            },
        ));
    }
    if task.cancelled {
        body = body.child(crate::ui::task::cancelled_banner(
            "cog-cancelled",
            board_changed,
        ));
    }
    if let Some(result) = task.result.clone() {
        body = body.child(result_card(&result, &before, task.stale));
        body = body.child(swaps(&result.swaps));
    }

    ScrollArea::new("cog-scroll")
        .vertical()
        .flex_1()
        .min_h_0()
        .child(body)
        .into_any_element()
}

/// No save has been imported yet.
fn empty_state(cx: &mut Context<AppState>) -> AnyElement {
    let entity = cx.entity();
    Card::new()
        .header(
            CardHeader::new("Cog board").description(
                "Placement optimizer for build speed, construction EXP and Flaggy rate",
            ),
        )
        .child(
            EmptyState::new(
                "cog-empty",
                IconName::Map,
                "Import a save to read the cog board",
            )
            .body(
                "The section reads the board, the shelf and the construction level from the \
                 decoded save. Without one there is nothing to place.",
            )
            .action(
                Button::new("cog-import", "Import a save")
                    .icon(IconName::Upload)
                    .on_click(move |_event, window, cx| {
                        entity.update(cx, |state, cx| state.open_import(window, cx));
                    }),
            ),
        )
        .into_any_element()
}

/// The three saved totals, the construction level and the occupied slots.
fn summary(
    state: &AppState,
    now: &idleon_core::cog::BoardTotals,
    before: &idleon_core::cog::BoardTotals,
    filled: usize,
) -> AnyElement {
    let level = state.cog.save.construction_level;
    div()
        .id("cog-summary")
        .role(gpui::Role::Label)
        .aria_label(format!(
            "Build {} · Construction EXP {} · Flaggy {} · level {level} · {filled} of {BOARD_SIZE} slots",
            format::fmt_val(now.total_build),
            format::fmt_val(now.player_con_exp_hr),
            format::fmt_val(now.total_flaggy)
        ))
        .flex()
        .flex_wrap()
        .gap_3()
        .child(kpi(
            state.cog.goal,
            "cog-kpi-build",
            "Build Speed",
            now.total_build,
            before.total_build,
            "Base build × board surrounds",
        ))
        .child(kpi(
            state.cog.goal,
            "cog-kpi-conexp",
            "Construction EXP/hr",
            now.player_con_exp_hr,
            before.player_con_exp_hr,
            "Best player × surround × totalD",
        ))
        .child(kpi(
            state.cog.goal,
            "cog-kpi-flaggy",
            "Flaggy Rate",
            now.total_flaggy,
            before.total_flaggy,
            "Base Flaggy × board surrounds",
        ))
        .child(kpi(
            state.cog.goal,
            "cog-kpi-level",
            "Construction Level",
            f64::from(level),
            0.0,
            "Highest saved Lv0 construction",
        ))
        .child(kpi(
            state.cog.goal,
            "cog-kpi-cogs",
            "Board Cogs",
            filled as f64,
            0.0,
            "Filled of 96 slots",
        ))
        .into_any_element()
}

/// One summary card: label, scaled figure and its change from the save.
fn kpi(
    goal: Goal,
    id: &'static str,
    label: &'static str,
    value: f64,
    before: f64,
    caption: &'static str,
) -> AnyElement {
    let (scaled, suffix) = format::scale(value);
    let mut statistic = Statistic::new(id, label, scaled)
        .decimals(2)
        .size(TextSize::Xl);
    if !suffix.is_empty() {
        statistic = statistic.suffix(suffix);
    }
    let change = if before > 0.0 && value != before {
        format!("{:+.2}% · ", (value - before) / before * 100.0)
    } else {
        String::new()
    };
    let goal_note = if goal == Goal::ConExp && id == "cog-kpi-conexp" {
        " · objective"
    } else {
        ""
    };
    KpiCard::new(statistic)
        .caption(format!("{change}{caption}{goal_note}"))
        .into_any_element()
}

/// Objective selection, Optimize, Reset and the saved construction level.
fn controls(
    state: &AppState,
    cx: &mut Context<AppState>,
    has_result: bool,
    running: bool,
) -> AnyElement {
    let entity = cx.entity();
    let goal_entity = entity.clone();
    let goal = SegmentedControl::new("cog-goal", state.cog.goal.id())
        .size(ControlSize::Sm)
        .segment(Goal::Build.id(), Goal::Build.label(), None)
        .segment(Goal::ConExp.id(), Goal::ConExp.label(), None)
        .segment(Goal::Flaggy.id(), Goal::Flaggy.label(), None)
        .on_change(move |value, _window, cx| {
            if let Some(goal) = Goal::from_id(value.as_ref()) {
                goal_entity.update(cx, |state, cx| state.set_cog_goal(goal, cx));
            }
        });
    let optimize_entity = entity.clone();
    let optimize = Button::new("cog-optimize", "Optimize")
        .variant(ButtonVariant::Primary)
        .size(ControlSize::Sm)
        .icon(IconName::Play)
        .disabled(running || !state.cog.save.has_board())
        .on_click(move |_event, _window, cx| {
            optimize_entity.update(cx, |state, cx| state.optimize_cog(cx));
        });
    let reset_entity = entity;
    let reset = Button::new("cog-reset", "Reset")
        .variant(ButtonVariant::Subtle)
        .size(ControlSize::Sm)
        .icon(IconName::RotateCcw)
        .disabled(!has_result && state.cog.board == state.cog.save.board)
        .on_click(move |_event, _window, cx| {
            reset_entity.update(cx, |state, cx| state.reset_cog(cx));
        });
    div()
        .id("cog-controls")
        .flex()
        .flex_wrap()
        .items_center()
        .gap_3()
        .child(goal)
        .child(optimize)
        .child(reset)
        .child(Caption::new(format!(
            "Construction level {} · {} shelf cogs",
            state.cog.save.construction_level,
            state.cog.save.shelf.len()
        )))
        .into_any_element()
}

/// The result banner: every category before and after, plus the swap count.
fn result_card(
    result: &CogResult,
    before: &idleon_core::cog::BoardTotals,
    stale: bool,
) -> AnyElement {
    let after = compute_totals(&result.board);
    let summary = format!(
        "Optimized board: {} swaps, {:+.2}% on the objective",
        result.swaps.len(),
        result.improvement_pct
    );
    let row = div()
        .id("cog-result")
        .role(gpui::Role::Label)
        .aria_label(summary.clone())
        .flex()
        .flex_wrap()
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
        .child(delta("Board score", result.initial_score, result.score));
    Card::new()
        .header(
            CardHeader::new("Result")
                .description(format!(
                    "{} swaps · {:+.2}% improvement",
                    result.swaps.len(),
                    result.improvement_pct
                ))
                .action(if stale {
                    crate::ui::task::stale_badge("cog").into_any_element()
                } else {
                    div().into_any_element()
                }),
        )
        .child(div().p_3().flex().flex_col().gap_2().child(row))
        .when(stale, |card| {
            card.child(crate::ui::task::stale_banner("cog"))
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
        .gap_0p5()
        .child(Caption::new(label))
        .child(
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_size(px(14.0)).child(format::fmt_val(after)))
                .child(
                    Badge::new(if change == 0.0 {
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
                    })
                    .tone(tone),
                ),
        )
        .child(Caption::new(format!("was {}", format::fmt_val(before))))
        .into_any_element()
}

/// The swap list from the save board to the optimized one.
fn swaps(steps: &[SwapStep]) -> AnyElement {
    if steps.is_empty() {
        return Card::new()
            .header(CardHeader::new("Swap list"))
            .child(div().id("cog-swaps-empty").p_3().child(Caption::new(
                "No swaps needed: the save board is already the best placement found.",
            )))
            .into_any_element();
    }
    let table = steps.iter().enumerate().fold(
        ely_gpui_component::tables::Table::new(["#", "From", "To", "At", "Source"]),
        |table, (index, step)| {
            table.row([
                div().child((index + 1).to_string()).into_any_element(),
                div()
                    .child(idleon_core::cog::display_name(&step.from))
                    .into_any_element(),
                div()
                    .child(idleon_core::cog::display_name(&step.to))
                    .into_any_element(),
                div()
                    .child(format!("({},{})", step.col, step.row))
                    .into_any_element(),
                div().child(step.source_label()).into_any_element(),
            ])
        },
    );
    Card::new()
        .header(CardHeader::new("Swap list").description(
            "Each row names the cog leaving the slot, the cog arriving and where it comes from.",
        ))
        .child(
            ScrollArea::new("cog-swaps")
                .vertical()
                .h(px(260.0))
                .child(table),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_save() -> CogSave {
        let mut board = empty_board();
        board[14] = Some(Cog::named("Player_Ana", 14));
        board[13] = Some(Cog::named("Cog3A00", 13));
        CogSave {
            board,
            shelf: vec![Cog::named("CogCry2", 108)],
            shelf_slots: Vec::new(),
            construction_level: 42,
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
        state.reset();
        assert_eq!(state.board, state.save.board);
        assert_eq!(state.goal, Goal::Flaggy);
    }

    #[test]
    fn the_run_inputs_follow_the_board_shelf_and_goal() {
        let mut state = State::default();
        state.load(sample_save());
        let inputs = state.inputs();
        assert_eq!(inputs.board, state.board);
        assert_eq!(inputs.shelf.len(), 1);
        assert_eq!(inputs.goal, Goal::ConExp);
    }
}
