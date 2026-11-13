//! Scurge combat: the EXP a Clamworks kill banks, the Self Affliction
//! comparison and the allocation of unspent Scurge points.
//!
//! Ports `js/stats/systems/w7/clamworks-scurge.js` and the `scurgeKill` half of
//! `js/stats/systems/common/combat-sim-progress.js`: one Clamworks kill banks
//! `(1 + RECKONING) × (1 + EVEN BETTER / 100) × (1 + SELF AFFLICTION / 100) ×
//! (1 + SWARMING × other players / 100)` Scurge EXP.
//!
//! The two upstream panels are one plan in two modes. The affliction mode moves
//! only SELF AFFLICTION, from zero to the selected limit, and leaves every other
//! upgrade saved; the upgrade mode holds SELF AFFLICTION and spends the unspent
//! points across the unlocked RECKONING, EVEN BETTER and SWARMING to maximise
//! immediate EXP per kill. Points freed by a lower SELF AFFLICTION stay unspent,
//! as upstream's own note says.
//!
//! Upstream measures both panels by running the combat simulator. Clam Work is
//! not a map that simulator runs here, so the app multiplies
//! [`Row::exp_per_kill`] by the character's kills per hour, the same stand-in the
//! AFK claim takes.

use super::{Clamworks, SCURGE_COUNT};
use crate::data::clamworks;

/// The most Self Affliction levels upstream accepts.
pub const MAX_AFFLICTION: u32 = 150;
/// The most other players upstream accepts.
pub const MAX_OTHER_PLAYERS: u32 = 50;

/// SELF AFFLICTION, the one upgrade the affliction mode moves.
const SELF_AFFLICTION: usize = 2;
/// The EXP upgrades the allocation may spend on, in upstream order.
const ALLOCATED: [usize; 3] = [0, 4, 7];

/// The two upstream comparison panels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// One Self Affliction level per row, every other upgrade saved.
    #[default]
    Affliction,
    /// The unspent points spread across the unlocked EXP upgrades.
    Upgrades,
}

impl Mode {
    pub const ALL: [Mode; 2] = [Mode::Affliction, Mode::Upgrades];

    pub fn id(self) -> &'static str {
        match self {
            Mode::Affliction => "affliction",
            Mode::Upgrades => "upgrades",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Mode::Affliction => "Self Affliction comparison",
            Mode::Upgrades => "Scurge upgrade optimization",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.id() == id)
    }
}

/// What the two panels ask for: the mode, the Self Affliction limit and how many
/// other real players share the map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Inputs {
    pub mode: Mode,
    pub max_affliction: u32,
    pub other_players: u32,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            mode: Mode::Affliction,
            max_affliction: 12,
            other_players: 0,
        }
    }
}

/// Why a Scurge comparison cannot be calculated.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// `Menagerie[2][4]` is not bought, so the Scurge menu does not exist.
    NotUnlocked,
    /// The saved upgrades spend more points than the saved Scurge level grants.
    OverBudget,
    MaxAffliction,
    OtherPlayers,
}

impl Error {
    pub fn message(self) -> &'static str {
        match self {
            Error::NotUnlocked => "Scurge is not unlocked in this save.",
            Error::OverBudget => {
                "The saved Scurge upgrades spend more points than the saved Scurge level grants."
            }
            Error::MaxAffliction => "Choose a maximum Self Affliction level from 0 to 150.",
            Error::OtherPlayers => "Enter 0-50 other real players on the map.",
        }
    }
}

/// Which comparison produced a row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The saved allocation.
    Current,
    /// SELF AFFLICTION at this level, everything else saved.
    Affliction(u32),
    /// The unspent points spread across the EXP upgrades.
    Optimized,
}

impl Kind {
    pub fn label(self) -> String {
        match self {
            Kind::Current => "Current loadout".into(),
            Kind::Affliction(level) => format!("Self Affliction {level}"),
            Kind::Optimized => "Optimized EXP upgrades".into(),
        }
    }
}

/// One Scurge upgrade level a row moves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Change {
    pub index: usize,
    pub from: u32,
    pub to: u32,
}

impl Change {
    /// The game's name for the upgrade this row moves.
    pub fn name(&self) -> &'static str {
        &clamworks().scurge[self.index].name
    }
}

/// One allocation the comparison shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub kind: Kind,
    /// The nine Scurge upgrade levels this row runs at.
    pub levels: [u32; SCURGE_COUNT],
    /// The Scurge points the allocation leaves unspent.
    pub points_left: u32,
    /// Scurge EXP one Clamworks kill banks at these levels.
    pub exp_per_kill: f64,
    pub changes: Vec<Change>,
}

impl Row {
    pub fn self_affliction(&self) -> u32 {
        self.levels[SELF_AFFLICTION]
    }

    /// Scurge EXP the row banks per hour at `kills_per_hour`.
    pub fn exp_per_hour(&self, kills_per_hour: f64) -> f64 {
        self.exp_per_kill * kills_per_hour.max(0.0)
    }
}

/// The comparison: the saved allocation and every row one mode tests.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub inputs: Inputs,
    /// The saved Scurge level, `Menagerie[0][8]`.
    pub rank: u32,
    /// The points the saved allocation leaves unspent.
    pub available: u32,
    /// The points a Self Affliction row may spend at most, refunded points
    /// included.
    pub self_budget: u32,
    /// The highest Self Affliction level the comparison tests, `0` when the
    /// upgrade is still locked.
    pub maximum: u32,
    pub rows: Vec<Row>,
}

impl Plan {
    /// Build the comparison the inputs ask for, or why it cannot be built.
    pub fn build(clamworks: &Clamworks, inputs: Inputs) -> Result<Self, Error> {
        if inputs.max_affliction > MAX_AFFLICTION {
            return Err(Error::MaxAffliction);
        }
        if inputs.other_players > MAX_OTHER_PLAYERS {
            return Err(Error::OtherPlayers);
        }
        if !clamworks.scurge_unlocked {
            return Err(Error::NotUnlocked);
        }
        let rank = clamworks.scurge_level;
        let saved = clamworks.scurge_upgrades;
        // A save can carry any level the importer could read, so the totals are
        // summed wider than a level and saturate instead of wrapping.
        let spent: u64 = saved.iter().map(|&level| u64::from(level)).sum();
        let Some(available) = u64::from(rank).checked_sub(spent) else {
            return Err(Error::OverBudget);
        };
        let available = available as u32;
        let self_budget = available.saturating_add(saved[SELF_AFFLICTION]);
        let maximum = if rank >= unlock(SELF_AFFLICTION) {
            self_budget.min(inputs.max_affliction)
        } else {
            0
        };
        let mut plan = Self {
            inputs,
            rank,
            available,
            self_budget,
            maximum,
            rows: vec![row(Kind::Current, saved, rank, inputs.other_players)],
        };
        // The upgrade mode holds Self Affliction at its saved level and spends
        // the unspent points; the affliction mode tests one level at a time.
        let tested: Vec<u32> = match inputs.mode {
            Mode::Affliction => (0..=maximum).collect(),
            Mode::Upgrades => vec![saved[SELF_AFFLICTION]],
        };
        for self_affliction in tested {
            let mut allocation = saved;
            allocation[SELF_AFFLICTION] = self_affliction;
            if inputs.mode == Mode::Upgrades {
                allocate(&mut allocation, available, rank, inputs.other_players);
            }
            if allocation == saved {
                continue;
            }
            let kind = match inputs.mode {
                Mode::Affliction => Kind::Affliction(self_affliction),
                Mode::Upgrades => Kind::Optimized,
            };
            let mut candidate = row(kind, allocation, rank, inputs.other_players);
            candidate.changes = (0..SCURGE_COUNT)
                .filter(|&index| allocation[index] != saved[index])
                .map(|index| Change {
                    index,
                    from: saved[index],
                    to: allocation[index],
                })
                .collect();
            plan.rows.push(candidate);
        }
        Ok(plan)
    }

    /// The saved allocation, every row is compared against it.
    pub fn baseline(&self) -> &Row {
        &self.rows[0]
    }

    /// The row banking the most Scurge EXP per kill. Every row shares one
    /// kills-per-hour input, so this is also upstream's best EXP an hour; ties
    /// keep the earlier row, as upstream's own reduction does.
    pub fn best(&self) -> &Row {
        let mut best = self.baseline();
        for row in &self.rows[1..] {
            if row.exp_per_kill > best.exp_per_kill {
                best = row;
            }
        }
        best
    }

    /// The extra Scurge EXP an hour the row banks over the saved allocation, as
    /// upstream's `expGainPerHour` prints it, zero for the baseline.
    pub fn gain(&self, row: &Row, kills_per_hour: f64) -> f64 {
        row.exp_per_hour(kills_per_hour) - self.baseline().exp_per_hour(kills_per_hour)
    }
}

/// `Scelunky[28][t + 9] × Menagerie[5][t]`, the game's own Scurge bonus.
fn bonus(levels: &[u32; SCURGE_COUNT], index: usize) -> f64 {
    clamworks().scurge[index].per_lv * f64::from(levels[index])
}

/// The Scurge level one upgrade unlocks at.
fn unlock(index: usize) -> u32 {
    clamworks().scurge[index].unlock_level.max(0.0) as u32
}

/// `_expPerKill`: what one Clamworks kill banks at `levels`.
fn exp_per_kill(levels: &[u32; SCURGE_COUNT], other_players: u32) -> f64 {
    let other_players = f64::from(other_players);
    (1.0 + bonus(levels, 0))
        * (1.0 + bonus(levels, 4) / 100.0)
        * (1.0 + bonus(levels, SELF_AFFLICTION) / 100.0)
        * (1.0 + bonus(levels, 7) * other_players / 100.0)
}

/// One row at `levels`, with the changes filled in against the saved allocation
/// by the caller.
fn row(kind: Kind, levels: [u32; SCURGE_COUNT], rank: u32, other_players: u32) -> Row {
    let spent: u64 = levels.iter().map(|&level| u64::from(level)).sum();
    Row {
        kind,
        points_left: u64::from(rank).saturating_sub(spent) as u32,
        exp_per_kill: exp_per_kill(&levels, other_players),
        levels,
        changes: Vec::new(),
    }
}

/// `_allocate`: the unspent points spread across the unlocked EXP upgrades.
///
/// Their equal-cost marginal gain is logarithmic, so the maximum is the level
/// where the sum of the gains reaches the budget; a binary search finds that
/// water level, the integer leftovers settle on the largest marginal gain, and
/// SWARMING's coefficient scales with the other players on the map.
fn allocate(levels: &mut [u32; SCURGE_COUNT], points: u32, rank: u32, other_players: u32) {
    let factors: Vec<(usize, f64)> = ALLOCATED
        .into_iter()
        .filter_map(|index| {
            let coefficient = clamworks().scurge[index].per_lv
                * match index {
                    0 => 1.0,
                    7 => f64::from(other_players) / 100.0,
                    _ => 1.0 / 100.0,
                };
            (rank >= unlock(index) && coefficient > 0.0).then(|| (index, 1.0 / coefficient))
        })
        .collect();
    if points == 0 || factors.is_empty() {
        return;
    }
    let mut low = 0.0;
    let mut high = factors
        .iter()
        .map(|&(index, offset)| f64::from(levels[index]) + offset)
        .fold(0.0f64, f64::max)
        + f64::from(points);
    for _ in 0..64 {
        let mid = (low + high) / 2.0;
        let used: f64 = factors
            .iter()
            .map(|&(index, offset)| (mid - f64::from(levels[index]) - offset).max(0.0))
            .sum();
        if used <= f64::from(points) {
            low = mid;
        } else {
            high = mid;
        }
    }
    let mut left = points;
    for &(index, offset) in &factors {
        let add =
            f64::from(left).min((low - f64::from(levels[index]) - offset).max(0.0).floor()) as u32;
        levels[index] = levels[index].saturating_add(add);
        left -= add;
    }
    while left > 0 {
        let mut best = factors[0];
        for &(index, offset) in &factors[1..] {
            let gain = 1.0 / (f64::from(levels[index]) + offset);
            if gain > 1.0 / (f64::from(levels[best.0]) + best.1) {
                best = (index, offset);
            }
        }
        levels[best.0] = levels[best.0].saturating_add(1);
        left -= 1;
    }
}
