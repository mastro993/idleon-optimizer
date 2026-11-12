//! Cog board (W3): the saved layout, the game's two-phase totals and the
//! objective scorer.
//!
//! Ported from `js/cogs/cog-board.js` in
//! <https://github.com/Corgan/idleon-research-optimizer>. The board is 12x8
//! slots; every cog can surround other slots with its `e`/`f`/`g`/`j` values,
//! and the game applies those surrounds before it sums the base stats:
//!
//! 1. **Surround pass:** every directional cog adds its surround percentages to
//!    the slots its direction reaches. Yin pieces only contribute while their
//!    four pieces sit in the one 2x2 order the game accepts.
//! 2. **Totals pass:** build speed is `a × (1 + eT/100)`, Flaggy rate is
//!    `c × (1 + gT/100)`, construction EXP is the sum of every `d`, and the
//!    player's EXP/hr is `b × (1 + fT/100) × (1 + totalD/100)`.
//!
//! The optimizer is [`optimize()`], the board advice [`board_insights`], the
//! game's roll maths [`roll_ranges`] and save decoding [`CogSave`]; the
//! step-by-step replay of a finished swap list lives in [`SwapWalkthrough`].

mod display;
mod insights;
mod optimize;
mod perfect;
mod roll;
mod save;
mod walkthrough;

pub(crate) use display::stat_tier;
pub use display::{CogKind, classify, display_name};
pub use insights::{BoardInsights, Insight, RollTarget, board_insights};
pub use optimize::{CogResult, OptimizeOptions, SwapStep, apply_swaps, optimize};
pub use perfect::{
    PerfectBoard, PerfectLayer, PerfectStats, perfect_board, perfect_stats, player_slot,
};
pub use roll::{RollRange, raw_magnitude, roll_ranges};
pub(crate) use roll::{infer_construction_level, max_rolls, stat_from_value};
pub use save::CogSave;
pub use save::has_cog;
pub use walkthrough::SwapWalkthrough;

/// The board is 12 slots wide.
pub const BOARD_W: usize = 12;
/// The board is 8 slots tall.
pub const BOARD_H: usize = 8;
/// The board holds 96 cogs.
pub const BOARD_SIZE: usize = BOARD_W * BOARD_H;

/// The direction a cog's surround bonus travels, as the save's `h` field names
/// it. Indexes into the upstream `COG_MATRICES` order are kept in comments.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `0`: the four orthogonal neighbours.
    Adjacent,
    /// `1`: the four diagonals.
    Diagonal,
    /// `2`: two to the left and their diagonals.
    Left,
    /// `3`: two to the right and their diagonals.
    Right,
    /// `4`: two up and their diagonals.
    Up,
    /// `5`: two down and their diagonals.
    Down,
    /// `8`: the four slots two rows and two columns away.
    Corners,
    /// `10`: the eight neighbours and the four slots two away.
    Around,
    /// The whole row.
    Row,
    /// The whole column.
    Column,
    /// Every slot on the board.
    Everything,
}

/// The offsets of each matrix direction, from the upstream `COG_MATRICES`.
const ADJACENT: [(i32, i32); 4] = [(0, 1), (0, -1), (1, 0), (-1, 0)];
const DIAGONAL: [(i32, i32); 4] = [(1, 1), (1, -1), (-1, -1), (-1, 1)];
const LEFT: [(i32, i32); 6] = [(-1, 0), (-2, 0), (-1, 1), (-1, -1), (-2, 1), (-2, -1)];
const RIGHT: [(i32, i32); 6] = [(1, 0), (2, 0), (1, 1), (1, -1), (2, 1), (2, -1)];
const UP: [(i32, i32); 6] = [(0, -1), (0, -2), (-1, -1), (1, -1), (-1, -2), (1, -2)];
const DOWN: [(i32, i32); 6] = [(0, 1), (0, 2), (1, 1), (-1, 1), (1, 2), (-1, 2)];
const CORNERS: [(i32, i32); 4] = [(2, 2), (2, -2), (-2, 2), (-2, -2)];
const AROUND: [(i32, i32); 12] = [
    (0, 1),
    (0, -1),
    (1, 0),
    (-1, 0),
    (1, 1),
    (1, -1),
    (-1, -1),
    (-1, 1),
    (0, 2),
    (0, -2),
    (2, 0),
    (-2, 0),
];

impl Direction {
    /// The save's `h` string as a direction, or `None` for a value this build
    /// does not know (a missing direction is not a direction).
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "adjacent" => Self::Adjacent,
            "diagonal" => Self::Diagonal,
            "left" => Self::Left,
            "right" => Self::Right,
            "up" => Self::Up,
            "down" => Self::Down,
            "corners" => Self::Corners,
            "around" => Self::Around,
            "row" => Self::Row,
            "column" => Self::Column,
            "everything" => Self::Everything,
            _ => return None,
        })
    }

    /// The save's own name for the direction, as `h` stores it.
    pub fn name(self) -> &'static str {
        match self {
            Self::Adjacent => "adjacent",
            Self::Diagonal => "diagonal",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
            Self::Corners => "corners",
            Self::Around => "around",
            Self::Row => "row",
            Self::Column => "column",
            Self::Everything => "everything",
        }
    }

    /// The matrix offsets, or an empty slice for the row/column/everything
    /// directions the totals pass expands itself.
    fn offsets(self) -> &'static [(i32, i32)] {
        match self {
            Self::Adjacent => &ADJACENT,
            Self::Diagonal => &DIAGONAL,
            Self::Left => &LEFT,
            Self::Right => &RIGHT,
            Self::Up => &UP,
            Self::Down => &DOWN,
            Self::Corners => &CORNERS,
            Self::Around => &AROUND,
            Self::Row | Self::Column | Self::Everything => &[],
        }
    }
}

/// One cog as the save stores it. `a`/`b`/`c` are base build speed,
/// construction EXP and Flaggy rate; `d` is the cog's own construction EXP
/// percentage; `e`/`f`/`g`/`j` are the surrounds it gives.
#[derive(Clone, Debug)]
pub struct Cog {
    pub name: String,
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
    pub g: f64,
    pub j: f64,
    pub direction: Option<Direction>,
    pub i: f64,
    /// The save's `l` marker: a Yin (`CogZA`) piece.
    pub yin: bool,
    pub is_player: bool,
    /// The slot the cog came from, for the save view and the tooltip. It does
    /// not follow the cog when it moves.
    pub slot: usize,
}

impl PartialEq for Cog {
    /// Two cogs are interchangeable when their name and every stat match; the
    /// slot they sit in is not part of their identity, so a swap that only
    /// moves an identical cog is not reported.
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name
            && self.a == other.a
            && self.b == other.b
            && self.c == other.c
            && self.d == other.d
            && self.e == other.e
            && self.f == other.f
            && self.g == other.g
            && self.j == other.j
            && self.direction == other.direction
            && self.i == other.i
            && self.yin == other.yin
            && self.is_player == other.is_player
    }
}

impl Cog {
    /// A cog with every stat at zero; callers set what the save carries.
    pub fn named(name: &str, slot: usize) -> Self {
        Self {
            name: name.to_string(),
            a: 0.0,
            b: 0.0,
            c: 0.0,
            d: 0.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            j: 0.0,
            direction: None,
            i: 0.0,
            yin: false,
            is_player: name.starts_with("Player_"),
            slot,
        }
    }

    /// The Yin piece number (0-3), or `None` when the cog is not a Yin cog.
    /// Upstream reads the last character of the name.
    pub(crate) fn yin_piece(&self) -> Option<usize> {
        if !self.yin && !self.name.starts_with("CogZA") {
            return None;
        }
        let piece = self
            .name
            .chars()
            .last()
            .and_then(|ch| ch.to_digit(10))
            .unwrap_or(0) as usize;
        Some(piece)
    }

    /// The base stat the goal sums: `a` for build, `d` for construction EXP,
    /// `c` for Flaggy rate.
    pub(crate) fn goal_base(&self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.a,
            Goal::ConExp => self.d,
            Goal::Flaggy => self.c,
        }
    }

    /// The surround stat the goal cares about: `e`, `f` or `g`.
    pub(crate) fn goal_surround(&self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.e,
            Goal::ConExp => self.f,
            Goal::Flaggy => self.g,
        }
    }

    /// The name the upstream page shows in its result and swap list.
    pub fn display_name(&self) -> String {
        display_name(&self.name)
    }

    /// A two- or three-character label for the board grid.
    pub fn short_label(&self) -> String {
        match classify(&self.name) {
            CogKind::Player(name) => name.chars().take(5).collect(),
            CogKind::Yellow => "Yang".into(),
            CogKind::Excogia { piece } => format!("Yin{}", piece + 1),
            CogKind::Crystal { level } => format!("Cry{level}"),
            CogKind::Standard { tier, .. } => format!("T{tier}"),
            CogKind::Small { kind, level } => {
                format!("{}{}", ["F", "B", "X"][kind.min(2)], level + 1)
            }
            CogKind::Empty | CogKind::Unknown => "—".into(),
        }
    }
}

/// The objectives the optimizer can maximise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Goal {
    Build,
    #[default]
    ConExp,
    Flaggy,
}

impl Goal {
    /// Every goal in the section's control order.
    pub const ALL: [Goal; 3] = [Goal::Build, Goal::ConExp, Goal::Flaggy];

    /// The stable id the controls and the project config use.
    pub fn id(self) -> &'static str {
        match self {
            Goal::Build => "build",
            Goal::ConExp => "conexp",
            Goal::Flaggy => "flaggy",
        }
    }

    /// The label the controls show.
    pub fn label(self) -> &'static str {
        match self {
            Goal::Build => "Build Speed",
            Goal::ConExp => "Construction EXP",
            Goal::Flaggy => "Flaggy Rate",
        }
    }

    /// The saved goal id, if this build knows it.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|goal| goal.id() == id)
    }
}

/// The normalised weights of a weighted objective, one per [`Goal`].
///
/// The weights sum to one. Upstream divides the three fields by their total and
/// keeps the ratios; this build refuses the inputs it would silently replace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Weights {
    build: f64,
    conexp: f64,
    flaggy: f64,
}

impl Weights {
    /// Validate raw weights and normalise them to sum to one.
    ///
    /// Negative, non-finite and all-zero weights are rejected: there would be
    /// no objective to maximise.
    pub fn new(build: f64, conexp: f64, flaggy: f64) -> Result<Self, WeightError> {
        if !build.is_finite() || !conexp.is_finite() || !flaggy.is_finite() {
            return Err(WeightError::NotFinite);
        }
        if build < 0.0 || conexp < 0.0 || flaggy < 0.0 {
            return Err(WeightError::Negative);
        }
        let total = build + conexp + flaggy;
        if !total.is_finite() {
            return Err(WeightError::NotFinite);
        }
        if total <= 0.0 {
            return Err(WeightError::Empty);
        }
        Ok(Self {
            build: build / total,
            conexp: conexp / total,
            flaggy: flaggy / total,
        })
    }

    /// Equal thirds, the upstream default.
    pub fn uniform() -> Self {
        Self::new(1.0, 1.0, 1.0).expect("equal weights are valid")
    }

    /// One goal at full weight.
    pub fn single(goal: Goal) -> Self {
        let mut weights = Self {
            build: 0.0,
            conexp: 0.0,
            flaggy: 0.0,
        };
        *weights.get_mut(goal) = 1.0;
        weights
    }

    /// The weight of one goal, 0..=1.
    pub fn get(self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.build,
            Goal::ConExp => self.conexp,
            Goal::Flaggy => self.flaggy,
        }
    }

    fn get_mut(&mut self, goal: Goal) -> &mut f64 {
        match goal {
            Goal::Build => &mut self.build,
            Goal::ConExp => &mut self.conexp,
            Goal::Flaggy => &mut self.flaggy,
        }
    }
}

/// Why a set of weights cannot describe an objective.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeightError {
    /// A weight was NaN or infinite.
    NotFinite,
    /// A weight was negative.
    Negative,
    /// Every weight was zero, so there is nothing to maximise.
    Empty,
}

impl WeightError {
    /// The line the controls show under an invalid set.
    pub fn message(self) -> &'static str {
        match self {
            Self::NotFinite => "Weights must be finite numbers.",
            Self::Negative => "Weights cannot be negative.",
            Self::Empty => "At least one weight must be above zero.",
        }
    }
}

impl std::fmt::Display for WeightError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

/// The weight fields as the controls enter them, before validation.
///
/// The section keeps these raw so the fields show the user's own numbers; a
/// run normalises them with [`Weights::new`].
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct WeightInputs {
    pub build: f64,
    pub conexp: f64,
    pub flaggy: f64,
}

impl Default for WeightInputs {
    /// The upstream control defaults: 33 / 34 / 33.
    fn default() -> Self {
        Self {
            build: 33.0,
            conexp: 34.0,
            flaggy: 33.0,
        }
    }
}

impl WeightInputs {
    /// The weight of one goal, 0..=100.
    pub fn get(self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.build,
            Goal::ConExp => self.conexp,
            Goal::Flaggy => self.flaggy,
        }
    }

    /// Replace one goal's weight, clamped to the controls' range.
    pub fn set(&mut self, goal: Goal, value: f64) {
        let value = if value.is_finite() {
            value.clamp(0.0, 100.0)
        } else {
            0.0
        };
        match goal {
            Goal::Build => self.build = value,
            Goal::ConExp => self.conexp = value,
            Goal::Flaggy => self.flaggy = value,
        }
    }

    /// The validated, normalised weights, or why they cannot run.
    pub fn weights(self) -> Result<Weights, WeightError> {
        Weights::new(self.build, self.conexp, self.flaggy)
    }
}

/// What an optimizer run maximises.
#[derive(Clone, Debug, PartialEq)]
pub enum Objective {
    /// One raw metric, the single-objective runs.
    Goal(Goal),
    /// The weighted sum of each metric relative to a reference board, the
    /// upstream `scoreBoard` weighted branch. A metric the reference board does
    /// not have is left out, as upstream does.
    Weighted {
        weights: Weights,
        /// The metric values of the board the weights are read against, in
        /// [`Goal::ALL`] order.
        reference: [f64; 3],
    },
}

impl Objective {
    /// Maximise one raw metric.
    pub fn goal(goal: Goal) -> Self {
        Self::Goal(goal)
    }

    /// Weight the metrics against `reference`'s totals. Upstream normalises
    /// against the save board, so a run scores one on that board and the
    /// percentage improvement reads as a share of the save's own mix.
    pub fn weighted(weights: Weights, reference: &Board) -> Self {
        let totals = compute_totals(reference);
        Self::Weighted {
            weights,
            reference: [
                totals.total_build,
                totals.player_con_exp_hr,
                totals.total_flaggy,
            ],
        }
    }

    /// The goal the greedy heuristics sort and bias for: the only goal, or the
    /// heaviest weight. Ties keep the earlier goal in [`Goal::ALL`] order, as
    /// the upstream `_dominantGoal` does.
    pub fn dominant(&self) -> Goal {
        match self {
            Self::Goal(goal) => *goal,
            Self::Weighted { weights, .. } => {
                let mut best = Goal::Build;
                for goal in [Goal::ConExp, Goal::Flaggy] {
                    if weights.get(goal) > weights.get(best) {
                        best = goal;
                    }
                }
                best
            }
        }
    }

    /// The weights this objective applies, a single goal at full weight for a
    /// single-objective run.
    pub fn weights(&self) -> Weights {
        match self {
            Self::Goal(goal) => Weights::single(*goal),
            Self::Weighted { weights, .. } => *weights,
        }
    }
}

/// Where a cog sits: its column and row on the 12x8 board.
pub fn slot_to_pos(slot: usize) -> (usize, usize) {
    (slot % BOARD_W, slot / BOARD_W)
}

/// The slot at a column and row.
pub(crate) fn pos_to_slot(col: usize, row: usize) -> usize {
    col + row * BOARD_W
}

/// A board layout: 96 slots, empty ones `None`.
pub type Board = Vec<Option<Cog>>;

/// An empty 96-slot board.
pub fn empty_board() -> Board {
    vec![None; BOARD_SIZE]
}

/// One slot's share of the surround pass.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SlotTotals {
    pub surround_build: f64,
    pub surround_con_exp: f64,
    pub surround_flaggy: f64,
}

/// The board totals the two-phase computation produces.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BoardTotals {
    pub total_build: f64,
    /// Every cog's `d`, the multiplier the player's EXP/hr reads.
    pub total_con_exp: f64,
    pub total_flaggy: f64,
    /// The best player cog's `b × (1 + fT/100) × (1 + totalD/100)`.
    pub player_con_exp_hr: f64,
    pub player_slot: Option<usize>,
    pub per_slot: Vec<SlotTotals>,
}

/// The `everything`/`row`/`column` directions and the matrices, as one walk
/// over the slots a cog at `slot` reaches.
fn for_each_target(slot: usize, direction: Direction, mut visit: impl FnMut(usize)) {
    let (col, row) = slot_to_pos(slot);
    match direction {
        Direction::Everything => {
            for target in 0..BOARD_SIZE {
                visit(target);
            }
        }
        Direction::Row => {
            for column in 0..BOARD_W {
                visit(pos_to_slot(column, row));
            }
        }
        Direction::Column => {
            for row in 0..BOARD_H {
                visit(pos_to_slot(col, row));
            }
        }
        _ => {
            for (dc, dr) in direction.offsets() {
                let target_col = col as i32 + dc;
                let target_row = row as i32 + dr;
                if target_col < 0
                    || target_col >= BOARD_W as i32
                    || target_row < 0
                    || target_row >= BOARD_H as i32
                {
                    continue;
                }
                visit(pos_to_slot(target_col as usize, target_row as usize));
            }
        }
    }
}

/// Every 2x2 block's slots, row by row, in the game's TL/TR/BL/BR order.
pub(crate) fn block_slots() -> impl Iterator<Item = [usize; 4]> {
    (0..BOARD_H - 1).flat_map(|row| {
        (0..BOARD_W - 1).map(move |col| {
            [
                pos_to_slot(col, row),
                pos_to_slot(col + 1, row),
                pos_to_slot(col, row + 1),
                pos_to_slot(col + 1, row + 1),
            ]
        })
    })
}

/// The slots covered by a valid 2x2 Yin block.
pub(crate) fn active_yin(board: &Board) -> [bool; BOARD_SIZE] {
    let mut active = [false; BOARD_SIZE];
    for slots in block_slots() {
        let pieces = slots.map(|slot| {
            board
                .get(slot)
                .and_then(Option::as_ref)
                .filter(|cog| cog.yin)
                .and_then(Cog::yin_piece)
        });
        if pieces == [Some(0), Some(1), Some(2), Some(3)] {
            for slot in slots {
                active[slot] = true;
            }
        }
    }
    active
}

/// Add one cog's surround to every slot it reaches, unless it is a Yin piece
/// outside a valid 2x2 block.
fn accumulate(
    board: &Board,
    active: &[bool; BOARD_SIZE],
    field: fn(&Cog) -> f64,
    cells: &mut [f64; BOARD_SIZE],
) {
    for (slot, cog) in board.iter().enumerate().take(BOARD_SIZE) {
        let Some(cog) = cog.as_ref() else {
            continue;
        };
        if cog.yin && !active[slot] {
            continue;
        }
        let Some(direction) = cog.direction else {
            continue;
        };
        let value = field(cog);
        if value == 0.0 {
            continue;
        }
        for_each_target(slot, direction, |target| cells[target] += value);
    }
}

/// Compute the full board totals: the surround pass, then the totals pass.
pub fn compute_totals(board: &Board) -> BoardTotals {
    let active = active_yin(board);
    let mut e = [0.0; BOARD_SIZE];
    let mut f = [0.0; BOARD_SIZE];
    let mut g = [0.0; BOARD_SIZE];
    accumulate(board, &active, |cog| cog.e, &mut e);
    accumulate(board, &active, |cog| cog.f, &mut f);
    accumulate(board, &active, |cog| cog.g, &mut g);

    let mut totals = BoardTotals {
        per_slot: vec![SlotTotals::default(); BOARD_SIZE],
        ..BoardTotals::default()
    };
    for slot in 0..BOARD_SIZE {
        let (a, c, d) = board
            .get(slot)
            .and_then(Option::as_ref)
            .map_or((0.0, 0.0, 0.0), |cog| (cog.a, cog.c, cog.d));
        totals.total_build += a * (1.0 + e[slot] / 100.0);
        totals.total_con_exp += d;
        totals.total_flaggy += c * (1.0 + g[slot] / 100.0);
        totals.per_slot[slot] = SlotTotals {
            surround_build: e[slot],
            surround_con_exp: f[slot],
            surround_flaggy: g[slot],
        };
    }

    // The player's construction EXP/hr, from the best player cog on the board.
    let mut best = 0.0;
    for (slot, (cog, surround)) in board.iter().zip(f.iter()).enumerate() {
        let Some(cog) = cog.as_ref() else {
            continue;
        };
        if !cog.is_player || cog.b <= 0.0 {
            continue;
        }
        let exp = cog.b * (1.0 + surround / 100.0) * (1.0 + totals.total_con_exp / 100.0);
        if totals.player_slot.is_none() || exp > best {
            best = exp;
            totals.player_slot = Some(slot);
        }
    }
    if totals.player_slot.is_some() {
        totals.player_con_exp_hr = best;
    }
    totals
}

impl BoardTotals {
    /// The raw value of one goal, as the weighted objective reads it.
    pub fn metric(&self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.total_build,
            Goal::ConExp => self.player_con_exp_hr,
            Goal::Flaggy => self.total_flaggy,
        }
    }
}

/// One goal's score without materialising the per-slot table, for the hot loop
/// in the optimizer and for the insights' contribution check. The maths is
/// exactly [`compute_totals`]'s, so the score equals the goal's own total:
/// build speed, Flaggy rate, or the best player's construction EXP/hr.
pub fn goal_score(board: &Board, goal: Goal) -> f64 {
    let active = active_yin(board);
    match goal {
        Goal::Build => {
            let mut e = [0.0; BOARD_SIZE];
            accumulate(board, &active, |cog| cog.e, &mut e);
            (0..BOARD_SIZE)
                .filter_map(|slot| {
                    board
                        .get(slot)
                        .and_then(Option::as_ref)
                        .map(|cog| cog.a * (1.0 + e[slot] / 100.0))
                })
                .sum()
        }
        Goal::Flaggy => {
            let mut g = [0.0; BOARD_SIZE];
            accumulate(board, &active, |cog| cog.g, &mut g);
            (0..BOARD_SIZE)
                .filter_map(|slot| {
                    board
                        .get(slot)
                        .and_then(Option::as_ref)
                        .map(|cog| cog.c * (1.0 + g[slot] / 100.0))
                })
                .sum()
        }
        Goal::ConExp => {
            let mut f = [0.0; BOARD_SIZE];
            accumulate(board, &active, |cog| cog.f, &mut f);
            let total_d: f64 = (0..BOARD_SIZE)
                .filter_map(|slot| board.get(slot).and_then(Option::as_ref).map(|cog| cog.d))
                .sum();
            let multiplier = 1.0 + total_d / 100.0;
            let mut best: Option<f64> = None;
            for (cog, surround) in board.iter().zip(f.iter()) {
                let Some(cog) = cog.as_ref() else {
                    continue;
                };
                if !cog.is_player || cog.b <= 0.0 {
                    continue;
                }
                let exp = cog.b * (1.0 + surround / 100.0) * multiplier;
                if best.is_none_or(|current| exp > current) {
                    best = Some(exp);
                }
            }
            best.unwrap_or(0.0)
        }
    }
}

/// The objective's score for `board`.
///
/// A single goal is the raw metric. A weighted objective is
/// `Σ weight × metric / reference`, skipping metrics the reference board does
/// not have, exactly the upstream `scoreBoard` weighted branch.
pub fn score(board: &Board, objective: &Objective) -> f64 {
    match objective {
        Objective::Goal(goal) => goal_score(board, *goal),
        Objective::Weighted { weights, reference } => {
            let totals = compute_totals(board);
            Goal::ALL
                .into_iter()
                .enumerate()
                .filter(|(index, _)| reference[*index] > 0.0)
                .map(|(index, goal)| weights.get(goal) * totals.metric(goal) / reference[index])
                .sum()
        }
    }
}

/// Whether a direction from `slot` reaches any player cog on `board`.
pub(crate) fn direction_reaches_player(board: &Board, slot: usize, direction: Direction) -> bool {
    let mut reaches = false;
    for_each_target(slot, direction, |target| {
        if board
            .get(target)
            .and_then(Option::as_ref)
            .is_some_and(|cog| cog.is_player)
        {
            reaches = true;
        }
    });
    reaches
}

/// Whether a directional cog at `slot` reaches any player cog.
pub(crate) fn affects_player(board: &Board, slot: usize) -> bool {
    let Some(cog) = board.get(slot).and_then(Option::as_ref) else {
        return false;
    };
    let Some(direction) = cog.direction else {
        return false;
    };
    direction_reaches_player(board, slot, direction)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cog(name: &str, slot: usize) -> Cog {
        Cog::named(name, slot)
    }

    fn player(slot: usize, b: f64, d: f64) -> Cog {
        let mut cog = cog("Player_Ana", slot);
        cog.a = 100.0;
        cog.c = 50.0;
        cog.b = b;
        cog.d = d;
        cog
    }

    #[test]
    fn a_surround_reaches_only_the_slots_inside_the_board() {
        let mut board = empty_board();
        let mut directional = cog("Cog3ri", 0);
        directional.direction = Some(Direction::Right);
        directional.f = 10.0;
        board[0] = Some(directional);
        board[1] = Some(player(1, 100.0, 20.0));
        board[12] = Some(cog("Cog3A00", 12));
        let totals = compute_totals(&board);
        assert_eq!(totals.per_slot[1].surround_con_exp, 10.0);
        assert_eq!(
            totals.per_slot[12].surround_con_exp, 0.0,
            "a left/right surround never wraps rows"
        );
        assert_eq!(totals.player_con_exp_hr, 100.0 * 1.1 * 1.2);

        let mut board = empty_board();
        let mut directional = cog("Cog3ri", 11);
        directional.direction = Some(Direction::Right);
        directional.f = 10.0;
        board[11] = Some(directional);
        board[12] = Some(player(12, 100.0, 0.0));
        assert_eq!(
            compute_totals(&board).per_slot[12].surround_con_exp,
            0.0,
            "the slot after the right edge is off the board"
        );
    }

    #[test]
    fn yin_surround_only_counts_in_the_games_2x2_order() {
        let pieces = |order: [usize; 4]| {
            let mut board = empty_board();
            for (index, piece) in order.into_iter().enumerate() {
                let slot = match index {
                    0 => pos_to_slot(0, 0),
                    1 => pos_to_slot(1, 0),
                    2 => pos_to_slot(0, 1),
                    _ => pos_to_slot(1, 1),
                };
                let mut yin = cog(&format!("CogZA0{piece}"), slot);
                yin.yin = true;
                yin.f = if piece == 0 { 20.0 } else { 0.0 };
                yin.direction = Some(Direction::Everything);
                board[slot] = Some(yin);
            }
            board
        };
        let valid = compute_totals(&pieces([0, 1, 2, 3]));
        assert_eq!(valid.per_slot[48].surround_con_exp, 20.0);
        let swapped = compute_totals(&pieces([1, 0, 2, 3]));
        assert_eq!(swapped.per_slot[48].surround_con_exp, 0.0);
    }

    #[test]
    fn row_column_and_everything_cover_their_whole_line() {
        let mut board = empty_board();
        let mut row = cog("Cog3ro", 13);
        row.direction = Some(Direction::Row);
        row.f = 5.0;
        board[13] = Some(row);
        let totals = compute_totals(&board);
        assert_eq!(totals.per_slot[12].surround_con_exp, 5.0);
        assert_eq!(totals.per_slot[23].surround_con_exp, 5.0);
        assert_eq!(totals.per_slot[0].surround_con_exp, 0.0);

        let mut board = empty_board();
        let mut column = cog("Cog3co", 13);
        column.direction = Some(Direction::Column);
        column.f = 7.0;
        board[13] = Some(column);
        let totals = compute_totals(&board);
        assert_eq!(totals.per_slot[1].surround_con_exp, 7.0);
        assert_eq!(totals.per_slot[85].surround_con_exp, 7.0);

        let mut board = empty_board();
        let mut everything = cog("CogY", 7);
        everything.direction = Some(Direction::Everything);
        everything.f = 2.0;
        board[7] = Some(everything);
        let totals = compute_totals(&board);
        assert_eq!(totals.per_slot[0].surround_con_exp, 2.0);
        assert_eq!(totals.per_slot[95].surround_con_exp, 2.0);
    }

    #[test]
    fn the_best_player_sets_the_construction_exp_rate() {
        let mut board = empty_board();
        board[0] = Some(player(0, 100.0, 10.0));
        let mut second = player(1, 200.0, 10.0);
        second.name = "Player_Bo".into();
        board[1] = Some(second);
        let totals = compute_totals(&board);
        assert_eq!(totals.player_slot, Some(1));
        assert_eq!(totals.player_con_exp_hr, 200.0 * 1.2);
    }

    #[test]
    fn display_names_match_the_upstream_labels() {
        assert_eq!(display_name("Player_Ana"), "Ana");
        assert_eq!(display_name("CogY"), "Yang Cog");
        assert_eq!(display_name("CogZA01"), "Yin Cog");
        assert_eq!(display_name("CogCry3"), "Garnet Cog");
        assert_eq!(display_name("Cog3cr"), "Ulti Omni Cog");
        assert_eq!(display_name("Cog0A00"), "Nooby Cog");
        assert_eq!(display_name("CogSmb4"), "Construction EXP Tiny Cog T5");
        assert_eq!(display_name("Blank"), "Empty");
    }
}
