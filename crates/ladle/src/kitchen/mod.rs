//! The Kitchens & Jellyfish reset (W4): meal levels and stocks, meal costs, the
//! reset kitchen layout and the daily ladle investment plan.
//!
//! Ported from `js/stats/systems/w4/cooking-ladle.js` (`mealLevels`, `mealStocks`,
//! `mealName`, `mealRequirement`, `mealCostInputs`, `mealLevelCost`,
//! `blueJellyfishInfo`, `nmlbInfo`, `nmlbTarget`, `nmlbResultLevel`,
//! `optimizeKitchenReset`, `planMealLeveling` and `planKitchenForecast`) and the
//! "Kitchens & Jellyfish" tab of `ladle-calc.html`.
//!
//! Upstream offers three objectives: the most meal ranks, the meal targets and
//! kitchen speed. [`Objective`] carries the first two; the speed objective ranks by
//! the meal-level `kitchenSpeedEvaluator`, which this build does not have, so it is
//! not offered rather than ranked by a constant.
//!
//! The kitchen speed model upstream builds from the account stats engine
//! (`cookingSpeedModel`) is one entered number here: the Cooking speed /hr a
//! kitchen has at Speed Lv 0, read from the game's Cooking panel the same way the
//! ladle rates read theirs in [`super::Panel`]. The meal max level is entered
//! beside it for the same reason. The save carries the rest: the unlocked kitchens
//! and their speed levels, the meal levels and stocks, the Blue Jellyfish grant
//! and No Meal Left Behind's level count.

pub mod forecast;
pub mod layout;
mod plan;
#[cfg(test)]
mod tests;

pub use forecast::{Forecast, SCENARIOS, Scenario, forecast};
pub use layout::{Layout, LayoutChoice, LayoutKitchen, LayoutMeal, LayoutMode};
pub use plan::{Day, Inputs, MealOutcome, Phase, PhaseKind, Plan, Policy, Saving, Spend, plan};

use idleon_save::Save;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const COOKING_JSON: &str = include_str!("../../data/cooking.json");

/// `NMLB_LEVEL_CAP`: the reset awards levels only below 150.
pub const NMLB_LEVEL_CAP: f64 = 150.0;

/// The longest horizon `planMealLeveling` accepts, in days.
pub const MAX_DAYS: u32 = 3650;

/// The horizon the upstream page starts on.
pub const DEFAULT_DAYS: u32 = 30;

/// How a plan and the reset layout rank their candidates.
///
/// Upstream also offers `speed`, which ranks by the Cooking speed the meal levels
/// reach. That term is the `kitchenSpeedEvaluator` model this build does not have,
/// so the objective stays out until the meal-level speed engine lands rather than
/// ranking by a number the section cannot compute.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Objective {
    /// Spend on the cheapest meal level, wherever it is: `_planObjective`'s `ranks`.
    #[default]
    Ranks,
    /// Spend only on meals below their entered target: the `targets` objective.
    Targets,
}

/// One meal's static row from `MealINFO`.
#[derive(Clone, Debug, Deserialize)]
pub struct Meal {
    /// The row's name with the underscores replaced by spaces.
    pub name: String,
    /// `MealINFO[idx][1]`: the Cooking skill level that unlocks it. A meal with
    /// no requirement cannot be cooked, so the plan never levels it.
    pub requirement: f64,
    /// `MealINFO[idx][2]`: the bonus one level grants.
    pub bonus: f64,
}

/// The extracted Cooking tables: `MealINFO` and the `N2L` encoding.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Data {
    meals: Vec<Meal>,
    /// `N2L`: the alphabet the emporium and event-shop ownership strings encode
    /// their flags with.
    number_to_letter: String,
}

fn data() -> &'static Data {
    static DATA: OnceLock<Data> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(COOKING_JSON).expect("crates/ladle/data/cooking.json is extracted")
    })
}

/// Every meal `MealINFO` defines, in index order.
pub fn meals() -> &'static [Meal] {
    &data().meals
}

/// One meal's display name, or `Meal` when the index is past the table.
pub fn meal_name(index: usize) -> &'static str {
    meals()
        .get(index)
        .map(|meal| meal.name.as_str())
        .unwrap_or("Meal")
}

/// `mealRequirement`: the Cooking level a meal unlocks at, or 0 when unknown.
pub fn meal_requirement(index: usize) -> f64 {
    meals()
        .get(index)
        .map(|meal| meal.requirement)
        .unwrap_or(0.0)
}

/// `mealCostInputs`: the account terms the meal cost formula reads.
///
/// Upstream resolves all five from the save; the stats engine terms are entered
/// here, and the defaults are the neutral values the formula carries when a save
/// has none of them.
#[derive(Clone, Copy, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CostInputs {
    /// `companionBonusForSave(162)`: the meal cost companion.
    pub companion_162: f64,
    /// `achieveStatus(233)`: 0 or 1.
    pub achievement_233: f64,
    /// `cloudBonus(33)`: the weekly boss' meal cost discount, as a fraction.
    pub cloud_33: f64,
    /// `OptLacc[193]`: the Dream days the save has banked.
    pub discount_days: f64,
    /// `Dream[11]`: the most Dream days the discount can stack.
    pub dream_11: f64,
}

/// `mealLevelCost`: what one meal level costs at `level`.
///
/// `discount_days` is the value the plan passes in (the save's Dream days, or 0
/// once the first purchase has spent the current day's), capped by `inputs.dream_11`.
pub fn meal_level_cost(level: f64, inputs: CostInputs, discount_days: f64) -> f64 {
    let level = if level.is_finite() {
        level.max(0.0)
    } else {
        0.0
    };
    let days = discount_days.max(0.0).min(inputs.dream_11.max(0.0));
    let tiers = ((level + 1000.0) / 1111.0).floor();
    let companion = (1.0 / (5.0 * inputs.companion_162).max(1.0)).max(0.001);
    let achievement = 1.0 / (1.0 + 10.0 * inputs.achievement_233 / 100.0).clamp(1.0, 5.0);
    let dream = (0.8 - 0.22 * inputs.cloud_33)
        .max(0.58)
        .powf(days)
        .max(0.001);
    companion
        * 10f64.powf(22.0 * tiers)
        * achievement
        * dream
        * (10.0 + level + level * level)
        * (1.2 + 0.05 * level).powf(level)
        * (1.0 + 0.4 * tiers).powf(level)
}

/// One kitchen's save row: whether it is unlocked, its speed level and the meal
/// it was last set to cook.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kitchen {
    /// The `Cooking` row index.
    pub index: usize,
    /// `Cooking[k][0] !== 0`: the kitchen is unlocked.
    pub unlocked: bool,
    /// `Cooking[k][1]`: the assigned meal, or -1 when idle.
    pub saved_meal: i32,
    /// `Cooking[k][6]`: the kitchen's speed upgrade level.
    pub speed_level: f64,
    /// `gemItemsData[120] > k`: the Gem Shop kitchen doubler (`dn1`).
    pub doubler: bool,
}

impl Kitchen {
    /// The kitchen's speed at the panel's base speed: `base × (1 + Lv / 10) ×
    /// (1 + dn1)`, the upstream `Gem Shop kitchen doubler` and speed level terms
    /// with every account-wide term folded into `base`.
    pub fn speed(&self, base: f64) -> f64 {
        let doubler = if self.doubler { 2.0 } else { 0.0 };
        base * (1.0 + self.speed_level / 10.0) * (1.0 + doubler)
    }
}

/// The save's meal levels and stocks (`Meals[0]` and `Meals[2]`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Meals {
    pub levels: Vec<f64>,
    pub stocks: Vec<f64>,
}

/// `blueJellyfishInfo`: the Blue Jellyfish grant a save carries.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Jelly {
    /// Whether the save carries `Menagerie` rows at all.
    pub available: bool,
    /// `menagerieUnlockOwned(5) === 1`: new donations are allowed.
    pub unlocked: bool,
    /// `Menagerie[0][5]`: the hours every kitchen cooks at each reset.
    pub daily_hours: f64,
}

/// The save-derived Cooking state the reset plan reads.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reset {
    pub meals: Meals,
    pub kitchens: Vec<Kitchen>,
    pub jelly: Jelly,
    /// `nmlbInfo(S).levels`: 0 without the Emporium 16 or bundle unlock, then one
    /// level for each.
    pub nmlb: f64,
}

impl Reset {
    /// Read the kitchens, meals and Blue Jellyfish grant from a decoded save.
    ///
    /// `None` when the save carries neither `Cooking` nor `Meals`: the plan needs
    /// both a kitchen list and the meal levels it raises.
    pub fn from_save(save: &Save) -> Option<Self> {
        let meals = meal_rows(save)?;
        let mut kitchens = kitchens(save);
        if kitchens.is_empty() {
            return None;
        }
        kitchens.sort_by_key(|kitchen| kitchen.index);
        Some(Self {
            meals,
            kitchens,
            jelly: jelly(save),
            nmlb: nmlb(save),
        })
    }

    /// The total speed of every unlocked kitchen.
    pub fn speed(&self, base: f64) -> f64 {
        self.kitchens
            .iter()
            .filter(|kitchen| kitchen.unlocked)
            .map(|kitchen| kitchen.speed(base))
            .sum()
    }

    /// The kitchens the reset can assign.
    pub fn unlocked(&self) -> impl Iterator<Item = &Kitchen> {
        self.kitchens.iter().filter(|kitchen| kitchen.unlocked)
    }

    /// Whether No Meal Left Behind raises a level at every reset.
    pub fn nmlb_active(&self) -> bool {
        self.nmlb > 0.0
    }
}

/// Why a reset plan cannot be built.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// `Cooking` defines more kitchens than the game supports.
    Kitchens,
    /// A pinned kitchen is locked or missing.
    PinnedKitchen,
    /// A pinned meal is excluded, locked or undiscovered.
    PinnedMeal,
    /// A Blue Jellyfish donation is not a whole number of available ladles.
    Donation,
    /// A donation was asked for without the Blue Jellyfish unlocked.
    Locked,
    /// A meal level, stock, speed or horizon is not a finite number.
    Numbers,
    /// The target objective was asked for without a target to spend toward.
    Targets,
}

impl Problem {
    /// The message the upstream page shows for the same rejected input.
    pub fn message(self) -> &'static str {
        match self {
            Problem::Kitchens => "Cooking supports at most ten kitchens.",
            Problem::PinnedKitchen => "A pinned kitchen is locked or missing.",
            Problem::PinnedMeal => "A pinned meal is excluded or undiscovered.",
            Problem::Donation => {
                "Blue Jellyfish donations must be whole ladles within the available budget."
            }
            Problem::Locked => "Blue Jellyfish is not unlocked in this save.",
            Problem::Numbers => "Enter a finite horizon, speed and meal state.",
            Problem::Targets => "Set at least one meal target level.",
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message())
    }
}

impl std::error::Error for Problem {}

/// `nmlbResultLevel`: the level No Meal Left Behind leaves a meal at.
pub fn nmlb_result_level(from: f64, gain: f64, max_level: f64) -> f64 {
    if from >= NMLB_LEVEL_CAP {
        return from;
    }
    NMLB_LEVEL_CAP.min(max_level).min(from + gain)
}

/// `nmlbTarget`: the lowest eligible meal, ties to the highest index.
pub fn nmlb_target(levels: &[f64], max_level: f64) -> Option<usize> {
    let mut pick = None;
    let mut lowest = f64::INFINITY;
    for (meal, level) in levels.iter().enumerate() {
        if *level >= 2.0 && *level <= lowest && *level < max_level {
            pick = Some(meal);
            lowest = *level;
        }
    }
    pick
}

/// A whole-ladle donation, or 0: upstream rejects anything else.
fn donation(value: f64, available: f64, unlocked: bool) -> Result<f64, Problem> {
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 {
        return Err(Problem::Donation);
    }
    if value > available {
        return Err(Problem::Donation);
    }
    if value > 0.0 && !unlocked {
        return Err(Problem::Locked);
    }
    Ok(value)
}

/// The save's meals: `Meals[0]` levels and `Meals[2]` stocks.
fn meal_rows(save: &Save) -> Option<Meals> {
    let rows = save.get("Meals")?;
    let levels = idleon_save::at(&rows, 0)
        .map(idleon_save::row_values)
        .unwrap_or_default()
        .iter()
        .map(|value| idleon_save::number(value).unwrap_or(0.0))
        .collect::<Vec<f64>>();
    if levels.is_empty() {
        return None;
    }
    let stocks = idleon_save::at(&rows, 2)
        .map(idleon_save::row_values)
        .unwrap_or_default()
        .iter()
        .map(|value| idleon_save::number(value).unwrap_or(0.0).max(0.0))
        .collect();
    Some(Meals { levels, stocks })
}

/// Every `Cooking` row: its unlock state, meal, speed level and gem doubler.
fn kitchens(save: &Save) -> Vec<Kitchen> {
    let doubler_count = save
        .get("GemItemsPurchased")
        .map(|row| idleon_save::row_values(&row))
        .and_then(|row| row.get(120).cloned())
        .and_then(|value| idleon_save::number(&value))
        .unwrap_or(0.0);
    let Some(rows) = save.get("Cooking") else {
        return Vec::new();
    };
    let columns = idleon_save::row_values(&rows);
    columns
        .iter()
        .enumerate()
        .filter_map(|(index, row)| {
            let cells = idleon_save::row_values(row);
            if cells.is_empty() {
                return None;
            }
            let state = cells.first().and_then(idleon_save::number).unwrap_or(0.0);
            let meal = cells.get(1).and_then(idleon_save::number);
            let speed_level = cells.get(6).and_then(idleon_save::number).unwrap_or(0.0);
            Some(Kitchen {
                index,
                unlocked: state != 0.0,
                saved_meal: match (state == 2.0, meal) {
                    (true, Some(meal)) if meal >= 0.0 && meal.fract() == 0.0 => meal as i32,
                    _ => -1,
                },
                speed_level,
                doubler: doubler_count > index as f64,
            })
        })
        .collect()
}

/// `blueJellyfishInfo`: the Menagerie rows that hold the grant and its unlock.
fn jelly(save: &Save) -> Jelly {
    let Some(rows) = save.get("Menagerie") else {
        return Jelly::default();
    };
    let row = |index: usize| {
        idleon_save::at(&rows, index)
            .map(idleon_save::row_values)
            .unwrap_or_default()
    };
    let daily_hours = row(0)
        .get(5)
        .and_then(idleon_save::number)
        .unwrap_or(0.0)
        .max(0.0);
    let unlocked = row(2)
        .get(5)
        .and_then(idleon_save::number)
        .unwrap_or(0.0)
        .max(0.0)
        >= 1.0;
    Jelly {
        available: true,
        unlocked,
        daily_hours,
    }
}

/// `nmlbInfo(S).levels`: the Emporium 16 flag plus the bundle's two levels.
fn nmlb(save: &Save) -> f64 {
    let bundle = bundle(save, "bun_s");
    let emporium = emporium_owned(save, 16);
    (if bundle { 2.0 } else { 0.0 }) + (if emporium { 1.0 } else { 0.0 })
}

/// `hasBundle`: a bundle key set to 1, inside the save's own or the `h` holder.
fn bundle(save: &Save, key: &str) -> bool {
    let Some(bundles) = save.get("BundlesReceived") else {
        return false;
    };
    let holder = match bundles.get("h") {
        Some(inner @ serde_json::Value::Object(_)) => inner.clone(),
        _ => bundles,
    };
    holder
        .get(key)
        .and_then(idleon_save::number)
        .is_some_and(|value| value == 1.0)
}

/// `emporiumBonus(t, Ninja[102][9])`: `N2L` membership in the emporium string.
fn emporium_owned(save: &Save, index: usize) -> bool {
    let flags = save
        .get("Ninja")
        .and_then(|rows| idleon_save::at(&rows, 102).cloned())
        .map(|row| idleon_save::row_values(&row))
        .and_then(|row| row.get(9).cloned())
        .and_then(|value| value.as_str().map(str::to_owned))
        .unwrap_or_default();
    data()
        .number_to_letter
        .chars()
        .nth(index)
        .is_some_and(|letter| flags.contains(letter))
}

/// The pin map a layout choice carries: kitchen index to meal index, `-1` idle.
pub type Pins = BTreeMap<usize, i32>;
