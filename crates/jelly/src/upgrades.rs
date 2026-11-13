//! The upgrade list: every `JellyUPG` row in the `Research[44]` purchase order,
//! what the next one costs and what the save's level buys.
//!
//! Ports `jellyUpgradeAtOrder`, `jellyUpgradeResearchLevelRequirement`,
//! `jellyUpgradeCost`, `jellyUpgradeCanBuy`, `jellyUpgradeStatus` and
//! `jellyUpgradeDisplay` from `js/stats/systems/w7/jelly-operator.js`.
//!
//! The price is the order position's own curve, the upgrade's cost scale at the
//! level already owned and Lower Cholesterol's cost divisor. Menagerie's Upgrade
//! Doubler doubles a quantity, not a price, so a doubled upgrade is worth twice
//! as much for the same curve.
//!
//! The game's description text carries placeholders the page fills: `{` is the
//! upgrade's own quantity, `}` the Bloodcell factor it buys and `$` the effect
//! that row computes. [`fmt_val`] is the page's `fmtVal`, which the filled text
//! prints with.

use super::board::slot_purchases_left;
use super::data::data;
use super::{Jelly, fmt_val};

/// How many rows the page lists (`jellyUpgradeOrder().slice(0, 40)`).
pub const LISTED: usize = 40;

/// `jellyUpgradeAtOrder`: the upgrade the purchase order buys at `order`.
///
/// The order names 100 slots while the game's `JellyUPG` table defines 40, so a
/// slot past the table is `None`.
pub fn at_order(order: usize) -> Option<usize> {
    data().upgrade_order.get(order).copied()
}

/// `jellyUpgradeResearchLevelRequirement`: the Research level a slot appears at.
pub fn research_level_requirement(order: usize) -> f64 {
    let order = order as f64;
    15.0 + 2.0 * order + (order / 15.0).floor() - (order / 11.0).floor()
}

/// `jellyUpgradeStatus`: whether one purchase slot can be bought from this save.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Status {
    /// The upgrade the slot buys; `None` past the shipped table.
    pub id: Option<usize>,
    pub required_research_level: f64,
    pub research_level: f64,
    /// Whether the Research level reveals the slot at all.
    pub level_visible: bool,
    /// Whether the previous slot is owned.
    pub prerequisite_met: bool,
    /// Whether the upgrade still has a level left to buy.
    pub below_max: bool,
    /// Whether the saved Bloodcells cover the price.
    pub affordable: bool,
    pub can_buy: bool,
}

/// One row of the upgrade list: `jellyUpgradeDisplay` joined with the slot's
/// price and status.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub id: usize,
    /// Where the slot sits in the purchase order.
    pub order: usize,
    pub name: String,
    /// The game text with its placeholders filled.
    pub description: String,
    pub level: f64,
    pub max_level: f64,
    pub quantity: f64,
    pub doubled: bool,
    /// What the saved level buys right now.
    pub effect: String,
    /// What one more level buys.
    pub per_level: String,
    /// The page's own note about a formula this build corrects.
    pub note: String,
    /// The slot's price; `f64::INFINITY` when the order names an unfilled slot.
    pub cost: f64,
    pub status: Status,
}

impl Jelly {
    /// `jellyUpgradeCost`: the price of the purchase slot at `order`.
    pub fn upgrade_cost(&self, order: usize) -> f64 {
        if order == 0 {
            return 0.0;
        }
        let Some(upgrade) = at_order(order).and_then(|id| data().upgrade(id)) else {
            return f64::INFINITY;
        };
        let base_cost = if upgrade.base_cost == 0.0 {
            1.0
        } else {
            upgrade.base_cost
        };
        let order = order as f64;
        base_cost.max(0.1)
            * (1.0 + order / 7.0)
            * (6.0 + 5.0 * order + order * order)
            * (1.4 + (order - 3.0).max(0.0) / 30.0).powf((order - 4.0).max(0.0))
            * 1.3_f64.powf((order - 20.0).max(0.0))
            / (1.0 + self.upgrade_quantity(34) / 100.0)
            * upgrade.cost_scale.powf(self.upgrade_level(upgrade.id))
    }

    /// `jellyUpgradeStatus`: whether the slot at `order` can be bought.
    pub fn upgrade_status(&self, order: usize) -> Status {
        let id = at_order(order);
        let upgrade = id.and_then(|id| data().upgrade(id));
        let research_level = self.account.research_level.max(0.0).floor();
        let required_research_level = research_level_requirement(order);
        let previous = order.checked_sub(1).and_then(at_order);
        let prerequisite_met =
            order == 0 || previous.is_some_and(|id| self.upgrade_level(id) >= 1.0);
        let below_max = upgrade.is_some_and(|upgrade| {
            upgrade.max_level > 998.0 || self.upgrade_level(upgrade.id) < upgrade.max_level
        });
        let level_visible = research_level >= required_research_level;
        let affordable = upgrade.is_some() && self.progress.bloodcells >= self.upgrade_cost(order);
        Status {
            id,
            required_research_level,
            research_level,
            level_visible,
            prerequisite_met,
            below_max,
            affordable,
            can_buy: level_visible && prerequisite_met && below_max && affordable,
        }
    }

    /// `jellyUpgradeDisplay`: one slot as the list prints it.
    pub fn upgrade_row(&self, order: usize) -> Option<Row> {
        let upgrade = at_order(order).and_then(|id| data().upgrade(id))?;
        let id = upgrade.id;
        let quantity = self.upgrade_quantity(id);
        let level = self.upgrade_level(id);
        let doubled = self.upgrade_doubled(id);
        let step = upgrade.per_level * if doubled { 2.0 } else { 1.0 };
        let mut per_level = format!("+{step} per level");
        let plural = if step == 1.0 { "" } else { "s" };
        if upgrade.max_level == 1.0 && upgrade.per_level == 1.0 {
            per_level = "One-time unlock".to_string();
        } else if id == 16 {
            per_level = format!("+{step} Fever mode{plural} per level");
        } else if [8, 9, 15, 35].contains(&id) {
            per_level = format!("+{step} use{plural} per level");
        } else if id == 17 {
            per_level = format!("+{step} percentage point{plural} per cell level");
        } else if id == 29 {
            per_level = format!(
                "+{}x Stronkroid speed per level, starting at 1.51x",
                display_number(step / 100.0)
            );
        } else if id == 34 {
            per_level = format!(
                "+{step} cost-divisor point{plural} per level; effective reduction is diminishing"
            );
        } else if id == 38 {
            per_level = format!("+{step}% of best operation Bloodcells daily per level");
        } else if upgrade.description.contains('{') || upgrade.description.contains('}') {
            per_level = format!("+{step}% per level");
        }
        if doubled && per_level != "One-time unlock" {
            per_level.push_str(" (Upgrade Doubler 2x)");
        }

        let mut note = "";
        let mut dollar = format!("{quantity}");
        match id {
            5 => {
                note = "Runtime correction: infected slots add to one global damage multiplier; \
                        they do not multiply damage by 1.10x independently.";
            }
            9 => dollar = format!("{}", slot_purchases_left(self)),
            10 => {
                dollar = format!(
                    "{}",
                    (100.0 * self.cell_exp_multiplier(None)).floor() / 100.0
                );
            }
            15 => dollar = format!("{}", (1.0 + quantity).round()),
            17 => dollar = format!("{}", (1.0 + quantity).round()),
            23 => {
                dollar = format!(
                    "{}",
                    (100.0
                        * self.currency_multiplier(
                            None,
                            self.total_cell_level(),
                            self.progress.best_dps,
                        ))
                    .round()
                        / 100.0
                );
            }
            29 => dollar = format!("{}", 1.0 + (50.0 + quantity) / 100.0),
            32 => dollar = format!("{}", quantity * (self.total_cell_level() / 10.0).floor()),
            33 => dollar = format!("{}", quantity * self.total_cell_level()),
            34 => {
                dollar = format!(
                    "{}",
                    (1e4 * (1.0 - 1.0 / (1.0 + quantity / 100.0))).round() / 100.0
                );
            }
            37 => {
                note = "No Research[17][37] formula consumer exists in this build, so the \
                        modeled immediate gain is zero.";
            }
            38 => dollar = display_number(self.daily_bloodcells()),
            39 => {
                note = "This unlocks the obstruction-bonus interface/tutorial; reward formulas \
                        themselves are gated by cleared obstructions.";
            }
            _ => {}
        }
        if id == 32 {
            note = "Runtime correction: the operation formula uses every 100 combined cell \
                    levels, although the game description and displayed total use every 10.";
        }
        let mut description = fill(&upgrade.description, quantity, &dollar);
        if id == 10 && self.upgrade_quantity(17) >= 1.0 {
            let scale = (100.0 + (1.0 + self.upgrade_quantity(17))).round() / 100.0;
            description = description.replacen("1.01x", &format!("{scale}x"), 1);
        }

        let mut effect = format!("+{quantity}");
        let plural = if quantity == 1.0 { "" } else { "s" };
        if id <= 7 {
            effect = if level >= 1.0 { "Unlocked" } else { "Locked" }.to_string();
        } else if [12, 14, 28, 36].contains(&id) {
            effect = if level >= 1.0 { "Active" } else { "Locked" }.to_string();
        } else if [5, 37, 39].contains(&id) {
            effect = if level >= 1.0 {
                "Unlocked; no scalable value"
            } else {
                "Locked"
            }
            .to_string();
        } else if [10, 17, 18, 19, 20, 23, 24, 25, 30, 31, 32, 33, 38].contains(&id) {
            effect = format!("+{quantity}%");
        } else if [11, 13, 21, 22, 26, 27].contains(&id) {
            effect = format!("{:.2}x", 1.0 + quantity / 100.0);
        } else if id == 8 {
            effect = format!("{} plot token{plural}", quantity.round());
        } else if id == 9 {
            let left = slot_purchases_left(self);
            let plural = if left == 1.0 { "" } else { "s" };
            effect = format!("{} unused plot token{plural}", left);
        } else if id == 15 {
            effect = format!("{} Virus maximum", (1.0 + quantity).round());
        } else if id == 16 {
            effect = format!("{} Fever type{plural}", quantity.round());
        } else if id == 29 {
            effect = format!("{:.2}x attack speed", 1.0 + (50.0 + quantity) / 100.0);
        } else if id == 34 {
            effect = format!("{:.2}x cost divisor", 1.0 + quantity / 100.0);
        } else if id == 35 {
            effect = format!("{} revival{plural}", quantity.round());
        }
        match id {
            10 => effect = format!("{:.2}x EXP per hit", self.cell_exp_multiplier(None)),
            12 => {
                effect = format!(
                    "{:.3}x Bloodcells",
                    super::metrics::best_dps_multiplier_from_value(self.progress.best_dps)
                );
            }
            17 => effect = format!("+{}% damage per cell level", 1.0 + quantity),
            32 => {
                effect = format!(
                    "+{}% total damage",
                    quantity * (self.total_cell_level() / 100.0).floor()
                );
            }
            33 => effect = format!("+{}% total Bloodcells", quantity * self.total_cell_level()),
            38 => {
                effect = format!(
                    "{} Bloodcells per day",
                    display_number(self.daily_bloodcells())
                );
            }
            _ => {}
        }

        Some(Row {
            id,
            order,
            name: upgrade.name.clone(),
            description,
            level,
            max_level: upgrade.max_level,
            quantity,
            doubled,
            effect,
            per_level,
            note: note.to_string(),
            cost: self.upgrade_cost(order),
            status: self.upgrade_status(order),
        })
    }

    /// The rows the list prints, in the purchase order.
    pub fn upgrade_rows(&self) -> Vec<Row> {
        (0..LISTED)
            .filter_map(|order| self.upgrade_row(order))
            .collect()
    }

    /// `jellyDailyBloodcells`: what the daily reset pays from the best operation.
    pub fn daily_bloodcells(&self) -> f64 {
        self.progress.best_bloodcells * self.upgrade_quantity(38) / 100.0
    }
}

/// The game description with its placeholders filled, as the page prints it:
/// `{` is the quantity, `}` the Bloodcell factor it buys, `$` the row's effect
/// and `@` a line break.
fn fill(description: &str, quantity: f64, dollar: &str) -> String {
    let text = description.replacen('{', &format!("{quantity}"), 1);
    let text = text.replacen('}', &format!("{:.2}", 1.0 + quantity / 100.0), 1);
    text.replacen('$', dollar, 1)
        .replace('@', " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// `displayNumber`: a whole number, two decimals, or the game's suffix at 1000.
pub fn display_number(value: f64) -> String {
    if value.abs() >= 1000.0 {
        return fmt_val(value);
    }
    if value.fract() == 0.0 {
        return format!("{value}");
    }
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
