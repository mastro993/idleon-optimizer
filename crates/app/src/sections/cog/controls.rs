//! The cog section's kit controls, its virtualized tables and the board figures
//! it draws, built once and refreshed only when what they read changes.
//!
//! The persisted goal, weights and board stay on [`State`](super::State)
//! (`docs/section-state.md`). The weight fields only show them: edits flow
//! control → value through the subscriptions made here, and values flow back
//! in render through `show_number` (`docs/ui-kit/forms.md`). The swap list,
//! the walkthrough steps and the shelf can each run past a hundred rows, so
//! they are `DataTable`s whose rows [`Controls::sync`] builds when the run,
//! the step or the save changes (`docs/ui-kit/data.md`).

use super::State;
use crate::state::AppState;
use gpui::{AppContext as _, Context, Entity, SharedString, Subscription, Window};
use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::table::TableState;
use idleon_cog::{
    Board, BoardInsights, BoardTotals, CogSave, Goal, RollRange, SwapStep, SwapWalkthrough,
    board_insights, compute_totals, display_name,
};
use idleon_ui::format;
use idleon_ui::widgets::{RowColumn, RowsDelegate, number_state};

/// What the board on screen measures to, worked out once per board.
pub(super) struct Figures {
    /// The board's totals.
    pub(super) now: BoardTotals,
    /// The save board's totals.
    pub(super) before: BoardTotals,
    /// The occupied board slots.
    pub(super) filled: usize,
    /// The insights for the selected goal, while the board scores anything.
    pub(super) insights: Option<BoardInsights>,
}

/// One row of the swap list or the walkthrough.
pub(super) struct StepRow {
    number: SharedString,
    from: SharedString,
    to: SharedString,
    at: SharedString,
    source: SharedString,
    state: SharedString,
}

impl StepRow {
    fn of(index: usize, step: &SwapStep, applied: Option<usize>) -> Self {
        Self {
            number: (index + 1).to_string().into(),
            from: display_name(&step.from).into(),
            to: display_name(&step.to).into(),
            at: format!("({},{})", step.col, step.row).into(),
            source: step.source_label().into(),
            state: applied.map_or_else(SharedString::default, |applied| {
                match index.cmp(&applied) {
                    std::cmp::Ordering::Less => "Applied",
                    std::cmp::Ordering::Equal => "Now",
                    std::cmp::Ordering::Greater => "Pending",
                }
                .into()
            }),
        }
    }
}

/// One shelf cog, as the objective reads it.
pub(super) struct ShelfRow {
    number: SharedString,
    cog: SharedString,
    base: SharedString,
    surround: SharedString,
}

type Table<T> = Entity<TableState<RowsDelegate<T>>>;

/// The weight fields, the long tables and the cached figures.
pub(crate) struct Controls {
    /// Build, Construction EXP and Flaggy, in [`WEIGHTS`] order.
    weights: [Entity<InputState>; 3],
    pub(super) swaps: Table<StepRow>,
    pub(super) steps: Table<StepRow>,
    pub(super) shelf: Table<ShelfRow>,
    figures_key: Option<(u64, Goal, Board)>,
    figures: Option<Figures>,
    shelf_key: Option<(u64, Goal)>,
    pub(super) ranges: Vec<RollRange>,
    swaps_key: Option<Vec<SwapStep>>,
    steps_key: Option<(Vec<SwapStep>, usize)>,
    after_key: Option<Board>,
    /// The totals of the stored result's or perfect board's placement.
    pub(super) after: BoardTotals,
    _subscriptions: Vec<Subscription>,
}

/// The weight fields' goals, in their on-screen order.
pub(super) const WEIGHTS: [Goal; 3] = [Goal::Build, Goal::ConExp, Goal::Flaggy];

impl Controls {
    pub(crate) fn new(window: &mut Window, cx: &mut Context<AppState>) -> Self {
        let weights = WEIGHTS.map(|_| number_state(0.0, 100.0, 1.0, window, cx));
        let subscriptions = WEIGHTS
            .into_iter()
            .zip(&weights)
            .map(|(goal, field)| {
                cx.subscribe(field, move |state, field, event: &InputEvent, cx| {
                    if matches!(event, InputEvent::Change)
                        && let Some(value) = idleon_ui::widgets::number_value(field.read(cx))
                    {
                        state.set_cog_weight(goal, value.clamp(0.0, 100.0), cx);
                    }
                })
            })
            .collect();
        Self {
            weights,
            swaps: table(step_columns(false), window, cx),
            steps: table(step_columns(true), window, cx),
            shelf: table(shelf_columns(), window, cx),
            figures_key: None,
            figures: None,
            shelf_key: None,
            ranges: Vec::new(),
            swaps_key: None,
            steps_key: None,
            after_key: None,
            after: BoardTotals::default(),
            _subscriptions: subscriptions,
        }
    }

    /// The field for `goal`'s weight.
    pub(super) fn weight(&self, goal: Goal) -> &Entity<InputState> {
        let index = WEIGHTS.iter().position(|entry| *entry == goal).unwrap_or(0);
        &self.weights[index]
    }

    /// Bring the figures and tables in line with the board on screen, the
    /// stored swaps, the open walkthrough and the save. Each piece compares
    /// the inputs it was built from, so a frame that changes none of them
    /// only compares.
    pub(super) fn sync(
        &mut self,
        revision: u64,
        cog: &State,
        board: &Board,
        swaps: Option<&[SwapStep]>,
        after: Option<&Board>,
        cx: &mut Context<AppState>,
    ) {
        if self.figures_key.as_ref().is_none_or(|(rev, goal, shown)| {
            *rev != revision || *goal != cog.goal || shown != board
        }) {
            self.figures = Some(Figures {
                now: compute_totals(board),
                before: compute_totals(&cog.save.board),
                filled: board.iter().flatten().count(),
                insights: board_insights(board, cog.goal),
            });
            self.figures_key = Some((revision, cog.goal, board.clone()));
        }
        if self.shelf_key != Some((revision, cog.goal)) {
            self.shelf_key = Some((revision, cog.goal));
            self.ranges = idleon_cog::roll_ranges(cog.goal, cog.save.max_construction_level);
            let rows = shelf_rows(&cog.save, cog.goal);
            set_rows(&self.shelf, rows, cx);
        }
        if self.swaps_key.as_deref() != swaps {
            self.swaps_key = swaps.map(<[SwapStep]>::to_vec);
            let rows = swaps.unwrap_or_default().iter().enumerate();
            let rows = rows.map(|(index, step)| StepRow::of(index, step, None));
            set_rows(&self.swaps, rows.collect(), cx);
        }
        self.sync_steps(cog.walkthrough.as_ref(), cx);
        if self.after_key.as_ref() != after {
            self.after_key = after.cloned();
            self.after = after.map(compute_totals).unwrap_or_default();
        }
    }

    fn sync_steps(&mut self, walkthrough: Option<&SwapWalkthrough>, cx: &mut Context<AppState>) {
        let Some(walkthrough) = walkthrough else {
            return;
        };
        let applied = walkthrough.applied();
        if self
            .steps_key
            .as_ref()
            .is_some_and(|(steps, at)| *at == applied && steps == walkthrough.steps())
        {
            return;
        }
        self.steps_key = Some((walkthrough.steps().to_vec(), applied));
        let rows = walkthrough.steps().iter().enumerate();
        let rows = rows.map(|(index, step)| StepRow::of(index, step, Some(applied)));
        set_rows(&self.steps, rows.collect(), cx);
    }

    /// The figures [`Self::sync`] last built.
    pub(super) fn figures(&self) -> &Figures {
        self.figures
            .as_ref()
            .expect("the section syncs its figures before drawing")
    }
}

fn table<T: 'static>(
    columns: Vec<RowColumn<T>>,
    window: &mut Window,
    cx: &mut Context<AppState>,
) -> Table<T> {
    cx.new(|cx| TableState::new(RowsDelegate::new(columns), window, cx))
}

fn set_rows<T: 'static>(table: &Table<T>, rows: Vec<T>, cx: &mut Context<AppState>) {
    table.update(cx, |table, cx| {
        table.delegate_mut().set_rows(rows);
        cx.notify();
    });
}

/// The swap list's columns; the walkthrough adds each step's state.
fn step_columns(with_state: bool) -> Vec<RowColumn<StepRow>> {
    let mut columns = vec![
        RowColumn::number("number", "#", |row: &StepRow| row.number.clone())
            .column(|column| column.width(gpui::px(48.0))),
        RowColumn::text("from", "From", |row: &StepRow| row.from.clone()),
        RowColumn::text("to", "To", |row: &StepRow| row.to.clone()),
        RowColumn::text("at", "At", |row: &StepRow| row.at.clone()),
        RowColumn::text("source", "Source", |row: &StepRow| row.source.clone()),
    ];
    if with_state {
        columns.push(RowColumn::text("state", "State", |row: &StepRow| {
            row.state.clone()
        }));
    }
    columns
}

fn shelf_columns() -> Vec<RowColumn<ShelfRow>> {
    vec![
        RowColumn::text("shelf", "Shelf", |row: &ShelfRow| row.number.clone()),
        RowColumn::text("cog", "Cog", |row: &ShelfRow| row.cog.clone()),
        RowColumn::number("base", "Base", |row: &ShelfRow| row.base.clone()),
        RowColumn::text("surround", "Surround", |row: &ShelfRow| {
            row.surround.clone()
        }),
    ]
}

/// Every placeable shelf cog with the objective's base and surround.
fn shelf_rows(save: &CogSave, goal: Goal) -> Vec<ShelfRow> {
    save.shelf
        .iter()
        .enumerate()
        .map(|(index, cog)| {
            let (base, surround) = match goal {
                Goal::Build => (cog.a, cog.e),
                Goal::ConExp => (cog.d, cog.f),
                Goal::Flaggy => (cog.c, cog.g),
            };
            let surround = match cog.direction {
                Some(direction) if surround > 0.0 => {
                    format!("{} +{surround:.0}%", direction.name())
                }
                Some(direction) => direction.name().to_string(),
                None => "—".into(),
            };
            ShelfRow {
                number: format!("#{}", index + 1).into(),
                cog: cog.display_name().into(),
                base: if base >= 1e6 {
                    format::fmt_val(base)
                } else {
                    format!("{base:.0}")
                }
                .into(),
                surround: surround.into(),
            }
        })
        .collect()
}
