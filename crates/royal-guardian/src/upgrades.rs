//! The upgrade list: every Armory shelf and every Orblet, what the next level
//! costs and how far the current currency goes.
//!
//! Ported from `armoryRows`, `armoryUnlockOrder`, `armoryUnlockedCount`,
//! `armoryUpgradeCost`, `armoryCostProjection`, `allMasterclassCostRedux*`,
//! `orbletRows`, `orbletCost`, `orbletUnlocked`, `orbletMax` and
//! `orbletOwnedDetail`.
//!
//! The Armory price is the product of seven factors: a base of 25, the daily
//! Masterclass reduction (the Legend talent's daily-purchase counter against the
//! Spelunk unlock, the bundle prefix and the Orblet BARGAIN reduction), the two
//! Jelly obstruction discounts for the first shelves, the order growth and
//! coefficient, the shelf's base cost and its growth at the level being priced.
//! Two shelves carry a hard-coded first-level price instead of that product.
//!
//! The upstream prices a shelf's currency slot from the row at its *order*
//! position, not at its own index. That is the game's own lookup, so the port
//! copies it and the currency column can name a different slot than the shelf's
//! own row.
//!
//! The shelf descriptions upstream prints read systems this build does not
//! decode (Hole, Guild, Star Sign and prayer bonuses among them), so the list
//! shows each shelf's own bonus instead of the templated sentence.

use super::*;
use crate::rates::Factor;

/// The first shelf's price: every Armory cost starts here.
const ARMORY_BASE: f64 = 25.0;

/// The two shelves with a fixed first-level price: upstream hard-codes both as
/// a source special case.
const SPECIAL_PRICES: [(usize, f64, f64); 2] = [(46, 3.0, 2.0), (58, 1.0, 3.0)];

/// A purchase counter past any daily limit, so the price reads at full.
const MAX_COUNTER: f64 = 9_007_199_254_740_991.0;

/// How many levels a cost projection lists when the caller does not say.
pub const PROJECTION_LEVELS: usize = 50;

/// The daily Masterclass reduction: upstream `allMasterclassCostReduxDetail`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Redux {
    /// The whole reduction, prefix times BARGAIN.
    pub value: f64,
    /// The daily-prefix reduction alone.
    pub prefix: f64,
    /// Orblet 7's BARGAIN reduction.
    pub bargain: f64,
    pub reduction_percent: f64,
    /// How many discounted purchases the save has made today.
    pub purchase_counter: f64,
    /// How many discounted purchases the Legend talent allows today.
    pub daily_limit: f64,
    pub daily_discount_active: bool,
}

impl Redux {
    /// The discounted purchases left today.
    pub fn discounts_left(&self) -> f64 {
        (self.daily_limit - self.purchase_counter).max(0.0)
    }
}

/// What one shelf costs, and the factors that price it.
#[derive(Clone, Debug, PartialEq)]
pub struct Cost {
    pub value: f64,
    pub index: usize,
    /// The level the price was read at.
    pub level: f64,
    /// The currency slot the price is paid in; `-1` when no row carries one.
    pub currency_slot: i64,
    pub factors: Vec<Factor>,
    pub missing: Vec<String>,
}

/// One Armory shelf, as the list prints it: upstream `armoryRows` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct ArmoryRow {
    /// The `ArmoryUpg` index.
    pub index: usize,
    /// Where the shelf sits in `Research[43]`.
    pub order: usize,
    pub name: &'static str,
    pub level: f64,
    pub max_level: f64,
    pub current_bonus: f64,
    pub next_bonus: f64,
    pub bonus_per_level: f64,
    pub cost: Cost,
    pub maxed: bool,
    /// Whether the shelf's Armory level total has been reached.
    pub unlocked: bool,
    /// The currency balance the shelf's price is paid from.
    pub balance: f64,
    pub affordable: bool,
}

/// One priced level in an Armory cost plan.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CostStep {
    pub level: f64,
    pub next_level: f64,
    /// The price with the daily discount applied.
    pub discounted: f64,
    /// The price once the daily discount is spent.
    pub full: f64,
    pub cumulative_discounted: f64,
    pub cumulative_full: f64,
}

/// The next levels of one shelf priced one at a time: upstream
/// `armoryCostProjection`.
#[derive(Clone, Debug, PartialEq)]
pub struct CostPlan {
    pub index: usize,
    pub order: usize,
    pub name: &'static str,
    pub level: f64,
    pub max_level: f64,
    pub currency_slot: i64,
    pub balance: f64,
    pub daily_limit: f64,
    pub purchase_counter: f64,
    pub discounts_left: f64,
    pub discount_available: bool,
    pub rows: Vec<CostStep>,
}

impl CostPlan {
    /// How many listed levels the balance covers at full price.
    pub fn covered_full(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.cumulative_full <= self.balance)
            .count()
    }

    /// How many listed levels the balance covers with the daily discount.
    pub fn covered_discounted(&self) -> usize {
        self.rows
            .iter()
            .filter(|row| row.cumulative_discounted <= self.balance)
            .count()
    }
}

/// One Orblet market row: upstream `orbletRows` entry.
#[derive(Clone, Debug, PartialEq)]
pub struct OrbletRow {
    pub index: usize,
    pub name: &'static str,
    pub level: f64,
    pub max_level: f64,
    pub bonus: f64,
    /// The bonus the next level reaches; `None` when maxed or an action row.
    pub next_bonus: Option<f64>,
    /// Orblet 4 is the Glorification action, not a passive bonus.
    pub action: bool,
    pub maxed: bool,
    /// The previous row must be bought at least once.
    pub unlocked: bool,
    pub cost: f64,
    pub owned: f64,
    /// Whether the owned Orblets cover the cost.
    pub affordable: bool,
    /// Whether the owned count could be read at all.
    pub partial: bool,
    pub missing: Vec<String>,
}

/// The Orblets the save owns: upstream `itemCount('Orblet')`.
#[derive(Clone, Debug, PartialEq)]
pub struct Owned {
    pub value: f64,
    pub partial: bool,
    pub missing: Vec<String>,
}

impl RoyalGuardian {
    /// Every Armory level the save holds, summed.
    pub fn armory_total_levels(&self) -> f64 {
        self.armory_levels()
    }

    /// The shelf order: upstream `armoryUnlockOrder`.
    pub fn armory_unlock_order(&self) -> &'static [f64] {
        data::armory_order()
    }

    /// How many shelves the Armory level total has reached: upstream
    /// `armoryUnlockedCount`.
    pub fn armory_unlocked_count(&self) -> usize {
        let total = self.armory_total_levels();
        let unlocked = data::data()
            .armory
            .iter()
            .filter(|shelf| total >= shelf.research_order)
            .count();
        unlocked.min(self.armory_unlock_order().len())
    }

    /// How many discounted purchases the Legend talent allows today:
    /// upstream `legendPTSbonus(23)`.
    pub fn daily_discount_limit(&self) -> f64 {
        (self.spelunk_cell(18, 23) * data::data().legend_talent_per_point23).round()
    }

    /// How many discounted purchases the save has made today:
    /// `OptLacc[480]`, zero when the save carries no options list.
    pub fn daily_purchase_counter(&self) -> f64 {
        self.options_cell(480)
    }

    /// One `OptLacc` cell.
    fn options_cell(&self, index: usize) -> f64 {
        self.options
            .get(index)
            .and_then(Value::as_f64)
            .unwrap_or(0.0)
    }

    /// Whether the save owns the bundle that lowers the prefix.
    fn bundle_owned(&self) -> bool {
        self.bundles.get("bon_p").is_some_and(|value| match value {
            Value::Bool(flag) => *flag,
            Value::Number(number) => number.as_f64().unwrap_or(0.0) != 0.0,
            Value::String(text) => !text.is_empty() && text != "0",
            _ => false,
        })
    }

    /// The daily Masterclass reduction: upstream `allMasterclassCostReduxDetail`
    /// at the saved purchase counter.
    pub fn masterclass_redux(&self) -> Redux {
        self.masterclass_redux_at(self.daily_purchase_counter())
    }

    /// The daily Masterclass reduction as it reads at `counter` purchased
    /// discounts, which is how a cost projection prices a spent discount.
    pub fn masterclass_redux_at(&self, counter: f64) -> Redux {
        let limit = self.daily_discount_limit();
        let bundle = self.bundle_owned();
        let prefix = if counter < limit {
            if bundle { 0.05 } else { 0.2 }
        } else if bundle {
            0.25
        } else {
            1.0
        };
        let bargain = 1.0 / (1.0 + self.orblet_bonus(7) / 100.0);
        Redux {
            value: prefix * bargain,
            prefix,
            bargain,
            reduction_percent: (1.0 - prefix * bargain) * 100.0,
            purchase_counter: counter,
            daily_limit: limit,
            daily_discount_active: counter < limit,
        }
    }

    /// What the shelf at `order` costs at its saved level: upstream
    /// `armoryUpgradeCost`.
    pub fn armory_upgrade_cost(&self, order: usize) -> Cost {
        let index = self.shelf_index(order);
        let level = self
            .armory_row(index)
            .map(|_| self.armory_level(index))
            .unwrap_or(0.0);
        self.armory_upgrade_cost_at(order, level, self.daily_purchase_counter())
    }

    /// What a shelf costs at `level` with `counter` discounts already spent:
    /// the same function the cost projection prices every level with.
    pub fn armory_upgrade_cost_at(&self, order: usize, level: f64, counter: f64) -> Cost {
        let index = self.shelf_index(order);
        // The upstream reads the currency slot from the order position, not the
        // shelf index; the port keeps that lookup.
        let currency_slot = data::data()
            .armory
            .get(order)
            .map(|shelf| shelf.currency_slot)
            .unwrap_or(-1);
        let Some(shelf) = data::armory(index) else {
            return Cost {
                value: f64::INFINITY,
                index,
                level,
                currency_slot,
                factors: Vec::new(),
                missing: vec!["armory row".to_owned()],
            };
        };
        if let Some((_, below, price)) = SPECIAL_PRICES
            .iter()
            .find(|(special, below, _)| *special == index && level < *below)
        {
            let _ = below;
            return Cost {
                value: *price,
                index,
                level,
                currency_slot,
                factors: vec![Factor {
                    name: "source special case",
                    label: "Fixed first-level price",
                    value: *price,
                }],
                missing: Vec::new(),
            };
        }
        // Upstream searches the order table for the *number* `order`, which is
        // only a hit when the shelf order happens to contain it.
        let position = self
            .armory_unlock_order()
            .iter()
            .position(|value| *value == order as f64)
            .map(|found| found as f64)
            .unwrap_or(-1.0);
        let first_five = ((5.0 - position) / 5.0).ceil().max(0.0);
        let first_ten = ((10.0 - position) / 10.0).ceil().max(0.0);
        let jelly = 1.0
            / (1.0
                + (first_five * self.jelly_bonus(11) + first_ten * self.jelly_bonus(26)) / 100.0);
        let mut factors = vec![
            Factor {
                name: "base",
                label: "Base",
                value: ARMORY_BASE,
            },
            Factor {
                name: "Masterclass reduction",
                label: "Masterclass reduction",
                value: self.masterclass_redux_at(counter).value,
            },
            Factor {
                name: "Jelly obstruction discounts",
                label: "Jelly obstruction discounts",
                value: jelly,
            },
            Factor {
                name: "order growth",
                label: "Shelf growth",
                value: 1.24_f64.powf(order as f64),
            },
            Factor {
                name: "order coefficient",
                label: "Shelf coefficient",
                value: 3.0 + 5.0 * order as f64,
            },
            Factor {
                name: "base cost coefficient",
                label: "Shelf base cost",
                value: shelf.base_cost,
            },
            Factor {
                name: "level growth",
                label: "Level growth",
                value: shelf.cost_growth.powf(level),
            },
        ];
        let value = factors.iter().map(|factor| factor.value).product();
        // The first factor is the base price; it is kept so a breakdown can show
        // every term the upstream multiplies.
        factors[0].value = ARMORY_BASE;
        Cost {
            value,
            index,
            level,
            currency_slot,
            factors,
            missing: Vec::new(),
        }
    }

    /// The shelf index at an order position: upstream `ARMORY_ORDER[order] ??
    /// order`.
    fn shelf_index(&self, order: usize) -> usize {
        self.armory_unlock_order()
            .get(order)
            .map(|index| *index as usize)
            .unwrap_or(order)
    }

    /// One Armory shelf's static row by its `ArmoryUpg` index.
    fn armory_row(&self, index: usize) -> Option<&'static data::Armory> {
        data::armory(index)
    }

    /// Every shelf in the order the list prints: upstream `armoryRows`.
    pub fn armory_rows(&self) -> Vec<ArmoryRow> {
        let unlocked_count = self.armory_unlocked_count();
        self.armory_unlock_order()
            .iter()
            .enumerate()
            .filter_map(|(order, index)| {
                let index = *index as usize;
                let shelf = data::armory(index)?;
                let level = self.armory_level(index);
                let cost = self.armory_upgrade_cost(order);
                let balance = self.g_cell(row::CURRENCIES, cost.currency_slot.max(0) as usize);
                let maxed = level >= shelf.max_level;
                Some(ArmoryRow {
                    index,
                    order,
                    name: shelf.name.as_str(),
                    level,
                    max_level: shelf.max_level,
                    current_bonus: level * shelf.bonus_per_level,
                    next_bonus: (level + 1.0) * shelf.bonus_per_level,
                    bonus_per_level: shelf.bonus_per_level,
                    cost: cost.clone(),
                    maxed,
                    unlocked: order < unlocked_count,
                    balance,
                    affordable: !maxed && cost.value <= balance,
                })
            })
            .collect()
    }

    /// The next levels of one shelf, priced one at a time: upstream
    /// `armoryCostProjection`.
    pub fn armory_cost_plan(&self, order: usize, count: usize) -> Option<CostPlan> {
        let index = self.shelf_index(order);
        let shelf = data::armory(index)?;
        let level = self.armory_level(index);
        let max_level = shelf.max_level;
        let steps = (count as f64).floor().min(max_level - level).max(0.0) as usize;
        let redux = self.masterclass_redux();
        let currency_slot = data::data()
            .armory
            .get(order)
            .map(|row| row.currency_slot)
            .unwrap_or(-1);
        let mut rows = Vec::with_capacity(steps);
        let mut cumulative_discounted = 0.0;
        let mut cumulative_full = 0.0;
        for step in 0..steps {
            let at = level + step as f64;
            // A counter of zero is inside any positive Legend limit; a huge one
            // forces the post-limit prefix.
            let discounted = self.armory_upgrade_cost_at(order, at, 0.0).value;
            let full = self.armory_upgrade_cost_at(order, at, MAX_COUNTER).value;
            cumulative_discounted += discounted;
            cumulative_full += full;
            rows.push(CostStep {
                level: at,
                next_level: at + 1.0,
                discounted,
                full,
                cumulative_discounted,
                cumulative_full,
            });
        }
        Some(CostPlan {
            index,
            order,
            name: shelf.name.as_str(),
            level,
            max_level,
            currency_slot,
            balance: self.g_cell(row::CURRENCIES, currency_slot.max(0) as usize),
            daily_limit: redux.daily_limit,
            purchase_counter: redux.purchase_counter,
            discounts_left: redux.discounts_left(),
            discount_available: redux.daily_limit > 0.0,
            rows,
        })
    }

    /// What one orblet level costs: upstream `orbletCost`.
    pub fn orblet_cost(&self, index: usize) -> f64 {
        let Some(orblet) = data::orblet(index) else {
            return f64::INFINITY;
        };
        let level = self.orblet_level(index);
        let value = level + orblet.base_cost * orblet.cost_growth.powf(level);
        if value < 1e6 { value.floor() } else { value }
    }

    /// An orblet's level cap: upstream `orbletMax`.
    pub fn orblet_max(&self, index: usize) -> f64 {
        data::orblet(index)
            .map(|orblet| orblet.max_level)
            .unwrap_or(0.0)
    }

    /// Whether the previous orblet has been bought at least once.
    pub fn orblet_unlocked(&self, index: usize) -> bool {
        index == 0 || self.orblet_level(index - 1) >= 1.0
    }

    /// The Orblets the save owns, counted across the chest and every character's
    /// inventory: upstream `itemCount('Orblet')`.
    pub fn orblet_owned(&self) -> Owned {
        self.item_count("Orblet")
    }

    /// Every Orblet market row: upstream `orbletRows`.
    pub fn orblet_rows(&self) -> Vec<OrbletRow> {
        let owned = self.orblet_owned();
        (0..data::data().orblets.len())
            .map(|index| {
                let orblet = data::orblet(index).expect("every index the data carries");
                let level = self.orblet_level(index);
                let maxed = level >= orblet.max_level;
                let unlocked = self.orblet_unlocked(index);
                let cost = if maxed {
                    f64::INFINITY
                } else {
                    self.orblet_cost(index)
                };
                let purchasable = unlocked && !maxed && !owned.partial && cost.is_finite();
                OrbletRow {
                    index,
                    name: orblet.name.as_str(),
                    level,
                    max_level: orblet.max_level,
                    bonus: (level * orblet.bonus_per_level).floor(),
                    next_bonus: if maxed || index == 4 {
                        None
                    } else {
                        Some(((level + 1.0) * orblet.bonus_per_level).floor())
                    },
                    action: index == 4,
                    maxed,
                    unlocked,
                    cost: if maxed { f64::INFINITY } else { cost },
                    owned: owned.value,
                    affordable: purchasable && owned.value >= cost,
                    partial: owned.partial,
                    missing: owned.missing.clone(),
                }
            })
            .collect()
    }

    /// How many of one item the chest and every character inventory hold:
    /// upstream `itemCount`.
    fn item_count(&self, item: &str) -> Owned {
        let mut value = 0.0;
        let mut missing = Vec::new();
        match &self.inventories.chest {
            Some((order, quantity)) => value += pair_count(order, quantity, item),
            None => missing.push("storage inventory".to_owned()),
        }
        for (index, inventory) in self.inventories.characters.iter().enumerate() {
            match inventory {
                Some((order, quantity)) => value += pair_count(order, quantity, item),
                None => missing.push(format!("character inventory {index}")),
            }
        }
        Owned {
            value,
            partial: !missing.is_empty(),
            missing,
        }
    }
}

/// How many of `item` one inventory pair holds: upstream `_itemPairCount`.
fn pair_count(order: &[Value], quantity: &[Value], item: &str) -> f64 {
    order
        .iter()
        .zip(quantity)
        .filter(|(key, _)| key.as_str() == Some(item))
        .filter_map(|(_, count)| json::number(count))
        .sum()
}
