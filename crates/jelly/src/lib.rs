//! Jelly Operator (W7): the board, its cells and the operation estimate.
//!
//! Ports the overview, layout and simulator tabs of `jelly-operator-calc.html`
//! from the upstream `idleon-research-optimizer`; every function names the
//! upstream one it rewrites:
//!
//! - [`Jelly`] is the decoded save slice (`Research[7]`, `[14]`, `[15]`,
//!   `[16]`, `[17]`, `[18]`) plus the account terms the page reads from other
//!   systems ([`Account`]).
//! - [`board`] is `jellyLayoutFromSave`, `jellyFootprintSlots`,
//!   `jellyUnlockedSlots`, `jellyValidateLayout`, `jellyPlaceCell`,
//!   `jellyRemoveCell` and the simulator's `normalizeSimulatorLayout` reflow.
//! - [`metrics`] is `jellyLayoutMetrics` and the multipliers it multiplies:
//!   `jellyCellDamageMultiplier`, `jellyCellSpeedMultiplier`,
//!   `jellyCellExpMultiplier`, `jellyAttackCadence`, `jellyFeverBonus` and the
//!   Bloodcell currency factors.
//! - [`operation`] is `simulateJellyOperation`, the seeded 60 FPS runtime.
//! - [`Estimate`] is `jellyOperationEstimate`: `simulateJellyTrials` over
//!   `simulateJellyOperation` and its `summarizeJellyTrials` aggregate.
//! - [`Insights`] is `jellyOperationInsights` and [`surrogate`] is
//!   `jellyOperationSurrogate`, the deterministic projection the obstruction
//!   outlook rows use.
//! - [`planner`] is `planJellySections`: it screens every unpurchased board
//!   section, completes the board around the strongest, and compares the
//!   finalists with shared-seed trials.
//!
//! The shape overrides the upstream page persists for grid nodes 185 and 187
//! are not ported: they are a localStorage correction for a save-export timing
//! bug, and this app reads the save's own shape overlay.

pub mod board;
pub mod data;
pub mod estimate;
pub mod insights;
pub mod metrics;
pub mod operation;
pub mod planner;
pub mod surrogate;

#[cfg(test)]
mod tests;

pub use board::Layout;
pub use data::data;
pub use estimate::Estimate;
pub use insights::{ExpOutlook, Insights, ObstructionOutlook};
pub use metrics::{Cadence, CellMetrics, MetricOptions, Metrics};
pub use operation::{OperationOptions, Trial};

/// The slots the board starts with (`STARTING_SLOTS`).
pub const STARTING_SLOTS: [usize; 4] = [77, 95, 78, 96];
/// The slots the first obstruction clear unlocks (`FIRST_CLEAR_SLOTS`).
pub const FIRST_CLEAR_SLOTS: [usize; 2] = [76, 94];
/// The slots the second obstruction clear unlocks (`SECOND_CLEAR_SLOTS`).
pub const SECOND_CLEAR_SLOTS: [usize; 2] = [75, 93];

/// The slots whose cells carry the Proximity passive.
pub const PROXIMITY_ANCHORS: [usize; 16] = [
    43, 44, 45, 46, 60, 65, 78, 83, 96, 101, 114, 119, 133, 134, 135, 136,
];

/// The fever modes, in the game's own order (`FEVER_MODE_NAMES`).
pub const FEVER_MODES: [&str; 6] = ["COLD", "RASH", "SEPSIS", "NAUSEA", "RABIES", "PLAGUE"];

/// The fever modes' effect text, as the page's simulator lists them.
pub const FEVER_BONUSES: [&str; 6] = [
    "+1% all damage each elapsed second",
    "2.00x all damage",
    "4.00x Bloodcell gains",
    "2.00x EXP gains for all cells",
    "1.50x all damage and 1.25x all speed",
    "1.40x all speed",
];

/// `CurrencyMulti`'s innermost pool adds a flat +11%.
pub const BASE_BLOODCELL_BONUS_PCT: f64 = 11.0;

/// Stronkroid's duration (`SteroidTimeTOT`) and its per-frame drain.
pub const ROID_TIME_TOTAL: f64 = 300.0;
pub const ROID_DECREMENT_PER_FRAME: f64 = 0.61;

/// The frame rate the upstream runtime simulates.
pub(crate) const FRAME_SECONDS: f64 = 1.0 / 60.0;

/// How many frames Stronkroid stays active: 300 drained by 0.61 a frame.
pub fn roid_active_frames() -> u32 {
    let mut remaining = ROID_TIME_TOTAL;
    let mut frames = 0;
    while remaining > 0.0 {
        remaining -= ROID_DECREMENT_PER_FRAME;
        if remaining > 0.0 {
            frames += 1;
        }
    }
    frames
}

/// The seconds Stronkroid stays active.
pub fn roid_duration_seconds() -> f64 {
    f64::from(roid_active_frames()) * FRAME_SECONDS
}

/// The save's Jelly Operator progress row (`Research[7]`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Progress {
    pub obstruction: usize,
    pub tries: f64,
    pub bloodcells: f64,
    pub best_dps: f64,
    pub fever: u32,
    pub best_bloodcells: f64,
}

/// The account terms outside the Jelly Operator rows.
///
/// Every field is what the named upstream system returns from this save, so the
/// section's maths stays arithmetic and its tests need no save.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Account {
    /// Research grid node 185, Cellular Warfare (`gbWith(…, 185)`).
    pub grid_185: f64,
    /// Research grid node 187, Jelly Operator Linguistics.
    pub grid_187: f64,
    /// Spelunking palette 1 (`computePaletteBonus(1)`).
    pub palette_1: f64,
    /// Legend Talent 41 (`legendPTSbonus(41)`).
    pub legend_41: f64,
    /// Bloodclot artifact tier (`Sailing[3][40]`).
    pub bloodclot_tier: f64,
    /// Arcade 72, Jelly Bloodcell gain.
    pub arcade_72: f64,
    /// Atom 15, Sulfur (`atomsData[15] × AtomInfo[15][4]`).
    pub atom_15: f64,
    /// Whether the `ban_j` bundle is owned.
    pub bundle_ban_j: bool,
    /// Event shop 56 (Bloodcell gain) and 57 (a plot token).
    pub event_shop_56: f64,
    pub event_shop_57: f64,
    /// Legend Talent 41's plot tokens: `min(3, ceil(Spelunk[18][41] / 2))`.
    pub legend_slot_tokens: f64,
    /// Sushi `rogBonusQTY(63)`, which widens the Organelle multiplier.
    pub rog_63: f64,
    /// The Jelly upgrade indices Menagerie's Upgrade Doubler doubles.
    pub doubled: Vec<usize>,
    /// Research grid node 186, the daily attempt count.
    pub grid_186: f64,
}

/// The save's Jelly Operator state.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Jelly {
    pub progress: Progress,
    /// `Research[14]`: the saved layout, anchor to cell type.
    pub layout: Layout,
    pub cell_exp: [f64; 9],
    pub cell_level: [u32; 9],
    /// `Research[17]`: the upgrade levels.
    pub upgrades: Vec<u32>,
    /// `Research[18]`: the plot token ids the player bought.
    pub plots: Vec<usize>,
    pub account: Account,
}

impl Jelly {
    /// `hasJellyData`: the save carries the layout and upgrade rows.
    pub fn has_data(&self) -> bool {
        !self.upgrades.is_empty()
    }

    /// `jellyUpgradeLevel`.
    pub fn upgrade_level(&self, id: usize) -> f64 {
        self.upgrades.get(id).copied().unwrap_or(0) as f64
    }

    /// `jellyUpgradeDoubled`.
    pub fn upgrade_doubled(&self, id: usize) -> bool {
        self.account.doubled.contains(&id)
    }

    /// `jellyUpgradeQuantity`: the upgrade's per-level value times its level,
    /// doubled when Menagerie's Upgrade Doubler covers it.
    pub fn upgrade_quantity(&self, id: usize) -> f64 {
        let Some(upgrade) = data().upgrade(id) else {
            return 0.0;
        };
        let per_level = upgrade.per_level;
        let level = self.upgrade_level(id);
        if self.upgrade_doubled(id) {
            2.0 * per_level * level
        } else {
            per_level * level
        }
    }

    /// `jellyUnitsOwned`: how many cell types the save unlocked, capped at 8.
    pub fn units_owned(&self) -> usize {
        let total: f64 = (0..8).map(|id| self.upgrade_quantity(id)).sum();
        (total.round() as usize).min(8)
    }

    /// `jellyCellLevel`.
    pub fn cell_level(&self, cell_type: usize) -> u32 {
        self.cell_level.get(cell_type).copied().unwrap_or(0)
    }

    /// `jellyCellExp`.
    pub fn cell_exp(&self, cell_type: usize) -> f64 {
        self.cell_exp.get(cell_type).copied().unwrap_or(0.0)
    }

    /// `jellyCellExpRequirement`: `20 × 1.3^level`.
    pub fn cell_exp_requirement(&self, cell_type: usize) -> f64 {
        20.0 * 1.3_f64.powi(self.cell_level(cell_type) as i32)
    }

    /// `jellyTotalCellLevel`.
    pub fn total_cell_level(&self) -> f64 {
        (0..9)
            .map(|cell_type| f64::from(self.cell_level(cell_type)))
            .sum()
    }

    /// `jellyCompletionBonus`: a reward is active once its obstruction cleared.
    pub fn completion_bonus(&self, index: usize) -> f64 {
        if self.progress.obstruction > index {
            data()
                .reward(index)
                .map(|reward| reward.value)
                .unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// `jellyDailyTries`: `2 + gridLevels[186]`.
    pub fn daily_tries(&self) -> f64 {
        (2.0 + self.account.grid_186).round()
    }

    /// `jellyLegendSlotTokens`.
    pub fn legend_slot_tokens(&self) -> f64 {
        self.account.legend_slot_tokens.min(3.0)
    }
}

/// `jellyBossHp`: the obstruction's HP curve.
pub fn boss_hp(obstruction: usize) -> f64 {
    const EARLY: [f64; 12] = [
        100.0, 200.0, 400.0, 1000.0, 2000.0, 4000.0, 6000.0, 10000.0, 15000.0, 30000.0, 50000.0,
        100000.0,
    ];
    if let Some(hp) = EARLY.get(obstruction) {
        return *hp;
    }
    let over = obstruction as f64 - 11.0;
    100_000.0 * 1.65_f64.powf(over) * (1.0 + 0.9 * (over / 12.0).floor())
}

/// `jellyBossTime`: the normal-phase time limit in seconds.
pub fn boss_time(obstruction: usize) -> f64 {
    30.0 + 5.0 * (obstruction as f64 / 12.0).floor()
}

/// `jellyBossAttackCooldown`: the frames between boss attacks.
pub fn boss_attack_cooldown(obstruction: usize) -> f64 {
    (120.0 - obstruction as f64 / 6.0 * 20.0).max(10.0)
}

/// The fever mode's name, or `Fever <n>` past the six the game defines.
pub fn fever_mode_name(fever: u32) -> String {
    FEVER_MODES
        .get(fever as usize)
        .map(|name| (*name).to_string())
        .unwrap_or_else(|| format!("Fever {}", fever + 1))
}

/// The fever mode's effect text, or `None` past the six the game defines.
pub fn fever_mode_bonus(fever: u32) -> Option<&'static str> {
    FEVER_BONUSES.get(fever as usize).copied()
}
