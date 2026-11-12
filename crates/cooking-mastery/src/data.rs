//! The static meal table the yellow point allocator reads.
//!
//! `script/extract-cooking-data.mjs` writes `data/meals.json` from the upstream
//! `MealINFO`, whose rows are
//! `[name, requirement, bonusPerLevel, effect, flavor, effectKey]`: the
//! allocator needs the per-level bonus, the effect text and the category
//! (`effectKey`) that `js/stats/systems/common/meal-impact.js` groups meals by.

use serde::Deserialize;
use std::sync::OnceLock;

const MEALS_JSON: &str = include_str!("../data/meals.json");

/// One meal's static row, by its `Meals[0]` index.
#[derive(Debug, Deserialize)]
pub struct Meal {
    /// `MealINFO[idx][0]`, underscores turned into spaces.
    pub name: String,
    /// `MealINFO[idx][2]`: the bonus one meal level adds, as a percentage.
    pub bonus: f64,
    /// `MealINFO[idx][5]`: the category the goal table groups the meal under.
    pub category: String,
    /// `MealINFO[idx][3]` with the game's markup stripped.
    pub effect: String,
}

#[derive(Debug, Deserialize)]
struct Table {
    meals: Vec<Meal>,
}

/// Every meal, in `MealINFO` order.
pub fn meals() -> &'static [Meal] {
    static TABLE: OnceLock<Table> = OnceLock::new();
    &TABLE
        .get_or_init(|| serde_json::from_str(MEALS_JSON).expect("meals.json is extracted"))
        .meals
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_meal_carries_a_category_and_a_bonus() {
        assert_eq!(meals().len(), 74);
        for (index, meal) in meals().iter().enumerate() {
            assert!(!meal.name.is_empty(), "meal {index} has no name");
            assert!(!meal.category.is_empty(), "meal {index} has no category");
            assert!(meal.bonus > 0.0, "meal {index} has no per-level bonus");
        }
        assert_eq!(meals()[0].name, "Turkey of Thank");
        assert_eq!(meals()[73].category, "MineCurr");
    }
}
