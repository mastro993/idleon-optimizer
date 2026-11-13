//! `ladleableMeals` and `nmlbFloorPlan`: how the reset splits the meals and how
//! to hold No Meal Left Behind on an expensive target.
//!
//! NMLB raises the lowest meal at Lv 2 or more below the max level, up to 150,
//! once per reset, and ties go to the highest meal number. A meal whose next level
//! fits in one day of kitchen time is ladleable and worth leveling directly; the
//! rest cost more than a day, so NMLB's free levels are the cheap way up. Keeping
//! every ladleable meal above the floor (the lowest target level) makes each daily
//! hit land on a target instead, and the rotation is the order NMLB then walks.

use super::{CostInputs, meal_level_cost, meal_requirement, nmlb_result_level, nmlb_target};

/// The rotation upstream predicts when the page does not ask for another length.
pub const ROTATION_DAYS: u32 = 14;

/// The kitchen state and rates the steering plan reads.
pub struct Input<'a> {
    pub levels: &'a [f64],
    pub stocks: &'a [f64],
    pub max_level: f64,
    pub cost: CostInputs,
    /// The total speed of every unlocked kitchen.
    pub speed: f64,
    /// Talent 148: ladle hours per ladle.
    pub ladle_multi: f64,
    /// `nmlbInfo(S).levels`: the free levels a reset awards.
    pub nmlb: f64,
    /// The kitchen hours one day carries: the passive day plus the claim's ladles.
    pub day_hours: f64,
    /// How many resets the rotation predicts.
    pub rotation_days: u32,
}

/// `ladleableMeals`: one meal's class for the day.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MealClass {
    pub meal: usize,
    pub level: f64,
    /// Discovered, below the cap and cookable.
    pub open: bool,
    /// NMLB can pick it: open, Lv 2 or more and below 150.
    pub nmlb_eligible: bool,
    /// The ladles its next level needs, or infinity when it is closed.
    pub next_ladles: f64,
    /// Those ladles in kitchen hours.
    pub next_hours: f64,
    /// The next level fits in one day of kitchen time.
    pub ladleable: bool,
}

/// One meal below the floor that the ladles have to raise first.
#[derive(Clone, Debug, PartialEq)]
pub struct Blocker {
    pub meal: usize,
    pub level: f64,
    /// The level the meal must reach for NMLB to leave the floor alone.
    pub need: f64,
    pub levels: f64,
    pub ladles: f64,
    pub hours: f64,
    /// The meal is a target itself, so another target has to rise first.
    pub target: bool,
}

/// One target meal: its value per NMLB hit and what steering costs.
#[derive(Clone, Debug, PartialEq)]
pub struct Target {
    pub meal: usize,
    pub level: f64,
    /// The ladles its next level needs.
    pub next_ladles: f64,
    /// The ladles NMLB's free levels save at one hit.
    pub ladles_saved: f64,
    /// The reset the held-floor rotation reaches it, or `None` when later.
    pub rotation_day: Option<u32>,
    /// Tonight's NMLB pick.
    pub current: bool,
    /// The lowest target: what holding the floor aims at.
    pub floor: bool,
    pub ladleable_blockers: usize,
    /// Blockers that are targets themselves, so they have to rise first.
    pub target_blockers: usize,
    /// The ladles the ladleable blockers cost.
    pub steer_ladles: f64,
    /// Every blocker is ladleable.
    pub feasible: bool,
    pub fits_in_day: bool,
}

/// One reset of the held-floor rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct RotationDay {
    pub day: u32,
    pub meal: usize,
    pub from: f64,
    pub to: f64,
    pub ladles_saved: f64,
}

/// `nmlbFloorPlan`: the meal split, the floor, its blockers and the rotation.
#[derive(Clone, Debug, PartialEq)]
pub struct Floor {
    pub day_hours: f64,
    pub classes: Vec<MealClass>,
    pub current_pick: Option<usize>,
    pub current_on_target: bool,
    pub current_saved: f64,
    pub floor_meal: Option<usize>,
    pub floor_level: Option<f64>,
    pub floor_saved: f64,
    /// The floor's ladleable blockers, lowest first.
    pub blockers: Vec<Blocker>,
    pub blocker_ladles: f64,
    pub blocker_hours: f64,
    pub fits_in_day: bool,
    /// Every target, most ladles saved first.
    pub targets: Vec<Target>,
    pub rotation: Vec<RotationDay>,
    pub ladleable_count: usize,
    pub target_count: usize,
}

/// The floor's blockers, split by whether the ladles can raise them.
struct Blockers {
    ladleable: Vec<Blocker>,
    target: Vec<Blocker>,
    ladles: f64,
}

/// `ladlesForMealLevel`: the ladles raising `meal` one level from `level`, spending
/// `stock` first. The first-purchase discount is off, as upstream prices the plan.
fn ladles(meal: usize, level: f64, stock: f64, input: &Input) -> f64 {
    let cost = meal_level_cost(level, input.cost, 0.0);
    let per_ladle = per_ladle(meal, input);
    let deficit = (cost - stock).max(0.0);
    match per_ladle > 0.0 {
        true => deficit / per_ladle,
        false => f64::INFINITY,
    }
}

/// `mealsPerLadle`: the stock one ladle cooks.
fn per_ladle(meal: usize, input: &Input) -> f64 {
    let requirement = meal_requirement(meal);
    match requirement > 0.0 {
        true => input.speed * input.ladle_multi / requirement,
        false => 0.0,
    }
}

/// `_ladlesForLevels`: the ladles raising `meal` from `from` to `to`.
fn ladles_for_levels(meal: usize, from: f64, to: f64, stock: f64, input: &Input) -> f64 {
    let mut total = 0.0;
    let mut stock = stock;
    let mut level = from;
    while level < to {
        total += ladles(meal, level, stock, input);
        stock = (stock - meal_level_cost(level, input.cost, 0.0)).max(0.0);
        level += 1.0;
    }
    total
}

/// `_steerBlockers`: the meals NMLB would pick before `x`.
fn steer_blockers(levels: &[f64], x: usize, max_level: f64) -> Vec<usize> {
    let level = levels[x];
    if !(level >= 2.0 && level < max_level) {
        return Vec::new();
    }
    (0..levels.len())
        .filter(|meal| {
            let other = levels[*meal];
            *meal != x
                && other >= 2.0
                && other < max_level
                && other < if *meal > x { level + 1.0 } else { level }
        })
        .collect()
}

/// `_nmlbPickAmong`: the lowest eligible level among the allowed meals.
fn pick_among(levels: &[f64], max_level: f64, allowed: impl Fn(usize) -> bool) -> Option<usize> {
    let mut pick = None;
    let mut lowest = f64::INFINITY;
    for (meal, level) in levels.iter().enumerate() {
        if allowed(meal) && *level >= 2.0 && *level <= lowest && *level < max_level {
            pick = Some(meal);
            lowest = *level;
        }
    }
    pick
}

/// `ladleableMeals`: every meal's class for the day.
pub fn classes(input: &Input) -> Vec<MealClass> {
    (0..input.levels.len())
        .map(|meal| {
            let level = input.levels[meal];
            let open = level >= 1.0 && level < input.max_level && meal_requirement(meal) > 0.0;
            let stock = input.stocks.get(meal).copied().unwrap_or(0.0);
            let next_ladles = match open {
                true => ladles(meal, level, stock, input),
                false => f64::INFINITY,
            };
            let next_hours = match open {
                true => next_ladles * input.ladle_multi,
                false => f64::INFINITY,
            };
            MealClass {
                meal,
                level,
                open,
                nmlb_eligible: open && (2.0..super::NMLB_LEVEL_CAP).contains(&level),
                next_ladles,
                next_hours,
                ladleable: open && next_hours <= input.day_hours,
            }
        })
        .collect()
}

/// The ladles NMLB's free levels save on `meal` at one hit.
fn saved_for(meal: usize, from: f64, gain: f64, input: &Input) -> f64 {
    let to = nmlb_result_level(from, gain.max(1.0), input.max_level);
    ladles_for_levels(meal, from, to, 0.0, input)
}

/// The meals `x` needs raised before NMLB leaves it alone.
fn blockers_for(x: usize, is_target: &[bool], input: &Input) -> Blockers {
    let mut ladleable = Vec::new();
    let mut target = Vec::new();
    let mut ladles = 0.0;
    for meal in steer_blockers(input.levels, x, input.max_level) {
        let level = input.levels[meal];
        let need = input.max_level.min(if meal > x {
            input.levels[x] + 1.0
        } else {
            input.levels[x]
        });
        let cost = ladles_for_levels(
            meal,
            level,
            need,
            input.stocks.get(meal).copied().unwrap_or(0.0),
            input,
        );
        let blocker = Blocker {
            meal,
            level,
            need,
            levels: need - level,
            ladles: cost,
            hours: cost * input.ladle_multi,
            target: is_target[meal],
        };
        match blocker.target {
            true => target.push(blocker),
            false => {
                ladles += cost;
                ladleable.push(blocker);
            }
        }
    }
    // Upstream walks the blockers lowest level first, then highest meal number.
    ladleable.sort_by(|a, b| {
        a.level
            .partial_cmp(&b.level)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.meal.cmp(&a.meal))
    });
    Blockers {
        ladleable,
        target,
        ladles,
    }
}

/// `nmlbFloorPlan`: the split, tonight's pick, the floor and the rotation that
/// holding it predicts.
pub fn floor_plan(input: &Input) -> Floor {
    let classes = classes(input);
    let is_target: Vec<bool> = classes
        .iter()
        .map(|class| class.nmlb_eligible && !class.ladleable)
        .collect();
    let current_pick = nmlb_target(input.levels, input.max_level);
    let floor_meal = pick_among(input.levels, input.max_level, |meal| is_target[meal]);
    let gain = input.nmlb.max(0.0);
    let saved = |meal: usize, from: f64| saved_for(meal, from, gain, input);

    // The rotation assumes only the floor's blockers are raised: NMLB then cycles
    // through the targets it can reach.
    let mut rotation_levels = input.levels.to_vec();
    let mut rotation = Vec::new();
    let mut rotation_day: Vec<Option<u32>> = vec![None; input.levels.len()];
    let days = input.rotation_days.clamp(1, 365);
    for day in 1..=days {
        if gain <= 0.0 {
            break;
        }
        let Some(meal) = pick_among(&rotation_levels, input.max_level, |meal| is_target[meal])
        else {
            break;
        };
        let from = rotation_levels[meal];
        let to = nmlb_result_level(from, gain, input.max_level);
        if to <= from {
            break;
        }
        rotation.push(RotationDay {
            day,
            meal,
            from,
            to,
            ladles_saved: saved(meal, from),
        });
        rotation_day[meal].get_or_insert(day);
        rotation_levels[meal] = to;
    }

    let mut targets = Vec::new();
    for meal in 0..input.levels.len() {
        if !is_target[meal] {
            continue;
        }
        let blockers = blockers_for(meal, &is_target, input);
        targets.push(Target {
            meal,
            level: input.levels[meal],
            next_ladles: classes[meal].next_ladles,
            ladles_saved: saved(meal, input.levels[meal]),
            rotation_day: rotation_day[meal],
            current: current_pick == Some(meal),
            floor: floor_meal == Some(meal),
            ladleable_blockers: blockers.ladleable.len(),
            target_blockers: blockers.target.len(),
            steer_ladles: blockers.ladles,
            feasible: blockers.target.is_empty(),
            fits_in_day: blockers.target.is_empty()
                && blockers.ladles * input.ladle_multi <= input.day_hours,
        });
    }
    targets.sort_by(|a, b| {
        b.ladles_saved
            .partial_cmp(&a.ladles_saved)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.meal.cmp(&b.meal))
    });

    let floor_blockers = floor_meal
        .map(|meal| blockers_for(meal, &is_target, input))
        .unwrap_or(Blockers {
            ladleable: Vec::new(),
            target: Vec::new(),
            ladles: 0.0,
        });
    Floor {
        day_hours: input.day_hours,
        current_on_target: current_pick.is_some_and(|meal| is_target[meal]),
        current_saved: current_pick.map_or(0.0, |meal| saved(meal, input.levels[meal])),
        floor_level: floor_meal.map(|meal| input.levels[meal]),
        floor_saved: floor_meal.map_or(0.0, |meal| saved(meal, input.levels[meal])),
        blocker_ladles: floor_blockers.ladles,
        blocker_hours: floor_blockers.ladles * input.ladle_multi,
        fits_in_day: floor_blockers.ladles * input.ladle_multi <= input.day_hours,
        blockers: floor_blockers.ladleable,
        ladleable_count: classes.iter().filter(|class| class.ladleable).count(),
        target_count: targets.len(),
        current_pick,
        floor_meal,
        targets,
        rotation,
        classes,
    }
}
