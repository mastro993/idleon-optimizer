//! Clamworks (W7): pearl value, upgrade costs, upgrade priority, the AFK claim,
//! the budgeted upgrade plan and the promotion tracker.
//!
//! Ports `js/stats/systems/w7/clamworks.js` from the upstream calculators. Save
//! fields: `OptionsListAccount` (or `OptLacc`) index 454 is the pearl balance,
//! 455..=463 are the nine upgrade levels, 464 the promotion level and 465 how
//! far the pearl upgrades have unlocked; `Menagerie[5]` holds the Scurge upgrade
//! levels, `Menagerie[0][8]` the Scurge level and `Menagerie[2][4]` the unlock
//! that buys the Scurge menu.
//!
//! Multi-Scalping's bonus depends on the character's multikill total, which the
//! upstream derives from its combat simulator. Until that lands here the total
//! is an input; the formulas are the same for any value, and 0 leaves the bonus
//! at zero.
//!
//! [`afk`] ports the claim itself and the `+1` comparison beside it. [`scurge`]
//! ports the Scurge EXP panels: the per-kill EXP, the Self Affliction comparison
//! and the allocation of unspent Scurge points.

pub mod afk;
pub mod data;
pub mod plan;
pub mod promotion;
pub mod scurge;

pub use plan::{Input as PlanInput, Plan, Step as PlanStep};
pub use promotion::Promotion;

use data::clamworks;
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_save::json;
use serde_json::{Map, Value};

/// The pearl upgrades, in the game's own order.
const UPGRADE_COUNT: usize = 9;
/// The Scurge upgrades, in the game's own order.
const SCURGE_COUNT: usize = 9;

const PEARL_BALANCE: usize = 454;
const UPGRADE_BASE: usize = 455;
const PROMOTION_LEVEL: usize = 464;
const UNLOCK_PROGRESS: usize = 465;
/// Multi-Scalping (index 3) is the one upgrade whose bonus scales with multikill.
const MULTI_SCALPING: usize = 3;
/// Pure Pearls (index 5) gates the black-pearl drop rate.
const PURE_PEARLS: usize = 5;

/// The Clamworks state a save carries.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Clamworks {
    pub pearl_balance: f64,
    pub promotion_level: u32,
    pub upgrades: [u32; UPGRADE_COUNT],
    /// How many pearl upgrades the save has unlocked, `OptLacc[465]`.
    pub unlocked: u32,
    /// `Menagerie[2][4]`: whether the Scurge menu is bought.
    pub scurge_unlocked: bool,
    pub scurge_level: u32,
    pub scurge_upgrades: [u32; SCURGE_COUNT],
}

/// The factors of [`Clamworks::pearl_value`], one per upstream breakdown node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PearlValue {
    /// PEARL VALUE: `1 + bonus(0)`.
    pub base: f64,
    /// MULTI-SCALPING: `1 + bonus(3) / 100`.
    pub multikill: f64,
    /// SHINIER PEARLS: `1 + bonus(7) / 100`.
    pub shinier: f64,
    /// Scurge 1 and Scurge 8 at the saved Scurge level.
    pub scurge: f64,
}

impl PearlValue {
    /// The pearl value the factors multiply to.
    pub fn product(&self) -> f64 {
        self.base * self.multikill * self.shinier * self.scurge
    }
}

/// What an upgrade changes, as the upstream priority table groups it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    /// Pearl value, 10x chance or drop rate: ranked by pearls per kill.
    Pearl,
    /// Clam count: more kills, not more pearls per kill.
    Clams,
    /// Pure pearl value.
    Black,
    /// FRUGALITY and ANTI INFLATION: cheaper upgrades.
    Cost,
}

impl Category {
    fn of(index: usize) -> Self {
        match index {
            4 | 8 => Self::Cost,
            1 => Self::Clams,
            5 => Self::Black,
            _ => Self::Pearl,
        }
    }
}

/// One row of the upstream upgrade priority table.
#[derive(Clone, Copy, Debug)]
pub struct UpgradeRank {
    pub index: usize,
    pub level: u32,
    pub next_cost: f64,
    /// Fractional gain in pearls per kill from one more level.
    pub ppk_gain: f64,
    /// Fractional saving on upgrade costs from one more level.
    pub cost_save: f64,
    /// `ppk_gain / next_cost`.
    pub efficiency: f64,
    pub category: Category,
    /// Whether the level moves pearls per kill, as the upstream table filters.
    pub is_pearl_upgrade: bool,
}

impl Clamworks {
    /// Read the Clamworks fields a save carries. A save without an options list
    /// keeps the pearl state at zero and reports it; the Scurge half reads
    /// `Menagerie` on its own and names every row it cannot find.
    pub fn from_save(save: &Map<String, Value>, warnings: &mut Vec<String>) -> Self {
        let mut clamworks = Self::default();
        match options(save) {
            None => {
                warnings.push("OptLacc: not in the save; Clamworks values kept at zero".into());
            }
            Some(options) => {
                let number = |index: usize| json::index(&options, &[index]).and_then(json::number);
                let saved_level = |index: usize| number(index).map(whole_level).unwrap_or(0);
                clamworks.pearl_balance = number(PEARL_BALANCE).unwrap_or(0.0);
                clamworks.promotion_level = saved_level(PROMOTION_LEVEL);
                clamworks.upgrades = std::array::from_fn(|index| saved_level(UPGRADE_BASE + index));
                clamworks.unlocked = saved_level(UNLOCK_PROGRESS);
            }
        }
        clamworks.read_scurge(save, warnings);
        clamworks
    }

    /// `Menagerie`: `[0][8]` the Scurge level, `[2][4]` the Scurge unlock and
    /// `[5]` the nine Scurge upgrade levels.
    fn read_scurge(&mut self, save: &Map<String, Value>, warnings: &mut Vec<String>) {
        let Some(menagerie) = json::parsed_field(save, "Menagerie").filter(Value::is_array) else {
            warnings.push("Menagerie: not in the save; Scurge progress reads as zero".into());
            return;
        };
        let number = |path: &[usize]| json::index(&menagerie, path).and_then(json::number);
        match number(&[0, 8]) {
            Some(level) => self.scurge_level = whole_level(level),
            None => warnings
                .push("Menagerie[0][8]: Scurge level not in the save; it reads as zero".into()),
        }
        match number(&[2, 4]) {
            Some(unlock) => self.scurge_unlocked = unlock >= 1.0,
            None => warnings.push(
                "Menagerie[2][4]: Scurge unlock not in the save; Scurge reads as locked".into(),
            ),
        }
        if !json::index(&menagerie, &[5]).is_some_and(Value::is_array) {
            warnings.push(
                "Menagerie[5]: Scurge upgrade levels not in the save; they read as zero".into(),
            );
            return;
        }
        for index in 0..SCURGE_COUNT {
            self.scurge_upgrades[index] = number(&[5, index]).map(whole_level).unwrap_or(0);
        }
    }

    /// The raw bonus of one upgrade at its saved level.
    pub fn upgrade_bonus(&self, index: usize, multikill_total: f64) -> f64 {
        if index == MULTI_SCALPING {
            self.multikill_bonus(multikill_total)
        } else {
            per_lv(index) * f64::from(self.upgrades[index])
        }
    }

    /// `ClamBonuses(3)`: `MK / 1000 × perLv × level`.
    pub fn multikill_bonus(&self, multikill_total: f64) -> f64 {
        multikill_total.max(0.0) / 1000.0
            * per_lv(MULTI_SCALPING)
            * f64::from(self.upgrades[MULTI_SCALPING])
    }

    /// Clams on the map: `min(25, 2 + level of CLAM COMRADES)`.
    pub fn clam_count(&self) -> u32 {
        (2.0 + f64::from(self.upgrades[1])).min(25.0) as u32
    }

    /// Clam HP at the saved promotion level: `1e16 × 30^lv`.
    pub fn clam_hp(&self) -> f64 {
        self.clam_hp_at(self.promotion_level)
    }

    /// Clam HP at a promotion level: `1e16 × 30^lv`.
    pub fn clam_hp_at(&self, level: u32) -> f64 {
        1e16 * 30f64.powf(f64::from(level))
    }

    /// The pearl value factors, in upstream display order.
    pub fn pearl_value_breakdown(&self, multikill_total: f64) -> PearlValue {
        PearlValue {
            base: 1.0 + self.upgrade_bonus(0, multikill_total),
            multikill: 1.0 + self.multikill_bonus(multikill_total) / 100.0,
            shinier: 1.0 + self.upgrade_bonus(7, multikill_total) / 100.0,
            scurge: self.scurge_pearl_multi(),
        }
    }

    /// `pearlValue`: the product of the breakdown factors.
    pub fn pearl_value(&self, multikill_total: f64) -> f64 {
        self.pearl_value_breakdown(multikill_total).product()
    }

    /// The pearl value as the shared formula tree: the root is the product and
    /// each child is one upstream breakdown factor.
    pub fn pearl_value_tree(&self, multikill_total: f64) -> FormulaNode {
        let value = self.pearl_value(multikill_total);
        let bonus_0 = self.upgrade_bonus(0, multikill_total);
        let bonus_3 = self.multikill_bonus(multikill_total);
        let bonus_7 = self.upgrade_bonus(7, multikill_total);
        let scurge_1 = self.scurge_bonus(1);
        let scurge_8 = self.scurge_bonus(8);
        FormulaNode::new("Pearl Value", value)
            .with_format(ValueFormat::Multiplier)
            .with_input("PEARL VALUE × MULTI-SCALPING × SHINIER PEARLS × Scurge")
            .with_multiplier(value)
            .with_source("clamworks.js · pearlValue")
            .with_note("What one regular pearl is worth; the factors multiply.")
            .with_children([
                FormulaNode::leaf("PEARL VALUE", 1.0 + bonus_0, ValueFormat::Multiplier)
                    .with_input(format!("1 + {bonus_0}% bonus"))
                    .with_source(format!("PEARL VALUE Lv {}", self.upgrades[0])),
                FormulaNode::leaf(
                    "MULTI-SCALPING",
                    1.0 + bonus_3 / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_input(format!(
                    "{multikill_total}% multikill × {}% × Lv {} = +{bonus_3}%",
                    per_lv(MULTI_SCALPING),
                    self.upgrades[MULTI_SCALPING]
                ))
                .with_source(format!(
                    "MULTI-SCALPING Lv {}",
                    self.upgrades[MULTI_SCALPING]
                )),
                FormulaNode::leaf(
                    "SHINIER PEARLS",
                    1.0 + bonus_7 / 100.0,
                    ValueFormat::Multiplier,
                )
                .with_input(format!("+{bonus_7}%"))
                .with_source(format!("SHINIER PEARLS Lv {}", self.upgrades[7])),
                FormulaNode::leaf(
                    "Scurge pearl value",
                    self.scurge_pearl_multi(),
                    ValueFormat::Multiplier,
                )
                .with_input(format!(
                    "Scurge 1 +{scurge_1}%; Scurge 8 +{scurge_8}% × Lv {}",
                    self.scurge_level
                ))
                .with_source("Scurge upgrades 1 and 8"),
            ])
    }

    /// Chance a pearl is worth ten times as much.
    pub fn pearl_10x_chance(&self) -> f64 {
        1.0 - 1.0 / (1.0 + self.upgrade_bonus(2, 0.0) / 100.0)
    }

    /// Expected multiplier from the 10x chance.
    pub fn pearl_10x_expected(&self) -> f64 {
        let chance = self.pearl_10x_chance();
        1.0 * (1.0 - chance) + 10.0 * chance
    }

    /// What a pure pearl is worth, in normal pearls.
    pub fn black_pearl_value(&self) -> f64 {
        50.0 + self.upgrade_bonus(PURE_PEARLS, 0.0)
    }

    /// Pearls per drop including the 10x chance.
    pub fn effective_pearls_per_drop(&self, multikill_total: f64) -> f64 {
        self.pearl_value(multikill_total) * self.pearl_10x_expected()
    }

    /// The Scurge multiplier on pearl value.
    pub fn scurge_pearl_multi(&self) -> f64 {
        (1.0 + self.scurge_bonus(1) / 100.0)
            * (1.0 + self.scurge_bonus(8) * self.scurge_level as f64 / 100.0)
    }

    /// A Scurge upgrade's raw bonus: `perLv × level`.
    pub fn scurge_bonus(&self, index: usize) -> f64 {
        scurge_per_lv(index) * f64::from(self.scurge_upgrades[index])
    }

    /// FRUGALITY and ANTI INFLATION together, without a target upgrade.
    pub fn cost_reduction(&self) -> f64 {
        self.frugality() * self.anti_inflation()
    }

    /// The cost multiplier when buying `index`: FRUGALITY, Scurge 3, this
    /// upgrade's Scurge 5 scaling and ANTI INFLATION, in upstream order.
    pub fn cost_reduction_at(&self, index: usize) -> f64 {
        let scurge_3 = self.scurge_bonus(3);
        let scurge_5 = self.scurge_bonus(5);
        self.frugality()
            * (1.0 / (1.0 + scurge_3 / 100.0))
            * (1.0 / (1.0 + f64::from(self.scurge_upgrades[index]) * scurge_5 / 100.0))
            * self.anti_inflation()
    }

    /// FRUGALITY's cost factor: `1 / (1 + bonus(4) / 100)`.
    fn frugality(&self) -> f64 {
        1.0 / (1.0 + self.upgrade_bonus(4, 0.0) / 100.0)
    }

    /// ANTI INFLATION's cost factor: `1 / (1 + bonus(8) / 100)`.
    fn anti_inflation(&self) -> f64 {
        1.0 / (1.0 + self.upgrade_bonus(8, 0.0) / 100.0)
    }

    /// `Clam_Cost(t)`: what level `level` of upgrade `index` costs.
    pub fn upgrade_cost_at(&self, index: usize, level: f64) -> f64 {
        let reduction = self.cost_reduction_at(index);
        if index == 0 {
            return reduction * (cost_base(0).powf(level) + 3.0 * level + level.powf(2.5));
        }
        let promotion = 10.0 + 3.0 * f64::from(self.promotion_level);
        reduction
            * (20.0 * promotion.powf(index as f64 - 1.0) / 5.0 * cost_base(index).powf(level)
                + 2.0 * level
                + level.powf(1.5))
    }

    /// What the next level of `index` costs.
    pub fn next_upgrade_cost(&self, index: usize) -> f64 {
        self.upgrade_cost_at(index, f64::from(self.upgrades[index]) + 1.0)
    }

    /// Pearls a promotion attempt costs.
    pub fn promotion_cost(&self) -> f64 {
        1e5 * 10f64.powf(f64::from(self.promotion_level))
    }

    /// Chance a promotion attempt succeeds: `0.5 / (2 + lv)`.
    pub fn promotion_chance(&self) -> f64 {
        0.5 / (2.0 + f64::from(self.promotion_level))
    }

    /// The drop-rate penalty each promotion level adds: `0.66^lv`.
    pub fn promo_penalty(&self) -> f64 {
        0.66f64.powf(f64::from(self.promotion_level))
    }

    /// Regular pearl drops per kill.
    pub fn pearl_drop_rate(&self) -> f64 {
        0.005 * (1.0 + self.upgrade_bonus(6, 0.0) / 100.0) * self.promo_penalty()
    }

    /// Pure pearl drops per kill; zero until PURE PEARLS is bought.
    pub fn black_pearl_drop_rate(&self) -> f64 {
        if self.upgrades[PURE_PEARLS] < 1 {
            return 0.0;
        }
        0.000_025 * (1.0 + self.upgrade_bonus(6, 0.0) / 100.0) * self.promo_penalty()
    }

    /// Average pearls per kill: regular and pure pearls together.
    pub fn pearls_per_kill(&self, multikill_total: f64) -> f64 {
        self.pearl_drop_rate() * self.effective_pearls_per_drop(multikill_total)
            + self.black_pearl_drop_rate() * self.black_pearl_value()
    }

    /// The upstream upgrade ranking: pearls per kill gain per unit cost,
    /// highest first. Ties keep the game's upgrade order.
    pub fn priority(&self, multikill_total: f64) -> Vec<UpgradeRank> {
        let mut rows: Vec<UpgradeRank> = (0..UPGRADE_COUNT)
            .map(|index| {
                let next_cost = self.next_upgrade_cost(index);
                let ppk_gain = self.pearls_per_kill_marginal_gain(index, multikill_total);
                UpgradeRank {
                    index,
                    level: self.upgrades[index],
                    next_cost,
                    ppk_gain,
                    cost_save: self.cost_reduction_savings(index),
                    efficiency: if next_cost > 0.0 {
                        ppk_gain / next_cost
                    } else {
                        0.0
                    },
                    category: Category::of(index),
                    is_pearl_upgrade: ppk_gain > 0.0,
                }
            })
            .collect();
        rows.sort_by(|a, b| b.efficiency.total_cmp(&a.efficiency));
        rows
    }

    /// A copy with one upgrade level higher.
    fn with_upgrade(&self, index: usize) -> Self {
        let mut next = self.clone();
        next.upgrades[index] = next.upgrades[index].saturating_add(1);
        next
    }

    /// Fractional pearl-value gain from one more level (0, 3 and 7 only).
    fn pearl_value_marginal_gain(&self, index: usize, multikill_total: f64) -> f64 {
        if !matches!(index, 0 | MULTI_SCALPING | 7) {
            return 0.0;
        }
        let current = self.pearl_value(multikill_total);
        if current <= 0.0 {
            return 0.0;
        }
        self.with_upgrade(index).pearl_value(multikill_total) / current - 1.0
    }

    /// Fractional expected-value gain from one more 10x chance (index 2 only).
    fn pearl_10x_marginal_gain(&self, index: usize) -> f64 {
        if index != 2 {
            return 0.0;
        }
        self.with_upgrade(index).pearl_10x_expected() / self.pearl_10x_expected() - 1.0
    }

    /// Fractional drop-rate gain from one more level (index 6 only).
    fn drop_rate_marginal_gain(&self, index: usize) -> f64 {
        if index != 6 {
            return 0.0;
        }
        self.with_upgrade(index).pearl_drop_rate() / self.pearl_drop_rate() - 1.0
    }

    /// Absolute pearls-per-kill gain from one more pure-pearl value (index 5).
    fn black_pearl_value_marginal_gain(&self, index: usize) -> f64 {
        if index != PURE_PEARLS {
            return 0.0;
        }
        let rate = self.black_pearl_drop_rate();
        if rate <= 0.0 {
            return 0.0;
        }
        rate * (self.with_upgrade(index).black_pearl_value() - self.black_pearl_value())
    }

    /// Combined fractional pearls-per-kill gain from one more level.
    fn pearls_per_kill_marginal_gain(&self, index: usize, multikill_total: f64) -> f64 {
        // Clams and the cost upgrades do not change pearls per kill.
        if matches!(index, 1 | 4 | 8) {
            return 0.0;
        }
        let current = self.pearls_per_kill(multikill_total);
        if current <= 0.0 {
            return 0.0;
        }
        let value_gain = self.pearl_value_marginal_gain(index, multikill_total);
        let tenx_gain = self.pearl_10x_marginal_gain(index);
        let drop_gain = self.drop_rate_marginal_gain(index);
        let black_gain = self.black_pearl_value_marginal_gain(index);
        let regular = self.pearl_drop_rate() * self.effective_pearls_per_drop(multikill_total);
        let new_regular = regular * (1.0 + value_gain) * (1.0 + tenx_gain) * (1.0 + drop_gain);
        let black = self.black_pearl_drop_rate() * self.black_pearl_value();
        let new_black = black * (1.0 + drop_gain) + black_gain;
        (new_regular + new_black) / current - 1.0
    }

    /// Fractional cost reduction from one more FRUGALITY or ANTI INFLATION.
    fn cost_reduction_savings(&self, index: usize) -> f64 {
        if !matches!(index, 4 | 8) {
            return 0.0;
        }
        let current = self.cost_reduction();
        let next = self.with_upgrade(index).cost_reduction();
        (current - next) / current
    }
}

/// The upstream options list: `OptionsListAccount` wins over `OptLacc`.
fn options(save: &Map<String, Value>) -> Option<Value> {
    ["OptionsListAccount", "OptLacc"]
        .into_iter()
        .find_map(|key| json::parsed_field(save, key).filter(Value::is_array))
}

/// The game's per-level value of one pearl upgrade.
fn per_lv(index: usize) -> f64 {
    clamworks().upgrades[index].per_lv
}

/// The game's cost base of one pearl upgrade.
fn cost_base(index: usize) -> f64 {
    clamworks().upgrades[index].cost_base
}

/// The game's per-level value of one Scurge upgrade.
fn scurge_per_lv(index: usize) -> f64 {
    clamworks().scurge[index].per_lv
}

/// A save value as a whole level: negative and non-finite reads as zero.
fn whole_level(value: f64) -> u32 {
    value.max(0.0).round() as u32
}
