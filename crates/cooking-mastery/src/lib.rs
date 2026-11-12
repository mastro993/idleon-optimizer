//! Cooking Mastery (W4): the current Mastery EXP rate and the upgrade ranking.
//!
//! Ports the Mastery EXP tab of `cooking-mastery-calc.html` from the upstream
//! `idleon-research-optimizer`. The page keeps its maths inline, so every
//! function here names the page function it rewrites:
//!
//! - [`Mastery::exp_rate`] is `computeExpRate`: a base rate of 2 multiplied by
//!   each EXP upgrade's factor and the external multipliers.
//! - [`Mastery::scaling`] is `scalingFactor`: what each upgrade scales with
//!   (ladles used, account cooking level, Divorce Cake level, ribbon ranks and
//!   the mastery rank itself).
//! - [`Mastery::bonus`] is `bonusTotal`: `perLv × level × scaling`, a percentage
//!   of the rate.
//! - [`Mastery::exp_required`] is `expReq`: `100 × 2.5^lv × 5^max(0, lv − 40)`.
//! - [`Mastery::ranking`] is `findBestUpgrade`: the five EXP upgrades ordered by
//!   the percentage gain one more point would give.
//! - [`Mastery::breakdown`] is the page's `renderBreakdownTree` call: the same
//!   factors as a formula tree, whose root is the rate the cards show.
//! - [`Mastery::distribution`] is the page's `optimizeDistribution`: the daily
//!   reset budget spent across the EXP upgrades, for a [`Goal`].
//! - [`Yellow::allocation`] is the page's second tab, `optimizeForGoal`: the
//!   yellow point budget spent across the meals of one meal goal.
//!
//! SMOKY (ribbon rank chance) is not an EXP upgrade upstream and is left out of
//! the ranking and the tree, exactly as the page's `EXP_UPGRADES` list leaves it
//! out; a save's SMOKY level never changes the rate.
//!
//! The save-derived inputs arrive from the account decoder (`idleon_account`),
//! which owns the field names; this crate only does arithmetic, so its unit
//! tests need no save.

mod breakdown;
mod data;
mod distribution;
mod yellow;

pub use data::{Meal, meals};
pub use distribution::{Goal, Plan, Row};
pub use yellow::{
    Allocation, GOALS, Kitchen, Kitchens, MealCharacter, MealRow, Unit, Value, Yellow, YellowGoal,
    bonus_multi_cook, goal,
};

use idleon_common::formula::FormulaNode;

/// The mastery upgrades, in the game's own order.
pub const UPGRADE_NAMES: [&str; 6] = [
    "SALTY — Ladles Used",
    "SPICY — Account Cook LV",
    "SWEET — Divorce Cake",
    "SMOKY — Ribbon Rank+",
    "SOUR — Total Ribbons",
    "SAVORY — Mastery Rank",
];

/// The mastery level each upgrade unlocks at (`RANK_REQ`).
pub const REQUIRED_LEVEL: [u32; 6] = [0, 1, 5, 10, 25, 100];

/// The upgrades that feed the EXP rate (`EXP_UPGRADES`), in page order.
pub const EXP_UPGRADES: [usize; 5] = [0, 1, 2, 4, 5];

/// The index of SMOKY, the one upgrade the EXP table leaves out.
pub const RIBBON_CHANCE: usize = 3;

/// The rate every multiplier applies to (`var rate = 2`).
const BASE_RATE: f64 = 2.0;

/// The mastery points the Rift Spooker companion grants.
const COMPANION_POINTS: f64 = 5.0;

/// The account multipliers outside the mastery upgrades.
///
/// Every field is a percentage except [`Self::companion_87`], which is the
/// companion's own bonus and enters its factor doubled.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct External {
    /// Menagerie boost 21, `menagerieBoostQTY(21)`.
    pub menagerie: f64,
    /// Research grid node 190, Masterius Cookerius (`gbWith(…, 190)`).
    pub grid_190: f64,
    /// Zuperbit 68, Cooking Master: 1 when unlocked, 40% of it apply.
    pub super_bit_68: f64,
    /// Companion 87, Rift Spooker; its factor is `1 + 2 × bonus`.
    pub companion_87: f64,
    /// The Cook Maxxing fountain upgrade, `Fountain_BonTOT(2, 17)`.
    pub fountain: f64,
    /// The Canteen Read vial, with its lab, vault and meritocracy factors.
    pub vial: f64,
    /// Arcade 69, Cook Mastery EXP.
    pub arcade: f64,
    /// Salt lick 10, Refinery6.
    pub salt_lick: f64,
    /// The vial's level, which its tree node names.
    pub vial_level: f64,
}

impl External {
    /// `additivePct`: the vial, arcade and salt lick terms add together.
    pub fn additive(&self) -> f64 {
        self.vial + self.arcade + self.salt_lick
    }

    /// `computeExternalMulti().total`: the product of every external factor.
    pub fn total(&self) -> f64 {
        (1.0 + self.menagerie / 100.0)
            * (1.0 + self.grid_190 / 100.0)
            * (1.0 + 40.0 * self.super_bit_68 / 100.0)
            * (1.0 + 2.0 * self.companion_87)
            * (1.0 + self.fountain / 100.0)
            * (1.0 + self.additive() / 100.0)
    }
}

/// One save's Cooking Mastery state, as the Mastery EXP tab reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mastery {
    /// `RandoListo2[8]`: the bonus one level of each upgrade adds.
    pub per_lv: [f64; 6],
    /// `CookMaster[1][0]`: the mastery rank.
    pub level: f64,
    /// `CookMaster[1][3]`: the ladles the account has used.
    pub ladles: f64,
    /// The summed Cooking level of every character on the save.
    pub cooking_levels: f64,
    /// `Meals[0][73]`: the Divorce Cake meal's level.
    pub divorce_cake: f64,
    /// The summed ribbon ranks from slot 28 on.
    pub ribbons: f64,
    /// `CookMaster[2]`: the level of each upgrade, by [`UPGRADE_NAMES`] index.
    pub upgrades: [f64; 6],
    /// Whether companion 87 is owned; it grants five mastery points.
    pub companion_87_owned: bool,
    /// The account multipliers outside the mastery upgrades.
    pub external: External,
}

/// One row of the upgrade comparison (`findBestUpgrade`'s `results`).
#[derive(Clone, Debug, PartialEq)]
pub struct Upgrade {
    /// The upgrade's index in [`UPGRADE_NAMES`].
    pub index: usize,
    pub name: &'static str,
    /// The saved level.
    pub level: f64,
    /// The mastery rank the upgrade needs.
    pub required_level: u32,
    /// A locked upgrade gains nothing until the mastery rank reaches it.
    pub locked: bool,
    /// `PERLV[t]`: the bonus one level adds before scaling.
    pub per_lv: f64,
    /// `scalingFactor(t)`: what the upgrade scales with on this save.
    pub scaling: f64,
    /// `bonusTotal(t)`: the upgrade's current bonus, as a percentage.
    pub bonus: f64,
    /// `expMultiplier(t)`: `1 + bonus / 100`.
    pub multiplier: f64,
    /// The bonus one more level would give, locked or not.
    pub next_bonus: f64,
    /// The rate with one point spent here; the current rate when locked.
    pub rate_after: f64,
    /// `rate_after − exp_rate()`.
    pub gain: f64,
    /// The gain as a percentage of the current rate.
    pub gain_pct: f64,
}

impl Mastery {
    /// The bonus one level of an upgrade adds before scaling (`PERLV[t]`).
    pub fn per_lv(&self, index: usize) -> f64 {
        self.per_lv.get(index).copied().unwrap_or(0.0)
    }

    /// `scalingFactor(t)`: what the upgrade's level multiplies.
    pub fn scaling(&self, index: usize) -> f64 {
        match index {
            0 => idleon_common::curve::log(self.ladles.max(1.0)),
            1 => (self.cooking_levels - 1000.0).max(0.0),
            2 => (self.divorce_cake - 75.0).max(0.0),
            4 => self.ribbons,
            5 => self.level + 1.0,
            _ => 0.0,
        }
    }

    /// `bonusTotal(t, level)`: the upgrade's bonus as a percentage.
    pub fn bonus(&self, index: usize, level: f64) -> f64 {
        self.per_lv(index) * level * self.scaling(index)
    }

    /// `expMultiplier(t, level)`: the factor the upgrade applies to the rate.
    pub fn multiplier(&self, index: usize, level: f64) -> f64 {
        1.0 + self.bonus(index, level) / 100.0
    }

    /// Whether the mastery rank unlocks an upgrade.
    pub fn unlocked(&self, index: usize) -> bool {
        self.level >= f64::from(REQUIRED_LEVEL[index])
    }

    /// `computeExpRate`: the current Mastery EXP/hour.
    pub fn exp_rate(&self) -> f64 {
        self.rate_at(&self.upgrades, self.level)
    }

    /// The rate with one upgrade at `level`, every other upgrade as saved.
    fn exp_rate_with(&self, index: usize, level: f64) -> f64 {
        let mut levels = self.upgrades;
        levels[index] = level;
        self.rate_at(&levels, self.level)
    }

    /// `expReq`: the EXP one mastery level takes.
    pub fn exp_required(&self) -> f64 {
        100.0 * 2.5f64.powf(self.level) * 5f64.powf((self.level - 40.0).max(0.0))
    }

    /// `totalPointsSpent`: every mastery point the upgrades hold.
    pub fn points_spent(&self) -> f64 {
        self.upgrades.iter().sum()
    }

    /// The points the rank and the Rift Spooker companion grant in total.
    pub fn points_earned(&self) -> f64 {
        self.points_earned_at(self.level)
    }

    /// `pointsAvailable`: the earned points minus the ones the upgrades hold.
    pub fn points_available(&self) -> f64 {
        (self.points_earned() - self.points_spent()).max(0.0)
    }

    /// `reqExp / currentRate`; infinite while the rate is zero.
    pub fn hours_to_next_level(&self) -> f64 {
        let rate = self.exp_rate();
        if rate > 0.0 {
            self.exp_required() / rate
        } else {
            f64::INFINITY
        }
    }

    /// `findBestUpgrade`: the EXP upgrades by the gain one more point brings.
    ///
    /// Upstream sorts on the percentage gain alone, so locked upgrades share a
    /// zero gain and keep their page order. The index breaks the tie here, so
    /// the ranking cannot depend on the sort implementation.
    pub fn ranking(&self) -> Vec<Upgrade> {
        let rate = self.exp_rate();
        let mut upgrades: Vec<Upgrade> = EXP_UPGRADES
            .iter()
            .map(|index| {
                let index = *index;
                let level = self.upgrades[index];
                let locked = !self.unlocked(index);
                let rate_after = if locked {
                    rate
                } else {
                    self.exp_rate_with(index, level + 1.0)
                };
                let gain = rate_after - rate;
                Upgrade {
                    index,
                    name: UPGRADE_NAMES[index],
                    level,
                    required_level: REQUIRED_LEVEL[index],
                    locked,
                    per_lv: self.per_lv(index),
                    scaling: self.scaling(index),
                    bonus: self.bonus(index, level),
                    multiplier: self.multiplier(index, level),
                    next_bonus: self.bonus(index, level + 1.0),
                    rate_after,
                    gain,
                    gain_pct: if rate > 0.0 { gain / rate * 100.0 } else { 0.0 },
                }
            })
            .collect();
        upgrades.sort_by(|left, right| {
            right
                .gain_pct
                .total_cmp(&left.gain_pct)
                .then(left.index.cmp(&right.index))
        });
        upgrades
    }

    /// The page's `renderBreakdownTree` tree: every factor behind the rate.
    pub fn breakdown(&self) -> FormulaNode {
        breakdown::tree(self)
    }
}

#[cfg(test)]
mod tests;
