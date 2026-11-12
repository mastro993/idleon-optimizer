//! The Yellow Point Allocator: the yellow points spent on meal mastery.
//!
//! Ports the `meal-alloc` tab of `cooking-mastery-calc.html` and the module it
//! imports, `js/stats/systems/common/meal-impact.js`:
//!
//! - [`bonus_multi_cook`] is `bonusMultiCook`: the mastery factor a meal's
//!   contribution scales with, `1 + lv / (lv + 5)`.
//! - [`Yellow::allocation`] is `optimizeForGoal`: a greedy walk over the budget
//!   that gives each point to the meal whose category gains the most goal value,
//!   starting from zero mastery on every meal, so the plan is a full
//!   reallocation. Upstream keeps the first meal on a tie (`gain > bestGain`
//!   with `bestGain` starting at -1); this port does the same, which is why the
//!   oracle compares the whole allocation and not only its value.
//! - [`Yellow::budget`] is `yellowPointBudget`: the rank, the Rift Spooker, the
//!   research grid node, the event shop and the Jelly reward.
//! - [`GOALS`] is `GOALS` itself: each goal names the meal categories that feed
//!   it and how their totals turn into its value.
//!
//! The three stat goals upstream composes (`mcook`, `rcook`, `cropevo`) read
//! the selected character's Farming or Summoning level and the account's
//! kitchens; the rest read one category total. A meal only enters a goal when
//! its saved level and its per-level bonus are above zero, exactly as
//! `optimizeForGoal` filters them, and a goal with fewer than two such meals is
//! hidden the way the page hides it: one meal leaves nothing to compare.
//!
//! Every number here is the page's own arithmetic; the save reading that feeds
//! it lives in `idleon_account`, which owns the field names.

use crate::data::{Meal, meals};
use idleon_common::formula::ValueFormat;

/// How a goal's value reads: the page's `goal.unit`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Unit {
    /// A percentage of the category total, the page's default.
    #[default]
    Percent,
    /// A flat category total, e.g. Base Defence.
    Flat,
    /// A count of levels, e.g. Toilet Paper Postage.
    Levels,
    /// A multiplication factor, e.g. Meal Cooking Speed.
    Multiplier,
}

impl Unit {
    /// The value as the card prints it.
    pub fn format(self, value: f64) -> String {
        let number = ValueFormat::Raw;
        match self {
            Unit::Percent => ValueFormat::Percent.format(value),
            Unit::Flat => number.format(value),
            Unit::Levels => format!("+{} levels", number.format(value)),
            Unit::Multiplier => format!("{}x", number.format(value)),
        }
    }
}

/// How a goal turns its category totals into a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// The single category total itself.
    Total,
    /// `event53 × (1 + Mcook) × (1 + zMealFarm × farm steps) × (1 + KitchenEff ×
    /// meal kitchen steps)`.
    MealCooking,
    /// `(1 + Rcook) × (1 + KitchenEff × recipe kitchen steps)`.
    RecipeCooking,
    /// `(1 + zCropEvo) × (1 + zCropEvoSumm × summoning steps)`.
    CropEvolution,
}

/// One upstream goal: what it reads and how it reads it.
#[derive(Clone, Debug, PartialEq)]
pub struct YellowGoal {
    /// The page's `goal.id`, and the select's value.
    pub id: &'static str,
    pub name: &'static str,
    /// The `MealINFO` categories, in the order [`Value`] expects them.
    pub categories: &'static [&'static str],
    pub unit: Unit,
    /// The page's line under the result, empty for the plain goals.
    pub note: &'static str,
    pub value: Value,
}

impl YellowGoal {
    /// The page's `categoryLabels` line for one of the goal's categories.
    pub fn category_label(&self, category: &str) -> &'static str {
        match category {
            "Mcook" => "Direct Meal Cooking Speed",
            "zMealFarm" => "Burned Mello (Farming-scaled)",
            "KitchenEff" => "Cabbage (Kitchen upgrades)",
            "Rcook" => "Direct Recipe Cooking Speed",
            "zCropEvo" => "Direct Crop Evolution Chance",
            "zCropEvoSumm" => "Nyanborgir (Summoning-scaled)",
            _ => self.name,
        }
    }
}

/// A one-category goal, the page's `simpleGoal`.
const fn simple(
    id: &'static str,
    name: &'static str,
    category: &'static [&'static str],
    unit: Unit,
    note: &'static str,
) -> YellowGoal {
    YellowGoal {
        id,
        name,
        categories: category,
        unit,
        note,
        value: Value::Total,
    }
}

const PERCENT: Unit = Unit::Percent;

/// Every goal the page offers, in its own order (`GOALS`).
///
/// The UI hides the ones with fewer than two leveled meals; the order here is
/// what the select shows for the rest.
pub const GOALS: [YellowGoal; 44] = [
    // Skill & Lab.
    simple("seff", "Skill Efficiency", &["Seff"], PERCENT, ""),
    simple("lexp", "Lab EXP", &["Lexp"], PERCENT, ""),
    simple("linepct", "Lab Line Width (%)", &["LinePct"], PERCENT, ""),
    // Combat.
    simple("totdmg", "Total Damage", &["TotDmg"], PERCENT, ""),
    simple(
        "crit",
        "Critical Chance",
        &["Crit"],
        PERCENT,
        "Mutton grants Critical Chance, not Critical Damage.",
    ),
    simple("atkspd", "Basic Attack Speed", &["AtkSpd"], PERCENT, ""),
    simple("totacc", "Total Accuracy", &["TotAcc"], PERCENT, ""),
    simple("def", "Base Defence", &["Def"], Unit::Flat, ""),
    // Cooking.
    YellowGoal {
        id: "mcook",
        name: "Meal Cooking Speed",
        categories: &["Mcook", "zMealFarm", "KitchenEff"],
        unit: Unit::Multiplier,
        note: "Combines direct Meal Cooking Speed, Farming-scaled Burned Mello, and the \
               summed Cabbage benefit across all unlocked kitchens.",
        value: Value::MealCooking,
    },
    YellowGoal {
        id: "rcook",
        name: "Recipe Cooking Speed",
        categories: &["Rcook", "KitchenEff"],
        unit: Unit::Multiplier,
        note: "Combines direct Recipe Cooking Speed with the summed Cabbage benefit across \
               all unlocked kitchens.",
        value: Value::RecipeCooking,
    },
    simple(
        "kitchc",
        "Lower Kitchen Upgrade Costs",
        &["KitchC"],
        PERCENT,
        "",
    ),
    simple("cookexp", "Cooking EXP", &["CookExp"], PERCENT, ""),
    // Breeding & pets.
    simple("brexp", "Breeding EXP", &["BrExp"], PERCENT, ""),
    simple("breed", "Mob Breedability Rate", &["Breed"], PERCENT, ""),
    simple("npet", "New Mob Breed Odds", &["Npet"], PERCENT, ""),
    simple("petdmg", "Mob Battle Damage", &["PetDmg"], PERCENT, ""),
    simple(
        "tppete",
        "Toilet Paper Postage Max Levels",
        &["TPpete"],
        Unit::Levels,
        "",
    ),
    simple(
        "timeegg",
        "Lower Egg Creation Time",
        &["TimeEgg"],
        PERCENT,
        "",
    ),
    // Alchemy.
    simple(
        "liquid12",
        "Liquid 1+2 Max Capacity",
        &["Liquid12"],
        PERCENT,
        "",
    ),
    simple(
        "liquid34",
        "Liquid 3+4 Max Capacity",
        &["Liquid34"],
        PERCENT,
        "",
    ),
    // Sailing & gaming.
    simple("sailing", "Sailing Speed", &["Sailing"], PERCENT, ""),
    simple("gamingbits", "Bits Gain", &["GamingBits"], PERCENT, ""),
    simple("gamingexp", "Gaming EXP", &["GamingExp"], PERCENT, ""),
    simple(
        "sprow",
        "Skilling Prowess",
        &["Sprow"],
        PERCENT,
        "Leek grants skilling prowess. It does not affect Gaming sprout growth.",
    ),
    simple("divexp", "Divinity EXP", &["DivExp"], PERCENT, ""),
    // Library.
    simple("lib", "Library Checkout Speed", &["Lib"], PERCENT, ""),
    simple("vip", "VIP Library Points", &["VIP"], PERCENT, ""),
    // Farming.
    simple("farmexp", "Farming EXP", &["zFarmExp"], PERCENT, ""),
    YellowGoal {
        id: "cropevo",
        name: "Crop Evolution Chance",
        categories: &["zCropEvo", "zCropEvoSumm"],
        unit: Unit::Multiplier,
        note: "Combines direct Crop Evolution Chance with Nyanborgir scaled by the selected \
               character’s Summoning level.",
        value: Value::CropEvolution,
    },
    simple(
        "goldfood",
        "Golden Food Bonuses",
        &["zGoldFood"],
        PERCENT,
        "",
    ),
    // Sneaking, summoning and the W7 systems.
    simple("sneakexp", "Sneaking EXP", &["zSneakExp"], PERCENT, ""),
    simple("jade", "Jade Gain", &["zJade"], PERCENT, ""),
    simple(
        "sumess",
        "Summoning Essence Gain",
        &["zSumEss"],
        PERCENT,
        "",
    ),
    simple("sumexp", "Summoning EXP", &["zSummonExp"], PERCENT, ""),
    simple("splkexp", "Spelunking EXP", &["SplkExp"], PERCENT, ""),
    simple(
        "splkupg",
        "Lower Spelunking Costs",
        &["SplkUpg"],
        PERCENT,
        "",
    ),
    simple("splkpow", "Spelunking Power", &["SplkPOW"], PERCENT, ""),
    simple(
        "splkamb",
        "Amber Gain from Spelunking",
        &["SplkAmb"],
        PERCENT,
        "",
    ),
    simple("researchxp", "Research EXP", &["ResearchXP"], PERCENT, ""),
    simple("minecurr", "Mine Currency Gain", &["MineCurr"], PERCENT, ""),
    simple(
        "polyref",
        "Polymer Refinery Speed",
        &["PolyRefSpd"],
        PERCENT,
        "",
    ),
    // Cash & misc.
    simple("cash", "Cash from Monsters", &["Cash"], PERCENT, ""),
    simple("critter", "Critters from Traps", &["Critter"], PERCENT, ""),
    simple("tdpts", "Tower Defence Points", &["TDpts"], PERCENT, ""),
];

/// The goal with `id`, or `None` for an id the page does not define.
pub fn goal(id: &str) -> Option<&'static YellowGoal> {
    GOALS.iter().find(|goal| goal.id == id)
}

/// `bonusMultiCook`: what one mastery level multiplies a meal's contribution by.
pub fn bonus_multi_cook(level: f64) -> f64 {
    1.0 + level / (level + 5.0)
}

/// One character the stat goals scale with.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct MealCharacter {
    pub name: String,
    /// `Lv0_n[16]`: the character's Farming level.
    pub farming: f64,
    /// `Lv0_n[18]`: the character's Summoning level.
    pub summoning: f64,
}

/// One unlocked kitchen, from the save's `Cooking` rows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kitchen {
    /// `Cooking[n][6]`.
    pub speed: f64,
    /// `Cooking[n][7]`.
    pub fire: f64,
    /// `Cooking[n][8]`.
    pub luck: f64,
    /// Whether a gem purchase doubled or tripled the kitchen's rate.
    pub gem: bool,
}

/// `kitchenCookingAggregate`: what the Cabbage meals buy across the kitchens.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Kitchens {
    /// How many kitchens are unlocked.
    pub count: usize,
    /// The average Speed, Fire and Luck levels over the kitchens.
    pub average_levels: f64,
    /// The least and most `steps` any one kitchen holds.
    pub min_steps: f64,
    pub max_steps: f64,
    /// The rate-weighted `steps` the meal and recipe goals scale Cabbage by.
    pub meal_steps: f64,
    pub recipe_steps: f64,
}

impl Kitchens {
    /// `kitchenCookingAggregate` for the given kitchens.
    fn aggregate(kitchens: &[Kitchen]) -> Self {
        let mut summary = Kitchens {
            count: kitchens.len(),
            min_steps: f64::INFINITY,
            ..Kitchens::default()
        };
        let mut total_base = 0.0;
        let mut meal_weighted = 0.0;
        let mut recipe_base = 0.0;
        let mut recipe_weighted = 0.0;
        let mut total_levels = 0.0;
        for kitchen in kitchens {
            let levels = kitchen.speed + kitchen.fire + kitchen.luck;
            let steps = (levels / 10.0).floor();
            let gem = if kitchen.gem { 3.0 } else { 1.0 };
            let recipe_gem = if kitchen.gem { 2.0 } else { 1.0 };
            let meal_base = gem * (1.0 + kitchen.speed / 10.0);
            let recipe_rate = recipe_gem * (1.0 + kitchen.fire / 10.0);
            total_base += meal_base;
            meal_weighted += meal_base * steps;
            recipe_base += recipe_rate;
            recipe_weighted += recipe_rate * steps;
            total_levels += levels;
            summary.min_steps = summary.min_steps.min(steps);
            summary.max_steps = summary.max_steps.max(steps);
        }
        if summary.count == 0 {
            summary.min_steps = 0.0;
        } else {
            summary.average_levels = total_levels / summary.count as f64;
        }
        if total_base > 0.0 {
            summary.meal_steps = meal_weighted / total_base;
        }
        if recipe_base > 0.0 {
            summary.recipe_steps = recipe_weighted / recipe_base;
        }
        summary
    }
}

/// One save's meal mastery state, as the yellow point allocator reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Yellow {
    /// `CookMaster[0][meal]`: the yellow points each meal holds.
    pub mastery: Vec<f64>,
    /// `Meals[0][meal]`: the meal levels.
    pub levels: Vec<f64>,
    /// `ribbonBonusAt(28 + meal)` per meal.
    pub ribbons: Vec<f64>,
    /// `cookingMealMulti(save).val`.
    pub cook_multi: f64,
    /// `yellowPointBudget(save)`: every yellow point the account holds.
    pub budget: f64,
    /// The characters the stat goals scale with.
    pub characters: Vec<MealCharacter>,
    /// The account's unlocked kitchens.
    pub kitchens: Vec<Kitchen>,
    /// `eventShopOwned(53)`: the event shop perk the meal cook goal reads.
    pub event_53: bool,
}

impl Yellow {
    /// `kitchenCookingAggregate` over the save's kitchens.
    pub fn kitchens(&self) -> Kitchens {
        Kitchens::aggregate(&self.kitchens)
    }

    /// The goals with at least two leveled meals in their categories: the ones
    /// the page offers.
    pub fn goals(&self) -> Vec<&'static YellowGoal> {
        GOALS.iter().filter(|goal| self.offers(goal)).collect()
    }

    /// Whether the page offers a goal: two leveled meals leave something to
    /// compare, and one does not.
    pub fn offers(&self, goal: &YellowGoal) -> bool {
        self.meals(goal).len() > 1
    }

    /// The first offered goal, which the page selects when none is chosen.
    pub fn default_goal(&self) -> Option<&'static YellowGoal> {
        self.goals().into_iter().next()
    }

    /// The meals `optimizeForGoal` compares for a goal: a saved level above
    /// zero and a per-level bonus above zero.
    fn meals(&self, goal: &YellowGoal) -> Vec<Relevant> {
        let mut relevant = Vec::new();
        for (index, meal) in meals().iter().enumerate() {
            if !goal.categories.contains(&meal.category.as_str()) {
                continue;
            }
            let level = self.levels.get(index).copied().unwrap_or(0.0);
            if level <= 0.0 || meal.bonus <= 0.0 {
                continue;
            }
            let ribbon = self.ribbons.get(index).copied().unwrap_or(1.0);
            relevant.push(Relevant {
                index,
                meal,
                base_weight: self.cook_multi * ribbon * level * meal.bonus,
            });
        }
        relevant
    }

    /// The character's Farming steps: `ceil((lv + 1) / 50)`.
    fn farm_steps(&self, character: usize) -> f64 {
        let level = self
            .characters
            .get(character)
            .map(|character| character.farming)
            .unwrap_or(0.0);
        ((level + 1.0) / 50.0).ceil()
    }

    /// The character's Summoning steps: `ceil((lv + 1) / 50)`.
    fn summon_steps(&self, character: usize) -> f64 {
        let level = self
            .characters
            .get(character)
            .map(|character| character.summoning)
            .unwrap_or(0.0);
        ((level + 1.0) / 50.0).ceil()
    }

    /// The goal's value for the given category totals.
    fn value(&self, goal: &YellowGoal, totals: &[f64], character: usize) -> f64 {
        let total = |index: usize| totals.get(index).copied().unwrap_or(0.0);
        match goal.value {
            Value::Total => total(0),
            Value::MealCooking => {
                let event = if self.event_53 { 2.0 } else { 1.0 };
                let kitchens = self.kitchens();
                event
                    * (1.0 + total(0) / 100.0)
                    * (1.0 + total(1) * self.farm_steps(character) / 100.0)
                    * (1.0 + total(2) * kitchens.meal_steps / 100.0)
            }
            Value::RecipeCooking => {
                let kitchens = self.kitchens();
                (1.0 + total(0) / 100.0) * (1.0 + total(1) * kitchens.recipe_steps / 100.0)
            }
            Value::CropEvolution => {
                (1.0 + total(0) / 100.0) * (1.0 + total(1) * self.summon_steps(character) / 100.0)
            }
        }
    }

    /// The category totals one allocation produces.
    fn totals(&self, goal: &YellowGoal, relevant: &[Relevant], levels: &[f64]) -> Vec<f64> {
        let mut totals = vec![0.0; goal.categories.len()];
        for (row, meal) in relevant.iter().enumerate() {
            let Some(position) = goal
                .categories
                .iter()
                .position(|category| *category == meal.meal.category.as_str())
            else {
                continue;
            };
            totals[position] += meal.base_weight * bonus_multi_cook(levels[row]);
        }
        totals
    }

    /// `optimizeForGoal`: the budget spent across the goal's meals.
    ///
    /// The request is clamped to `[0, budget]` and floored to whole points,
    /// because the page spends one point per iteration. A goal the page does
    /// not define, or one with no leveled meals, has no plan.
    pub fn allocation(&self, goal_id: &str, budget: f64, character: usize) -> Option<Allocation> {
        let goal = goal(goal_id)?;
        let relevant = self.meals(goal);
        let requested = budget;
        let budget = if budget.is_finite() {
            budget.max(0.0).min(self.budget).floor()
        } else {
            0.0
        };
        let mut levels = vec![0.0; relevant.len()];
        if !relevant.is_empty() {
            let mut spent = 0.0;
            while spent < budget {
                let totals = self.totals(goal, &relevant, &levels);
                let current = self.value(goal, &totals, character);
                let mut best: Option<(usize, f64)> = None;
                for (row, meal) in relevant.iter().enumerate() {
                    let mut test = totals.clone();
                    let Some(position) = goal
                        .categories
                        .iter()
                        .position(|category| *category == meal.meal.category.as_str())
                    else {
                        continue;
                    };
                    let delta = meal.base_weight
                        * (bonus_multi_cook(levels[row] + 1.0) - bonus_multi_cook(levels[row]));
                    test[position] += delta;
                    let gain = self.value(goal, &test, character) - current;
                    if best.is_none_or(|(_, best_gain)| gain > best_gain) {
                        best = Some((row, gain));
                    }
                }
                let Some((row, _)) = best else {
                    break;
                };
                levels[row] += 1.0;
                spent += 1.0;
            }
        }

        let totals = self.totals(goal, &relevant, &levels);
        let current_totals = self.totals(goal, &relevant, &self.saved_levels(&relevant));
        let mut rows: Vec<MealRow> = relevant
            .iter()
            .enumerate()
            .map(|(row, meal)| {
                let allocated = levels[row];
                let current = self.mastery.get(meal.index).copied().unwrap_or(0.0);
                MealRow {
                    index: meal.index,
                    name: meal.meal.name.clone(),
                    effect: meal.meal.effect.clone(),
                    category: meal.meal.category.clone(),
                    level: self.levels.get(meal.index).copied().unwrap_or(0.0),
                    bonus: meal.meal.bonus,
                    base_weight: meal.base_weight,
                    current,
                    allocated,
                    mastery: bonus_multi_cook(allocated),
                }
            })
            .collect();
        // The page sorts by the optimal points, keeping the meal order on ties.
        rows.sort_by(|left, right| right.allocated.total_cmp(&left.allocated));
        let spent: f64 = levels.iter().sum();
        Some(Allocation {
            goal,
            character,
            available: self.budget,
            requested,
            spent,
            value: self.value(goal, &totals, character),
            current_value: self.value(goal, &current_totals, character),
            unspent: (budget - spent).max(0.0),
            rows,
        })
    }

    /// The mastery levels the save holds for the goal's meals.
    fn saved_levels(&self, relevant: &[Relevant]) -> Vec<f64> {
        relevant
            .iter()
            .map(|meal| self.mastery.get(meal.index).copied().unwrap_or(0.0))
            .collect()
    }
}

/// A meal the goal can spend on.
#[derive(Clone, Debug)]
struct Relevant {
    /// Its `Meals[0]` index.
    pub index: usize,
    pub meal: &'static Meal,
    /// `cookMulti × ribbon × meal level × per-level bonus`.
    pub base_weight: f64,
}

/// One row of the allocation table.
#[derive(Clone, Debug, PartialEq)]
pub struct MealRow {
    /// The meal's `Meals[0]` index.
    pub index: usize,
    pub name: String,
    /// What the meal grants, from `MealINFO`.
    pub effect: String,
    /// The `MealINFO` category the goal reads it under.
    pub category: String,
    /// The saved meal level.
    pub level: f64,
    /// The bonus one meal level adds.
    pub bonus: f64,
    /// `cookMulti × ribbon × meal level × bonus`.
    pub base_weight: f64,
    /// `CookMaster[0][index]`: the yellow points the meal holds now.
    pub current: f64,
    /// The points the plan gives the meal.
    pub allocated: f64,
    /// `bonusMultiCook(allocated)`.
    pub mastery: f64,
}

/// The plan for one goal: the budget spent across its meals.
#[derive(Clone, Debug, PartialEq)]
pub struct Allocation {
    pub goal: &'static YellowGoal,
    /// The character the stat goals scaled with.
    pub character: usize,
    /// `yellowPointBudget(save)`: every point the account holds.
    pub available: f64,
    /// The budget asked for, before clamping.
    pub requested: f64,
    /// The points the plan spends; never more than [`Self::available`].
    pub spent: f64,
    /// The goal's value at the planned levels.
    pub value: f64,
    /// The goal's value at the saved mastery levels.
    pub current_value: f64,
    /// Points left for other categories.
    pub unspent: f64,
    /// One row per meal, most points first.
    pub rows: Vec<MealRow>,
}

impl Allocation {
    /// Whether the request had to be clamped to a valid budget.
    pub fn rejected(&self) -> bool {
        !self.requested.is_finite()
            || self.requested < 0.0
            || self.requested.floor() > self.available
    }

    /// The character the plan scaled with, named.
    pub fn character_name(&self, yellow: &Yellow) -> String {
        yellow
            .characters
            .get(self.character)
            .map(|character| character.name.clone())
            .unwrap_or_else(|| format!("Character {}", self.character + 1))
    }

    /// `(value − current_value) / current_value`, as a percentage.
    pub fn improvement(&self) -> f64 {
        if self.current_value > 0.0 {
            (self.value - self.current_value) / self.current_value * 100.0
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two TotDmg meals and two Seff meals, so both goals are offered.
    fn yellow() -> Yellow {
        let mut levels = vec![0.0; meals().len()];
        levels[0] = 100.0; // Turkey of Thank, TotDmg, 2% per level
        levels[42] = 100.0; // Onion, TotDmg, 3%
        levels[6] = 50.0; // Corn, Seff, 2%
        levels[27] = 50.0; // Riceball, Seff, 3%
        let mut mastery = vec![0.0; meals().len()];
        mastery[0] = 3.0;
        Yellow {
            mastery,
            levels,
            ribbons: vec![1.0; meals().len()],
            cook_multi: 1.0,
            budget: 12.0,
            characters: vec![MealCharacter {
                name: "Ana".into(),
                farming: 99.0,
                summoning: 49.0,
            }],
            kitchens: Vec::new(),
            event_53: false,
        }
    }

    #[test]
    fn the_meal_factor_is_the_pages_curve() {
        assert_eq!(bonus_multi_cook(0.0), 1.0);
        assert_eq!(bonus_multi_cook(5.0), 1.5);
        assert_eq!(bonus_multi_cook(20.0), 1.8);
    }

    #[test]
    fn a_goal_needs_two_leveled_meals_to_be_offered() {
        let yellow = yellow();
        let offered: Vec<&str> = yellow.goals().iter().map(|goal| goal.id).collect();
        assert_eq!(offered, ["seff", "totdmg"]);
        assert!(
            yellow.allocation("crit", 5.0, 0).is_some(),
            "the page still plans a goal with one meal"
        );
        assert!(yellow.allocation("chocolate", 5.0, 0).is_none());
    }

    #[test]
    fn the_plan_spends_the_budget_and_nothing_more() {
        let yellow = yellow();
        let plan = yellow
            .allocation("totdmg", 5.0, 0)
            .expect("totdmg is offered");
        assert_eq!(plan.spent, 5.0);
        assert_eq!(plan.unspent, 0.0);
        assert_eq!(plan.rows.iter().map(|row| row.allocated).sum::<f64>(), 5.0);
        // Onion's base weight is the larger of the two, so it takes the first
        // point and stays at the top of the table.
        assert_eq!(plan.rows[0].name, "Onion");
        assert!(plan.rows[0].allocated >= plan.rows[1].allocated);
        assert!(plan.value > plan.current_value);
    }

    #[test]
    fn an_invalid_budget_is_clamped_or_rejected() {
        let yellow = yellow();
        let over = yellow.allocation("totdmg", 40.0, 0).expect("totdmg");
        assert_eq!(over.spent, 12.0, "the plan cannot spend past the budget");
        assert!(over.rejected(), "an over-budget request is rejected");
        let negative = yellow.allocation("totdmg", -3.0, 0).expect("totdmg");
        assert_eq!(negative.spent, 0.0);
        assert!(negative.rejected());
        let nan = yellow.allocation("totdmg", f64::NAN, 0).expect("totdmg");
        assert_eq!(nan.spent, 0.0);
        assert!(nan.rejected());
        let valid = yellow.allocation("totdmg", 4.0, 0).expect("totdmg");
        assert!(!valid.rejected());
    }

    #[test]
    fn every_goal_names_the_categories_its_value_reads() {
        for goal in &GOALS {
            let expected = match goal.value {
                Value::Total => 1,
                Value::MealCooking => 3,
                Value::RecipeCooking | Value::CropEvolution => 2,
            };
            assert_eq!(
                goal.categories.len(),
                expected,
                "{} reads {expected} categories",
                goal.id
            );
        }
        // The order the value functions index into.
        assert_eq!(
            goal("mcook").expect("mcook").categories,
            ["Mcook", "zMealFarm", "KitchenEff"]
        );
        assert_eq!(
            goal("cropevo").expect("cropevo").categories,
            ["zCropEvo", "zCropEvoSumm"]
        );
    }

    #[test]
    fn every_category_in_the_goal_table_has_a_meal() {
        for goal in &GOALS {
            for category in goal.categories {
                assert!(
                    meals().iter().any(|meal| meal.category == *category),
                    "{}: no meal carries {category}",
                    goal.id
                );
            }
        }
    }

    #[test]
    fn the_kitchen_aggregate_weighs_each_kitchen_by_its_rate() {
        let kitchens = [
            Kitchen {
                speed: 20.0,
                fire: 10.0,
                luck: 0.0,
                gem: false,
            },
            Kitchen {
                speed: 0.0,
                fire: 30.0,
                luck: 0.0,
                gem: true,
            },
        ];
        let summary = Kitchens::aggregate(&kitchens);
        assert_eq!(summary.count, 2);
        assert_eq!(summary.average_levels, 30.0);
        assert_eq!(summary.min_steps, 3.0);
        assert_eq!(summary.max_steps, 3.0);
        // Meal: 3 × (1 + 20/10) = 9 and 1 × (1 + 0) = 1, both 3 steps.
        assert_eq!(summary.meal_steps, 3.0);
        // Recipe: 1 × (1 + 10/10) = 2 and 2 × (1 + 30/10) = 8, both 3 steps.
        assert_eq!(summary.recipe_steps, 3.0);
        assert_eq!(Kitchens::aggregate(&[]), Kitchens::default());
    }
}
