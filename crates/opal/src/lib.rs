//! The Opal Optimizer (W5): villager EXP rates, the current opal state and the
//! best way to split a total across the villagers.
//!
//! The upstream page keeps its maths inline in `opal-calc.html`, so every
//! function here names the page function it rewrites:
//!
//! - [`Opal::min_opals`] is the page's `minOpals`: the smallest opal count
//!   among the unlocked villagers, which Equal Spread scales with.
//! - [`Opal::exp_rate`] is `villagerExpRate`: the villager's Holeoz bonus, the
//!   account's base multiplier, its opals, its star multiplier, the Equal
//!   Spread factor and `1 + additive %`.
//! - [`Opal::total_exp_rate`] is `totalExpRate`: the unlocked villagers' rates
//!   added together.
//! - [`Opal::plan`] is the page's `optimizeTotal` and `optimizeForTarget`: the
//!   distribution of a total that earns the most for the chosen target. The port
//!   searches every placement the page's two greedy shapes can miss, so its plan
//!   is never worse than the saved distribution.
//! - [`Opal::simulation`] is the page's `recalcSim`: one hand-typed distribution
//!   read against the save, with the total it spends and the change it makes.
//! - [`Terms`] is the page's `_breakdown` object: every account bonus
//!   `buildExt` reads, kept apart so the tree can show where each came from.
//!   [`Terms::additive`] sums its additive terms and [`Terms::base_multiplier`]
//!   multiplies its factors, exactly as `buildExt` does.
//!
//! The save-derived values are read by the `idleon-account` crate, which owns
//! the field names; this crate only does arithmetic, so its tests need no save.

mod breakdown;
pub mod data;
mod optimize;
mod simulate;

#[cfg(test)]
mod tests;

pub use optimize::{Input as PlanInput, Plan, Target as PlanTarget};
pub use simulate::Simulation;

use idleon_common::formula::{FormulaNode, ValueFormat};

/// How many villagers the game has (`MAX_VILLAGERS`).
pub const VILLAGERS: usize = 5;

/// The villager names, in save order (`VILLAGER_NAMES`).
pub const NAMES: [&str; VILLAGERS] = ["Explorer", "Engineer", "Conjuror", "Measurer", "Librarian"];

/// One villager's saved state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Villager {
    /// `Holes[1][v]`: the villager's level; below 1 the villager is locked.
    pub level: f64,
    /// `Holes[3][v]`: the opals invested in the villager.
    pub opals: f64,
    /// `Holes[23][v]`: the villager's stars; each star doubles its EXP.
    pub stars: f64,
}

/// Every account bonus `buildExt` reads, as `_breakdown` reports it.
///
/// A percentage unless the name says otherwise; [`Self::chain_factors`] and
/// [`Self::additive_terms`] pair each with the label the page's tree shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Terms {
    /// `100 + B_UPG(0, 25)`: the villager EXP base, 125 with Opal Dividends.
    pub base_plus_upg: f64,
    /// `computeExoticBonus(51)`: the Grapevine exotic crop.
    pub exotic: f64,
    /// `arcadeBonus(38)`: the arcade's villager EXP upgrade.
    pub arcade: f64,
    /// `grimoireUpgBonus(29)`: Villager Extracurricular.
    pub grimoire: f64,
    /// `arcaneUpgBonus(32)`: Villager Indoctrination.
    pub arcane: f64,
    /// `MonumentROGbonuses(0, 3)`: Bravery's villager EXP.
    pub mon_bravery: f64,
    /// `MonumentROGbonuses(1, 3)`: Justice's villager EXP.
    pub mon_justice: f64,
    /// `MonumentROGbonuses(2, 3)`: Wisdom's villager EXP.
    pub mon_wisdom: f64,
    /// `_measBonus(7)`: the Slab Items measurement.
    pub meas_slab: f64,
    /// `_measBonus(0)`: the Tome Score measurement.
    pub meas_tome: f64,
    /// `B_UPG(48, 0)`: the Gloomie Opie schematic.
    pub b_upg48: f64,
    /// `cardLv('caveB') × 4`, capped at 50: the Gloomie Mushroom card.
    pub card_cave_b: f64,
    /// `bellBonus(1)`: the bell's all-villager EXP.
    pub bell1: f64,
    /// `computeWinBonus(21)`: the summoning sigil.
    pub summon_win21: f64,
    /// `vaultUpgBonus(73)`: Smarter Villagers.
    pub vault73: f64,
    /// `cosmoBonus(1, 1)`: Contented Creator, per schematic.
    pub cosmo_1_1: f64,
    /// `getCosSchBlt()`: how many schematics the save has created.
    pub scholars: f64,
    /// `cosmoBonus(1, 2)`: Cosmo, Enhance!
    pub cosmo_1_2: f64,
    /// `computeCompassBonus(59)`: Villagerz Learnz.
    pub compass59: f64,
    /// `legendPTSbonus(12)`: Opa Opal Style.
    pub legend12: f64,
    /// `pristineBon(21)`: the Candy Cache pristine charm.
    pub pristine21: f64,
    /// `companions(13)`: the Samurai Guardian's bonus, applied twice.
    pub companion13: f64,
    /// `_statueBonusGiven(28)`: the Villager statue.
    pub statue28: f64,
    /// The six Villager EXP jar collectibles, each scaled by Legend 29.
    pub jar_sum: f64,
    /// `eventShopOwned(6)`: Village Encouragement.
    pub event_shop6: f64,
    /// `_loreEpiBon(1)`: the Villager EXP lore episode.
    pub lore_episode1: f64,
    /// `bundlesData.bun_u`: the clothing bundle.
    pub bundle_u: f64,
}

/// The multiplicative chain's children, in the page's tree order.
pub const CHAIN: [(&str, &str); 9] = [
    ("Villagerz Learnz (Compass)", "Compass[0][59]"),
    ("Opa Opal Style (Legend Talent)", "Spelunk[18][12]"),
    ("Candy Cache (Pristine Charm)", "Ninja[107][21]"),
    ("Samurai Guardian (Companion)", "companion 13"),
    ("Villager Statue", "StatueLevels_0[28]"),
    (
        "Villager EXP Jar Collectibles",
        "Holes[24][4, 10, 12, 22, 29, 35]",
    ),
    (
        "Village Encouragement (Event Shop)",
        "OptLacc[379] · event shop 6",
    ),
    (
        "Lore Episode: Villager EXP",
        "Spelunk[13][2] · Spelunky[21][1]",
    ),
    ("Clothing Bundle", "BundlesReceived.bun_u"),
];

impl Terms {
    /// The additive percentage `buildExt` sums, without a villager's Opal
    /// Enthusiasm (which depends on that villager's opals).
    pub fn additive(&self) -> f64 {
        self.mon_bravery
            + self.mon_justice
            + self.mon_wisdom
            + self.meas_slab
            + self.cosmo_1_1 * self.scholars
            + self.cosmo_1_2
            + self.b_upg48
            + self.card_cave_b
            + self.bell1
            + self.meas_tome
            + self.summon_win21
            + self.vault73
    }

    /// The twelve additive terms and their tree labels, in page order.
    pub fn additive_terms(&self) -> [(&'static str, f64); 12] {
        [
            ("Bravery: Villager EXP", self.mon_bravery),
            ("Justice: Villager EXP", self.mon_justice),
            ("Wisdom: Villager EXP", self.mon_wisdom),
            ("Villager EXP Measurement (Slab Items)", self.meas_slab),
            ("Villager EXP Measurement (Tome Score)", self.meas_tome),
            ("Contented Creator", self.cosmo_1_1 * self.scholars),
            ("Cosmo, Enhance!", self.cosmo_1_2),
            ("Gloomie Opie", self.b_upg48),
            ("Gloomie Mushroom Card", self.card_cave_b),
            ("Bell: All Villager EXP", self.bell1),
            ("Sigil Bonus (Summoning)", self.summon_win21),
            ("Smarter Villagers (Vault)", self.vault73),
        ]
    }

    /// The multiplicative chain's factors, one per [`CHAIN`] entry.
    pub fn chain_factors(&self) -> [f64; 9] {
        [
            1.0 + self.compass59 / 100.0,
            1.0 + self.legend12 / 100.0,
            1.0 + self.pristine21 / 100.0,
            1.0 + 2.0 * self.companion13,
            1.0 + self.statue28 / 100.0,
            1.0 + self.jar_sum / 100.0,
            1.0 + 25.0 * self.event_shop6 / 100.0,
            1.0 + self.lore_episode1 / 100.0,
            1.0 + 50.0 * self.bundle_u / 100.0,
        ]
    }

    /// `multiChain`: the chain's factors multiplied, floored at 1.
    pub fn chain(&self) -> f64 {
        self.chain_factors().iter().product::<f64>().max(1.0)
    }

    /// The four single-factor multipliers outside the chain, in page order.
    pub fn outer_factors(&self) -> [(&'static str, f64); 4] {
        [
            ("Grapevine (Exotic Crop)", 1.0 + self.exotic / 100.0),
            ("Villager XP Multiplier (Arcade)", 1.0 + self.arcade / 100.0),
            (
                "Villager Extracurricular (Grimoire)",
                1.0 + self.grimoire / 100.0,
            ),
            (
                "Villager Indoctrination (Arcane)",
                1.0 + self.arcane / 100.0,
            ),
        ]
    }

    /// `baseMulti`: the base, the chain and the four outer factors multiplied.
    pub fn base_multiplier(&self) -> f64 {
        self.base_plus_upg
            * self.chain()
            * self
                .outer_factors()
                .iter()
                .map(|(_, factor)| *factor)
                .product::<f64>()
    }
}

/// `buildExt`'s per-villager state: the star and Holeoz multipliers the EXP
/// rate applies, and every account term behind them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ext {
    /// `starMul`: `1 + stars` for each villager.
    pub star_mul: [f64; VILLAGERS],
    /// `cosmoBonus_1_0`: Opal Enthusiasm, per ten opals.
    pub enthusiasm: f64,
    /// `cosmoBonus_1_5`: Equal Spread, per five opals of the lowest villager.
    pub equal_spread: f64,
    /// `holeozArr`: the Holeoz EXP multiplier, floored at 1.
    pub holeoz: [f64; VILLAGERS],
    /// The account bonuses the rate applies.
    pub terms: Terms,
}

impl Ext {
    /// `baseMulti`, the account multiplier every villager's rate multiplies.
    pub fn base_multiplier(&self) -> f64 {
        self.terms.base_multiplier()
    }

    /// `additiveBase`, the constant part of the additive percentage.
    pub fn additive_base(&self) -> f64 {
        self.terms.additive()
    }
}

/// One save's villagers and the account bonuses their EXP rates apply.
#[derive(Clone, Debug, PartialEq)]
pub struct Opal {
    /// The five villagers, in save order.
    pub villagers: [Villager; VILLAGERS],
    /// `Holes[4]` and `Holes[5]`: the tier 0 and tier 1 cosmo levels.
    pub cosmo: [Vec<f64>; 2],
    /// `buildExt`'s per-villager and account state.
    pub ext: Ext,
}

impl Opal {
    /// Whether a villager is unlocked: a level of at least one.
    pub fn unlocked(&self, villager: usize) -> bool {
        self.villagers
            .get(villager)
            .is_some_and(|state| state.level >= 1.0)
    }

    /// `numUnlocked`: how many villagers the save has unlocked.
    pub fn unlocked_count(&self) -> usize {
        (0..VILLAGERS)
            .filter(|villager| self.unlocked(*villager))
            .count()
    }

    /// The saved opal counts, in villager order.
    pub fn opals(&self) -> [f64; VILLAGERS] {
        self.villagers.map(|villager| villager.opals)
    }

    /// `totalOpals`: every villager's invested opals added up.
    pub fn total_opals(&self) -> f64 {
        self.opals().iter().sum()
    }

    /// `minOpals`: the smallest count among the unlocked villagers.
    pub fn min_opals(&self, opals: &[f64; VILLAGERS]) -> f64 {
        let min = (0..VILLAGERS)
            .filter(|villager| self.unlocked(*villager))
            .map(|villager| opals[villager])
            .fold(f64::INFINITY, f64::min);
        if min.is_finite() { min } else { 0.0 }
    }

    /// `equalSpread`: Equal Spread's percentage at these opal counts.
    pub fn equal_spread(&self, opals: &[f64; VILLAGERS]) -> f64 {
        self.ext.equal_spread * (self.min_opals(opals) / 5.0).floor()
    }

    /// `equalSpreadMul`: `1 + equalSpread / 100`.
    pub fn equal_spread_mul(&self, opals: &[f64; VILLAGERS]) -> f64 {
        1.0 + self.equal_spread(opals) / 100.0
    }

    /// `opalEnthusiasm`: the villager's own per-ten-opals bonus.
    pub fn enthusiasm(&self, villager: usize, opals: &[f64; VILLAGERS]) -> f64 {
        (opals.get(villager).copied().unwrap_or(0.0) / 10.0).floor() * self.ext.enthusiasm
    }

    /// `addPct`: the additive percentage the villager's rate multiplies by.
    pub fn additive_pct(&self, villager: usize, opals: &[f64; VILLAGERS]) -> f64 {
        self.ext.additive_base() + self.enthusiasm(villager, opals)
    }

    /// `villagerExpRate`: one villager's EXP per hour.
    ///
    /// A villager with no opals invested earns nothing, exactly as the page
    /// returns zero before it reads any factor.
    pub fn exp_rate(&self, villager: usize, opals: &[f64; VILLAGERS]) -> f64 {
        let Some(count) = opals.get(villager).copied() else {
            return 0.0;
        };
        if count <= 0.0 || villager >= VILLAGERS {
            return 0.0;
        }
        self.ext.holeoz[villager]
            * self.ext.base_multiplier()
            * count
            * self.ext.star_mul[villager]
            * self.equal_spread_mul(opals)
            * (1.0 + self.additive_pct(villager, opals) / 100.0)
    }

    /// `totalExpRate`: the unlocked villagers' rates added up.
    pub fn total_exp_rate(&self, opals: &[f64; VILLAGERS]) -> f64 {
        (0..VILLAGERS)
            .filter(|villager| self.unlocked(*villager))
            .map(|villager| self.exp_rate(villager, opals))
            .sum()
    }

    /// The villager's share of the total rate, or zero when nothing earns.
    pub fn share(&self, villager: usize) -> f64 {
        let total = self.total_exp_rate(&self.opals());
        if total <= 0.0 {
            return 0.0;
        }
        self.exp_rate(villager, &self.opals()) / total
    }

    /// `Holes[4 + tier][index]`: one cosmo upgrade's level.
    pub fn cosmo_level(&self, tier: usize, index: usize) -> f64 {
        self.cosmo
            .get(tier)
            .and_then(|row| row.get(index))
            .copied()
            .unwrap_or(0.0)
    }

    /// `cosmoBonus(tier, i)`: `floor(base × level)` for one cosmo upgrade.
    pub fn cosmo_bonus(&self, tier: usize, index: usize) -> f64 {
        (data::cosmo_base(tier, index) * self.cosmo_level(tier, index)).floor()
    }

    /// The shared formula tree for the current state.
    ///
    /// `selected` is `None` for the total across every unlocked villager, or
    /// one villager to break down on its own.
    pub fn breakdown(&self, selected: Option<usize>) -> FormulaNode {
        breakdown::tree(self, selected)
    }
}

/// The format every rate node reads as.
pub(crate) const RATE: ValueFormat = ValueFormat::Rate;

/// A whole opal count: floored at zero, and zero for a value that is not a
/// number. The page reads every opal input through `Math.max(0, Number(v))`.
pub fn whole(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0).floor()
    } else {
        0.0
    }
}

/// The relative change from `before` to `after`, or `None` when nothing earned
/// before: a share of nothing is no improvement to report.
pub(crate) fn relative(before: f64, after: f64) -> Option<f64> {
    (before.is_finite() && before > 0.0).then(|| (after - before) / before)
}

/// The villager names, in save order (`VILLAGER_NAMES`), for the tree's labels.
pub(crate) fn name(villager: usize) -> &'static str {
    NAMES.get(villager).copied().unwrap_or("Villager")
}
