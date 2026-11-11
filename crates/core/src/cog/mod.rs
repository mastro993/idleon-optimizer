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
//! The optimizer lives in [`optimize`]. Save decoding lives in [`save`].

mod display;
mod optimize;
mod save;

pub use display::{CogKind, classify, display_name, small_cog};
pub use optimize::{CogResult, OptimizeOptions, SwapStep, apply_swaps, optimize};
pub use save::CogSave;
pub(crate) use save::has_cog;

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
    pub fn from_name(name: &str) -> Option<Self> {
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
    pub fn yin_piece(&self) -> Option<usize> {
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
    pub fn goal_base(&self, goal: Goal) -> f64 {
        match goal {
            Goal::Build => self.a,
            Goal::ConExp => self.d,
            Goal::Flaggy => self.c,
        }
    }

    /// The surround stat the goal cares about: `e`, `f` or `g`.
    pub fn goal_surround(&self, goal: Goal) -> f64 {
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

/// Where a cog sits: its column and row on the 12x8 board.
pub fn slot_to_pos(slot: usize) -> (usize, usize) {
    (slot % BOARD_W, slot / BOARD_W)
}

/// The slot at a column and row.
pub fn pos_to_slot(col: usize, row: usize) -> usize {
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

/// The slots covered by a valid 2x2 Yin block, in the game's TL/TR/BL/BR order.
fn active_yin(board: &Board) -> [bool; BOARD_SIZE] {
    let mut active = [false; BOARD_SIZE];
    for row in 0..BOARD_H - 1 {
        for col in 0..BOARD_W - 1 {
            let slots = [
                pos_to_slot(col, row),
                pos_to_slot(col + 1, row),
                pos_to_slot(col, row + 1),
                pos_to_slot(col + 1, row + 1),
            ];
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

/// The goal's score without materialising the per-slot table, for the hot loop
/// in the optimizer. The maths is exactly [`compute_totals`]'s.
pub(crate) fn score(board: &Board, goal: Goal) -> f64 {
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
pub fn affects_player(board: &Board, slot: usize) -> bool {
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
