//! The upgrades tab: the jar schematics, the jar-type costs and the plan for the
//! next type the rupies can pay for.
//!
//! Ported from `B_UPG_DATA`, `B_UPG_GROUPS`, `newJarCost`,
//! `newJarCurrencyType`, `canAffordNewJar` and `doublerTierValue` in
//! `js/stats/systems/w5/jar.js` and the `_renderUpgrades` tables.

use super::{Jar, RUPIE_NAMES, TYPE_NAMES, data};

/// Six gilded tiers, as `_renderUpgrades` lists them.
pub const GILDED_TIERS: u32 = 6;

/// One `B_UPG_DATA` schematic at its saved ownership.
#[derive(Clone, Debug, PartialEq)]
pub struct Schematic {
    pub index: usize,
    pub group: &'static str,
    pub name: &'static str,
    pub effect: f64,
    pub description: &'static str,
    /// The rupie index the schematic costs; meaningful when `cost` is non-zero.
    pub currency: usize,
    pub cost: f64,
    /// `bupgOwned`: `Holes[13][index] >= 1`.
    pub owned: bool,
}

/// How an upgrade step stands against the balances the save carries.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpgradeStatus {
    /// The jar type is already bought.
    Owned,
    /// The next type and the rupie it costs can pay for it.
    Affordable,
    /// The next type costs more than the rupie balance covers.
    Shortfall,
    /// A later type, which cannot be bought before the next one.
    Locked,
}

impl UpgradeStatus {
    /// The word the upgrade table shows.
    pub fn label(self) -> &'static str {
        match self {
            Self::Owned => "Owned",
            Self::Affordable => "Affordable",
            Self::Shortfall => "Short by",
            Self::Locked => "Locked",
        }
    }
}

/// One jar type's purchase.
#[derive(Clone, Debug, PartialEq)]
pub struct JarTypeUpgrade {
    /// The type this purchase unlocks, `1..=9`.
    pub jar_type: usize,
    pub name: &'static str,
    pub cost: f64,
    /// The rupie the purchase is paid with (`jar_type - 1`).
    pub currency: usize,
    pub currency_name: &'static str,
    /// The balance of that rupie in this save.
    pub balance: f64,
    pub status: UpgradeStatus,
}

impl JarTypeUpgrade {
    /// What the balance misses when the step is not affordable.
    pub fn shortfall(&self) -> f64 {
        (self.cost - self.balance).max(0.0)
    }

    /// Whether the step can be bought without exceeding the balance.
    pub fn affordable(&self) -> bool {
        self.status == UpgradeStatus::Affordable
    }
}

/// The nine jar-type purchases and the one the plan can act on.
#[derive(Clone, Debug, PartialEq)]
pub struct UpgradePlan {
    pub steps: Vec<JarTypeUpgrade>,
}

impl UpgradePlan {
    /// The only step that can be bought now, if any.
    pub fn next(&self) -> Option<&JarTypeUpgrade> {
        self.steps.iter().find(|step| {
            matches!(
                step.status,
                UpgradeStatus::Affordable | UpgradeStatus::Shortfall
            )
        })
    }

    /// The balance the next step spends from.
    pub fn budget(&self) -> f64 {
        self.next().map(|step| step.balance).unwrap_or(0.0)
    }

    /// What the plan can spend now: the next cost only when it fits.
    pub fn spent(&self) -> f64 {
        self.next()
            .filter(|step| step.affordable())
            .map(|step| step.cost)
            .unwrap_or(0.0)
    }

    /// The steps the plan would buy with the current balance.
    pub fn purchases(&self) -> impl Iterator<Item = &JarTypeUpgrade> {
        self.steps.iter().filter(|step| step.affordable())
    }
}

impl Jar {
    /// The schematics in the upstream group order.
    pub fn schematics(&self) -> Vec<Schematic> {
        let mut rows = Vec::new();
        for group in &data().schematic_groups {
            for index in &group.indices {
                let Some(row) = data().schematic(*index) else {
                    continue;
                };
                rows.push(Schematic {
                    index: row.index,
                    group: group.name.as_str(),
                    name: row.name.as_str(),
                    effect: row.effect,
                    description: row.description.as_str(),
                    currency: row.currency,
                    cost: row.cost,
                    owned: self.upgrade(row.index),
                });
            }
        }
        rows
    }

    /// `newJarCost` for the purchase that unlocks type `jar_type` (`1..=9`).
    pub fn jar_type_cost(jar_type: usize) -> f64 {
        let owned = jar_type.saturating_sub(1) as f64;
        50.0 * (1.0 + owned).powf(1.5) * 4.8_f64.powf(owned)
    }

    /// `upgradePlan`: the nine types in purchase order, each checked against
    /// the rupie balance the game pays it with.
    pub fn upgrade_plan(&self) -> UpgradePlan {
        let owned = self.types_purchased();
        let steps = (1..TYPE_NAMES.len())
            .map(|jar_type| {
                let currency = jar_type - 1;
                let cost = Self::jar_type_cost(jar_type);
                let balance = self.rupies(currency);
                let status = if jar_type <= owned {
                    UpgradeStatus::Owned
                } else if jar_type == owned + 1 {
                    if balance >= cost {
                        UpgradeStatus::Affordable
                    } else {
                        UpgradeStatus::Shortfall
                    }
                } else {
                    UpgradeStatus::Locked
                };
                JarTypeUpgrade {
                    jar_type,
                    name: TYPE_NAMES[jar_type],
                    cost,
                    currency,
                    currency_name: RUPIE_NAMES[currency],
                    balance,
                    status,
                }
            })
            .collect();
        UpgradePlan { steps }
    }

    /// The upgrade the plan can act on, or `None` at the final type.
    pub fn next_upgrade(&self) -> Option<JarTypeUpgrade> {
        self.upgrade_plan().next().cloned()
    }

    /// `doublerTierValue`: what a gilded tier adds to the next rupie break.
    pub fn doubler_tier_value(tier: u32) -> f64 {
        100.0 * 3_f64.powi(tier as i32)
    }

    /// `doublerMultiplier`: the gilded tier's multiplier.
    pub fn doubler_multiplier(tier: u32) -> f64 {
        1.0 + Self::doubler_tier_value(tier) / 100.0
    }
}
