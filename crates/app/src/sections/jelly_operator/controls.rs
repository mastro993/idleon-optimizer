//! The section's kit form controls and the table a plan fills, built once.
//!
//! The persisted values stay on [`State`](super::State) and its planner,
//! optimizer and simulator (`docs/section-state.md`). These entities only show
//! them: edits flow control → value through the subscriptions made here, and
//! values flow back in render through `show`/`show_number`
//! (`docs/ui-kit/forms.md`). The ranked section sequences can run to hundreds
//! of rows, so they live in a virtualized table whose rows are rebuilt when a
//! plan is stored, never in render (`docs/ui-kit/data.md`).

use super::optimizer::{Optimizer, RunInputs as OptimizerRunInputs};
use super::planner::RunInputs as PlannerRunInputs;
use super::simulator::RunInputs as SimulatorRunInputs;
use super::{RunInputs as EstimateRunInputs, planner as plan_view};
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::select::SelectEvent;
use gpui_kit::component::table::TableState;
use idleon_jelly::board;
use idleon_jelly::operation::RevivePolicy;
use idleon_jelly::optimizer::Optimization;
use idleon_jelly::planner::{self, Objective, Plan, Quality, SectionRow, Strategy};
use idleon_jelly::{Estimate, Insights, Jelly, Trial, boss_hp, boss_time, data};
use idleon_ui::format;
use idleon_ui::runner::{Ending, TaskEvent, TaskRunner};
use idleon_ui::widgets::{
    Choice, ChoiceSelect, RowColumn, RowsDelegate, number_state, number_value,
};
use std::rc::Rc;

/// The optimizer's count fields: element id, label, range and step.
pub(crate) const OPTIMIZER_COUNTS: [(&str, &str, f64, f64); 6] = [
    ("jelly-optimizer-starts", "Starts", 1.0, 32.0),
    ("jelly-optimizer-beam", "Beam", 4.0, 64.0),
    ("jelly-optimizer-iterations", "Build rounds", 1.0, 120.0),
    (
        "jelly-optimizer-saved-rounds",
        "Saved-board rounds",
        0.0,
        30.0,
    ),
    (
        "jelly-optimizer-refinement-rounds",
        "Refinement rounds",
        0.0,
        30.0,
    ),
    ("jelly-optimizer-finalists", "Finalists", 1.0, 16.0),
];

/// The optimizer count the field at `index` in [`OPTIMIZER_COUNTS`] edits.
pub(crate) fn optimizer_count(optimizer: &mut Optimizer, index: usize) -> &mut usize {
    match index {
        0 => &mut optimizer.starts,
        1 => &mut optimizer.beam_width,
        2 => &mut optimizer.iterations,
        3 => &mut optimizer.saved_refinement_rounds,
        4 => &mut optimizer.refinement_rounds,
        _ => &mut optimizer.finalists,
    }
}

/// One ranked section sequence, formatted once per stored plan.
pub(crate) struct SequenceRow {
    rank: SharedString,
    sequence: SharedString,
    slots: SharedString,
    proximity: SharedString,
    gain: SharedString,
    gain_per_slot: SharedString,
    beats: SharedString,
    footprint: SharedString,
}

impl SequenceRow {
    fn new(rank: usize, row: &SectionRow) -> Self {
        Self {
            rank: (rank + 1).to_string().into(),
            sequence: plan_view::sequence_label(&row.plot_ids).into(),
            slots: row.new_usable_slots.to_string().into(),
            proximity: row.proximity_slots.to_string().into(),
            gain: plan_view::signed(row.objective_gain).into(),
            gain_per_slot: plan_view::signed(row.gain_per_slot).into(),
            beats: row
                .chance_beats_baseline
                .map(format::chance)
                .unwrap_or_else(|| "—".to_string())
                .into(),
            footprint: plan_view::footprint_summary(row).into(),
        }
    }
}

pub(crate) type SequenceTable = Entity<TableState<RowsDelegate<SequenceRow>>>;

/// The section's four runners, whose stored results the tabs keep.
pub(crate) struct Runs<'a> {
    pub(crate) estimate: &'a Entity<TaskRunner<Estimate, EstimateRunInputs>>,
    pub(crate) trial: &'a Entity<TaskRunner<Trial, SimulatorRunInputs>>,
    pub(crate) plan: &'a Entity<TaskRunner<Plan, PlannerRunInputs>>,
    pub(crate) optimization: &'a Entity<TaskRunner<Optimization, OptimizerRunInputs>>,
}

impl Runs<'_> {
    /// When a run stores a result, keep it once for render and derive what the
    /// tabs read from it: the outlook, the plan's unlocked slots, the ranked
    /// sequences.
    fn keep_results(self, cx: &mut Context<AppState>) -> Vec<Subscription> {
        vec![
            on_stored(cx, self.estimate, |state, estimate, _| {
                if let Some(jelly) = &state.jelly {
                    let layout = &state.jelly_state.layout;
                    state.jelly_state.results.insights =
                        Some(Rc::new(Insights::of(jelly, layout, &estimate)));
                }
                state.jelly_state.results.estimate = Some(estimate);
            }),
            on_stored(cx, self.trial, |state, trial, _| {
                state.jelly_state.results.trial = Some(trial);
            }),
            on_stored(cx, self.plan, |state, plan, cx| {
                if let Some(jelly) = &state.jelly {
                    state.jelly_state.results.plan_unlocked = plan
                        .recommended
                        .as_ref()
                        .map(|row| {
                            board::unlocked_slots(&planner::save_with_plots(jelly, &row.plot_ids))
                        })
                        .unwrap_or_default();
                }
                let rows = plan
                    .sequences
                    .iter()
                    .enumerate()
                    .map(|(rank, row)| SequenceRow::new(rank, row))
                    .collect();
                state.jelly_state.results.plan = Some(plan);
                state.jelly_controls.sequences.update(cx, |table, cx| {
                    table.delegate_mut().set_rows(rows);
                    cx.notify();
                });
            }),
            on_stored(cx, self.optimization, |state, optimization, _| {
                state.jelly_state.results.optimization = Some(optimization);
            }),
        ]
    }
}

/// Hand `keep` the result a runner just stored, copied once.
fn on_stored<R: Clone + 'static, I: 'static>(
    cx: &mut Context<AppState>,
    runner: &Entity<TaskRunner<R, I>>,
    keep: impl Fn(&mut AppState, Rc<R>, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(runner, move |state, runner, event: &TaskEvent, cx| {
        if matches!(event, TaskEvent::Finished(Ending::Stored))
            && let Some(result) = runner.read(cx).result().cloned()
        {
            keep(state, Rc::new(result), cx);
        }
    })
}

/// Every kit control the tabs draw, and the sequences table.
pub(crate) struct Controls {
    pub(crate) trials: Entity<InputState>,
    pub(crate) seed: Entity<InputState>,
    pub(crate) sim_stage: ChoiceSelect<usize>,
    /// The stage list never changes, so it is built once.
    pub(crate) stage_choices: Vec<Choice<usize>>,
    pub(crate) sim_fever: ChoiceSelect<u32>,
    pub(crate) sim_roid: Entity<InputState>,
    pub(crate) sim_revive: ChoiceSelect<RevivePolicy>,
    pub(crate) sim_seed: Entity<InputState>,
    pub(crate) plan_strategy: ChoiceSelect<Strategy>,
    pub(crate) plan_objective: ChoiceSelect<Objective>,
    pub(crate) plan_exp: ChoiceSelect<Option<usize>>,
    pub(crate) plan_tokens: Entity<InputState>,
    pub(crate) plan_quality: ChoiceSelect<Quality>,
    pub(crate) plan_beam: Entity<InputState>,
    pub(crate) plan_seed: Entity<InputState>,
    pub(crate) sequences: SequenceTable,
    pub(crate) opt_objective: ChoiceSelect<Objective>,
    pub(crate) opt_exp: ChoiceSelect<Option<usize>>,
    pub(crate) opt_counts: [Entity<InputState>; 6],
    pub(crate) opt_trials: Entity<InputState>,
    pub(crate) opt_seed: Entity<InputState>,
    _subscriptions: Vec<Subscription>,
}

impl Controls {
    pub(crate) fn new(runs: Runs<'_>, window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let seed_max = f64::from(u32::MAX);
        let stage_choices = stage_choices();
        let controls = Self {
            trials: number_state(1.0, 1024.0, 1.0, window, cx),
            seed: number_state(0.0, seed_max, 1.0, window, cx),
            sim_stage: ChoiceSelect::new(stage_choices.clone(), Some(&0), window, cx),
            stage_choices,
            sim_fever: ChoiceSelect::new(Vec::new(), None, window, cx),
            sim_roid: number_state(-1.0, longest_normal_phase(), 1.0, window, cx),
            sim_revive: ChoiceSelect::new(super::simulator::revive_choices(), None, window, cx),
            sim_seed: number_state(0.0, seed_max, 1.0, window, cx),
            plan_strategy: ChoiceSelect::new(strategy_choices(), None, window, cx),
            plan_objective: ChoiceSelect::new(Vec::new(), None, window, cx),
            plan_exp: ChoiceSelect::new(Vec::new(), None, window, cx),
            plan_tokens: number_state(1.0, 40.0, 1.0, window, cx),
            plan_quality: ChoiceSelect::new(quality_choices(), None, window, cx),
            plan_beam: number_state(2.0, 24.0, 1.0, window, cx),
            plan_seed: number_state(0.0, seed_max, 1.0, window, cx),
            sequences: cx.new(|cx| TableState::new(sequence_table(), window, cx)),
            opt_objective: ChoiceSelect::new(Vec::new(), None, window, cx),
            opt_exp: ChoiceSelect::new(Vec::new(), None, window, cx),
            opt_counts: OPTIMIZER_COUNTS
                .map(|(_, _, min, max)| number_state(min, max, 1.0, window, cx)),
            opt_trials: number_state(4.0, 1024.0, 4.0, window, cx),
            opt_seed: number_state(0.0, seed_max, 1.0, window, cx),
            _subscriptions: Vec::new(),
        };
        let mut subscriptions = controls.subscribe(cx);
        subscriptions.extend(runs.keep_results(cx));
        Self {
            _subscriptions: subscriptions,
            ..controls
        }
    }

    /// Edits flow control → section value, through the section's setters.
    fn subscribe(&self, cx: &mut Context<AppState>) -> Vec<Subscription> {
        let mut subscriptions = vec![
            on_number(cx, &self.trials, |state, value, cx| {
                state.set_jelly_trials(value, cx);
            }),
            on_number(cx, &self.seed, |state, value, cx| {
                state.set_jelly_seed(value, cx);
            }),
            on_pick(cx, &self.sim_stage, |state, stage, cx| {
                state.set_jelly_sim_stage(stage, cx);
            }),
            on_pick(cx, &self.sim_fever, |state, fever, cx| {
                state.set_jelly_sim_fever(fever, cx);
            }),
            on_number(cx, &self.sim_roid, |state, seconds, cx| {
                state.set_jelly_sim_roid(seconds, cx);
            }),
            on_pick(cx, &self.sim_revive, |state, policy, cx| {
                state.set_jelly_sim_revive(policy, cx);
            }),
            on_number(cx, &self.sim_seed, |state, seed, cx| {
                state.set_jelly_sim_seed(seed, cx);
            }),
            on_pick(cx, &self.plan_strategy, |state, strategy, cx| {
                state.set_jelly_plan_strategy(strategy, cx);
            }),
            on_pick(cx, &self.plan_objective, |state, objective, cx| {
                state.set_jelly_plan_objective(objective, cx);
            }),
            on_pick(cx, &self.plan_exp, |state, cell_type, cx| {
                state.set_jelly_plan_exp_cell_type(cell_type, cx);
            }),
            on_number(cx, &self.plan_tokens, |state, value, cx| {
                state.set_jelly_plan_tokens(value.clamp(1.0, 40.0).round() as usize, cx);
            }),
            on_pick(cx, &self.plan_quality, |state, quality, cx| {
                state.set_jelly_plan_quality(quality, cx);
            }),
            on_number(cx, &self.plan_beam, |state, value, cx| {
                state.set_jelly_plan_beam(value.clamp(2.0, 24.0).round() as usize, cx);
            }),
            on_number(cx, &self.plan_seed, |state, value, cx| {
                state.set_jelly_plan_seed(seed(value), cx);
            }),
            on_pick(cx, &self.opt_objective, |state, objective, cx| {
                state.edit_jelly_optimizer(cx, |optimizer| optimizer.objective = objective);
            }),
            on_pick(cx, &self.opt_exp, |state, cell_type, cx| {
                state.edit_jelly_optimizer(cx, |optimizer| optimizer.exp_cell_type = cell_type);
            }),
            on_number(cx, &self.opt_trials, |state, value, cx| {
                let trials = value.clamp(4.0, 1024.0).round() as u32;
                state.edit_jelly_optimizer(cx, |optimizer| optimizer.trials = trials);
            }),
            on_number(cx, &self.opt_seed, |state, value, cx| {
                state.edit_jelly_optimizer(cx, |optimizer| optimizer.seed = seed(value));
            }),
        ];
        for (index, field) in self.opt_counts.iter().enumerate() {
            let (_, _, min, max) = OPTIMIZER_COUNTS[index];
            subscriptions.push(on_number(cx, field, move |state, value, cx| {
                let value = value.clamp(min, max).round() as usize;
                state.edit_jelly_optimizer(cx, |optimizer| {
                    *optimizer_count(optimizer, index) = value;
                });
            }));
        }
        subscriptions
    }
}

/// A typed number reaches the section once it parses.
fn on_number(
    cx: &mut Context<AppState>,
    field: &Entity<InputState>,
    apply: impl Fn(&mut AppState, f64, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(field, move |state, field, event: &InputEvent, cx| {
        if matches!(event, InputEvent::Change)
            && let Some(value) = number_value(field.read(cx))
        {
            apply(state, value, cx);
        }
    })
}

/// A confirmed pick reaches the section; a cleared one keeps the value.
fn on_pick<V: Clone + PartialEq + 'static>(
    cx: &mut Context<AppState>,
    select: &ChoiceSelect<V>,
    apply: impl Fn(&mut AppState, V, &mut Context<AppState>) + 'static,
) -> Subscription {
    cx.subscribe(
        select.state(),
        move |state, _, event: &SelectEvent<Vec<Choice<V>>>, cx| {
            let SelectEvent::Confirm(value) = event;
            if let Some(value) = value.clone() {
                apply(state, value, cx);
            }
        },
    )
}

/// A seed as the runner reads it: clamped to `u32`, 1 when not a number.
fn seed(value: f64) -> u32 {
    if value.is_finite() {
        value.clamp(0.0, f64::from(u32::MAX)) as u32
    } else {
        1
    }
}

/// Every obstruction, with its HP and normal phase.
fn stage_choices() -> Vec<Choice<usize>> {
    (0..=super::simulator::MAX_STAGE)
        .map(|index| {
            let name = data()
                .reward(index)
                .map(|reward| reward.name.clone())
                .unwrap_or_else(|| format!("Obstruction {}", index + 1));
            Choice::new(
                index,
                format!(
                    "{}. {name} · {} HP · {:.0}s normal phase",
                    index + 1,
                    format::value(boss_hp(index)),
                    boss_time(index)
                ),
            )
        })
        .collect()
}

/// The longest normal phase any stage has, the Stronkroid field's static bound;
/// the setter clamps to the picked stage's own.
fn longest_normal_phase() -> f64 {
    (0..=super::simulator::MAX_STAGE)
        .map(boss_time)
        .fold(0.0, f64::max)
}

/// The objectives the save can see.
pub(crate) fn objective_choices(jelly: &Jelly) -> Vec<Choice<Objective>> {
    [
        Objective::Clear,
        Objective::Bloodcells,
        Objective::Exp,
        Objective::Dps,
        Objective::Survival,
    ]
    .into_iter()
    .filter(|objective| objective.visible(jelly))
    .map(|objective| Choice::new(objective, objective.label()))
    .collect()
}

/// Total EXP, then each cell type the save owns.
pub(crate) fn exp_choices(jelly: &Jelly) -> Vec<Choice<Option<usize>>> {
    std::iter::once(Choice::new(None, "Total EXP"))
        .chain(
            (0..jelly.units_owned())
                .map(|cell_type| Choice::new(Some(cell_type), data().cell_name(cell_type))),
        )
        .collect()
}

pub(crate) fn strategy_choices() -> Vec<Choice<Strategy>> {
    Strategy::PAGE
        .into_iter()
        .map(|strategy| Choice::new(strategy, strategy.label()))
        .collect()
}

pub(crate) fn quality_choices() -> Vec<Choice<Quality>> {
    [Quality::Fast, Quality::Balanced, Quality::Thorough]
        .into_iter()
        .map(|quality| Choice::new(quality, quality.label()))
        .collect()
}

fn sequence_table() -> RowsDelegate<SequenceRow> {
    RowsDelegate::new(vec![
        RowColumn::number("rank", "#", |row: &SequenceRow| row.rank.clone())
            .column(|column| column.width(gpui::px(48.))),
        RowColumn::text("sequence", "Section sequence", |row: &SequenceRow| {
            row.sequence.clone()
        })
        .column(|column| column.width(gpui::px(220.))),
        RowColumn::number("slots", "Slots", |row: &SequenceRow| row.slots.clone()),
        RowColumn::number("proximity", "Proximity", |row: &SequenceRow| {
            row.proximity.clone()
        }),
        RowColumn::number("gain", "Gain", |row: &SequenceRow| row.gain.clone()),
        RowColumn::number("gain-per-slot", "Gain / slot", |row: &SequenceRow| {
            row.gain_per_slot.clone()
        }),
        RowColumn::number("beats", "Beats baseline", |row: &SequenceRow| {
            row.beats.clone()
        })
        .column(|column| column.width(gpui::px(120.))),
        RowColumn::text("footprint", "New footprint options", |row: &SequenceRow| {
            row.footprint.clone()
        })
        .column(|column| column.width(gpui::px(320.))),
    ])
}
