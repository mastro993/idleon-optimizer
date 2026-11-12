//! `optimizeKitchenReset`: the kitchen assignment search behind one reset batch.
//!
//! The reset queue cooks one batch per kitchen, so the whole layout has to be
//! decided before any of it lands. Whole kitchens are indivisible: the search
//! keeps the saved layout and every all-on-one-meal layout as candidates beside a
//! beam over the allowed meals, then relocates single kitchens and swaps pairs
//! around the leader. The final comparison also applies the Dream 11 discount,
//! which exactly one purchase can spend.
//!
//! The upstream ranks candidates by the chosen objective, then by the kitchen
//! speed their purchases reach, then by the stock progress they leave. The speed
//! term needs the meal-level speed engine this build does not have, so the entered
//! kitchen speed is constant across a batch and the ordering is the objective,
//! then the stock progress.

use super::{CostInputs, Kitchen, Pins, Problem, meal_level_cost, meal_requirement};
use std::cmp::Ordering;
use std::collections::HashMap;

/// The most kitchens Cooking defines.
pub const MAX_KITCHENS: usize = 10;

const DEFAULT_BEAM_WIDTH: usize = 8;
const MAX_BEAM_WIDTH: usize = 32;
const RELOCATION_ROUNDS: usize = 2;

/// How the reset picks the kitchens' meals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LayoutMode {
    /// Search for the assignment worth the most meal levels.
    #[default]
    Optimized,
    /// Cook exactly what the save has saved.
    Saved,
}

/// The layout inputs upstream keeps in `kitchenLayout`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct LayoutChoice {
    pub mode: LayoutMode,
    /// Kitchen index to meal index; `-1` keeps that kitchen idle.
    pub pins: Pins,
    /// Meals the reset never assigns.
    pub excluded: Vec<usize>,
    /// The beam width; 0 takes the upstream default of 8.
    pub beam_width: usize,
}

impl Default for LayoutChoice {
    fn default() -> Self {
        Self {
            mode: LayoutMode::Optimized,
            pins: Pins::new(),
            excluded: Vec::new(),
            beam_width: DEFAULT_BEAM_WIDTH,
        }
    }
}

/// What one reset batch produces and buys.
#[derive(Clone, Debug, PartialEq)]
pub struct Layout {
    /// The hours the batch cooks in every assigned kitchen.
    pub hours: f64,
    /// The meal levels the batch buys.
    pub ranks: f64,
    /// The meals to level, in purchase order (the discounted one first).
    pub purchase_order: Vec<usize>,
    /// Every meal the batch cooked or levelled.
    pub meals: Vec<LayoutMeal>,
    /// One row per unlocked kitchen.
    pub kitchens: Vec<LayoutKitchen>,
}

/// One meal's share of the batch.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutMeal {
    pub meal: usize,
    /// The level the batch started at.
    pub from: f64,
    /// The level its stock bought.
    pub to: f64,
    /// The stock the assigned kitchens cooked.
    pub produced: f64,
    /// The stock left after the purchases.
    pub stock: f64,
    /// How far the leftover stock is toward the next level.
    pub progress: f64,
}

/// One kitchen's assignment in the batch.
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutKitchen {
    /// The `Cooking` row index.
    pub index: usize,
    /// The meal the save had the kitchen on, or -1.
    pub saved_meal: i32,
    /// The meal the batch cooks, or -1 for idle.
    pub meal: i32,
    /// Whether the kitchen's meal was pinned.
    pub pinned: bool,
    /// The kitchen's speed at the panel's base speed.
    pub speed: f64,
    /// The stock this kitchen cooks for its meal.
    pub produced: f64,
}

/// One batch's inputs.
pub struct Input<'a> {
    /// The starting meal levels.
    pub levels: &'a [f64],
    /// The starting meal stocks.
    pub stocks: &'a [f64],
    /// Every kitchen the save carries, locked or not.
    pub kitchens: &'a [Kitchen],
    /// The hours the batch cooks in every assigned kitchen.
    pub hours: f64,
    /// The panel's kitchen speed at Speed Lv 0.
    pub base_speed: f64,
    /// The highest level a meal can reach.
    pub max_level: f64,
    pub cost: CostInputs,
    /// The Dream days a purchase can spend.
    pub discount_days: f64,
    pub choice: &'a LayoutChoice,
}

/// `optimizeKitchenReset`: the batch worth the most meal levels.
pub fn optimize(input: &Input) -> Result<Layout, Problem> {
    let kitchens: Vec<&Kitchen> = input
        .kitchens
        .iter()
        .filter(|kitchen| kitchen.unlocked)
        .collect();
    if kitchens.len() > MAX_KITCHENS {
        return Err(Problem::Kitchens);
    }
    let eligible: Vec<usize> = input
        .levels
        .iter()
        .enumerate()
        .filter(|(meal, level)| eligible(*meal, **level, input.max_level))
        .map(|(meal, _)| meal)
        .collect();
    let excluded = input.choice.excluded.as_slice();
    let allowed: Vec<usize> = eligible
        .iter()
        .copied()
        .filter(|meal| !excluded.contains(meal))
        .collect();
    for kitchen in &input.choice.pins {
        if !kitchens
            .iter()
            .any(|candidate| candidate.index == *kitchen.0)
        {
            return Err(Problem::PinnedKitchen);
        }
        let meal = *kitchen.1;
        if meal >= 0 && !allowed.contains(&(meal as usize)) {
            return Err(Problem::PinnedMeal);
        }
    }
    let rates: Vec<f64> = kitchens
        .iter()
        .map(|kitchen| kitchen.speed(input.base_speed))
        .collect();
    let mut search = Search {
        levels: input.levels,
        stocks: input.stocks,
        rates: &rates,
        hours: input.hours.max(0.0),
        max_level: input.max_level,
        cost: input.cost,
        discount_days: input.discount_days,
        dream_11: input.cost.dream_11,
        cache: HashMap::new(),
    };
    let saved: Vec<i32> = kitchens
        .iter()
        .map(|kitchen| match kitchen.saved_meal {
            meal if meal >= 0 && eligible.contains(&(meal as usize)) => meal,
            _ => -1,
        })
        .collect();
    let initial: Vec<i32> = kitchens
        .iter()
        .map(|kitchen| input.choice.pins.get(&kitchen.index).copied().unwrap_or(-1))
        .collect();
    let mut candidates: Vec<Placement> = Vec::new();
    if input.choice.mode == LayoutMode::Saved {
        candidates.push(search.make(&saved, -1));
    } else {
        let width = match input.choice.beam_width {
            0 => DEFAULT_BEAM_WIDTH,
            width => width.clamp(1, MAX_BEAM_WIDTH),
        };
        let mut order: Vec<usize> = (0..kitchens.len()).collect();
        order.sort_by(|a, b| rates[*b].total_cmp(&rates[*a]).then(a.cmp(b)));
        let mut beam = vec![search.make(&initial, -1)];
        for position in &order {
            if input.choice.pins.contains_key(&kitchens[*position].index) {
                continue;
            }
            let mut next: Vec<Placement> = Vec::new();
            for state in &beam {
                for meal in &allowed {
                    next.push(search.move_kitchen(state, *position, *meal as i32));
                }
            }
            next.sort_by(compare);
            if !next.is_empty() {
                next.truncate(width);
                beam = next;
            }
        }
        candidates.extend(beam);
        for meal in &allowed {
            let assignments: Vec<i32> = kitchens
                .iter()
                .map(|kitchen| {
                    input
                        .choice
                        .pins
                        .get(&kitchen.index)
                        .copied()
                        .unwrap_or(*meal as i32)
                })
                .collect();
            candidates.push(search.make(&assignments, -1));
        }
        let saved_legal = saved.iter().enumerate().all(|(position, meal)| {
            (*meal < 0 || allowed.contains(&(*meal as usize)))
                && input
                    .choice
                    .pins
                    .get(&kitchens[position].index)
                    .is_none_or(|pinned| *pinned == *meal)
        });
        if saved_legal {
            candidates.push(search.make(&saved, -1));
        }
        candidates.sort_by(compare);
        let mut leader = candidates.first().cloned();
        for _ in 0..RELOCATION_ROUNDS {
            let Some(state) = leader.clone() else { break };
            let mut neighbors = vec![state.clone()];
            for position in &order {
                if input.choice.pins.contains_key(&kitchens[*position].index) {
                    continue;
                }
                for meal in &allowed {
                    neighbors.push(search.move_kitchen(&state, *position, *meal as i32));
                }
                for other in &order {
                    if other <= position || input.choice.pins.contains_key(&kitchens[*other].index)
                    {
                        continue;
                    }
                    let first = search.move_kitchen(&state, *position, state.assignments[*other]);
                    let swapped = search.move_kitchen(&first, *other, state.assignments[*position]);
                    neighbors.push(swapped);
                }
            }
            neighbors.sort_by(compare);
            candidates.extend(neighbors.iter().take(width).cloned());
            if compare(&neighbors[0], &state) != Ordering::Less {
                break;
            }
            leader = neighbors.first().cloned();
        }
    }
    if candidates.is_empty() {
        candidates.push(search.make(&initial, -1));
    }
    let mut best: Option<Placement> = None;
    for candidate in &candidates {
        let mut can_purchase = false;
        if search.discount_days > 0.0 && search.dream_11 > 0.0 {
            for meal in &eligible {
                let mask = candidate.masks[*meal];
                let the_value = search.output(*meal, mask, true);
                if the_value.ranks <= 0.0 {
                    continue;
                }
                can_purchase = true;
                let discounted = search.replace(candidate, *meal, the_value, *meal as i32);
                if best
                    .as_ref()
                    .is_none_or(|best| compare(&discounted, best) == Ordering::Less)
                {
                    best = Some(discounted);
                }
            }
        }
        if !can_purchase
            && best
                .as_ref()
                .is_none_or(|best| compare(candidate, best) == Ordering::Less)
        {
            best = Some(candidate.clone());
        }
    }
    let best = match best {
        Some(best) => best,
        None => search.make(&initial, -1),
    };
    let mut purchase_order: Vec<usize> = eligible
        .iter()
        .copied()
        .filter(|meal| best.out[*meal].ranks > 0.0)
        .collect();
    if best.discounted_meal >= 0 {
        purchase_order
            .sort_by_key(|meal| (usize::from(*meal as i32 != best.discounted_meal), *meal));
    }
    let meals = (0..input.levels.len())
        .filter_map(|meal| {
            let out = best.out.get(meal)?;
            (out.produced > 0.0 || out.level > input.levels[meal]).then_some(LayoutMeal {
                meal,
                from: input.levels[meal],
                to: out.level,
                produced: out.produced,
                stock: out.stock,
                progress: out.progress,
            })
        })
        .collect();
    Ok(Layout {
        hours: search.hours,
        ranks: best.ranks,
        purchase_order,
        meals,
        kitchens: kitchens
            .iter()
            .enumerate()
            .map(|(position, kitchen)| {
                let meal = best.assignments[position];
                LayoutKitchen {
                    index: kitchen.index,
                    saved_meal: saved[position],
                    meal,
                    pinned: input.choice.pins.contains_key(&kitchen.index),
                    speed: rates[position],
                    produced: match meal >= 0 {
                        true => search.produced(meal as usize, rates[position]),
                        false => 0.0,
                    },
                }
            })
            .collect(),
    })
}

/// A meal the reset can raise: unlocked, below the cap and cookable.
fn eligible(meal: usize, level: f64, max_level: f64) -> bool {
    level >= 1.0 && level < max_level && meal_requirement(meal) > 0.0
}

/// One meal's outcome for one kitchen mask.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Output {
    level: f64,
    stock: f64,
    produced: f64,
    ranks: f64,
    progress: f64,
}

/// One complete kitchen assignment.
#[derive(Clone, Debug, PartialEq)]
struct Placement {
    assignments: Vec<i32>,
    masks: Vec<u32>,
    out: Vec<Output>,
    ranks: f64,
    progress: f64,
    discounted_meal: i32,
}

/// The upstream `compare`: more ranks first, then more stock progress.
fn compare(a: &Placement, b: &Placement) -> Ordering {
    b.ranks
        .total_cmp(&a.ranks)
        .then_with(|| b.progress.total_cmp(&a.progress))
}

/// The search's shared state: the starting meals, the per-kitchen rates and the
/// memo of every mask's outcome.
struct Search<'a> {
    levels: &'a [f64],
    stocks: &'a [f64],
    rates: &'a [f64],
    hours: f64,
    max_level: f64,
    cost: CostInputs,
    discount_days: f64,
    dream_11: f64,
    cache: HashMap<(usize, u32, bool), Output>,
}

impl Search<'_> {
    /// The stock one kitchen cooks for `meal` in the batch.
    fn produced(&self, meal: usize, rate: f64) -> f64 {
        let requirement = meal_requirement(meal);
        if requirement <= 0.0 {
            return 0.0;
        }
        self.hours * rate / requirement
    }

    /// `output`: what a meal's kitchen mask buys from its stock.
    fn output(&mut self, meal: usize, mask: u32, discount: bool) -> Output {
        let key = (meal, mask, discount);
        if let Some(output) = self.cache.get(&key) {
            return *output;
        }
        let requirement = meal_requirement(meal);
        let mut produced = 0.0;
        for (position, rate) in self.rates.iter().enumerate() {
            if mask & (1 << position) != 0 {
                produced += self.produced(meal, *rate);
            }
        }
        let mut stock = self.stocks.get(meal).copied().unwrap_or(0.0) + produced;
        let start = self.levels.get(meal).copied().unwrap_or(0.0);
        let mut level = start;
        while level >= 1.0 && level < self.max_level && requirement > 0.0 {
            let days = if discount && level == start {
                self.discount_days
            } else {
                0.0
            };
            let cost = meal_level_cost(level, self.cost, days);
            if stock < cost {
                break;
            }
            stock -= cost;
            level += 1.0;
        }
        let output = Output {
            level,
            stock,
            produced,
            ranks: level - start,
            progress: match level < self.max_level {
                true => (stock / meal_level_cost(level, self.cost, 0.0)).min(1.0),
                false => 0.0,
            },
        };
        self.cache.insert(key, output);
        output
    }

    /// `make`: the placement one assignment produces.
    fn make(&mut self, assignments: &[i32], discounted_meal: i32) -> Placement {
        let mut masks = vec![0u32; self.levels.len()];
        for (position, meal) in assignments.iter().enumerate() {
            if *meal >= 0 && (*meal as usize) < masks.len() {
                masks[*meal as usize] |= 1 << position;
            }
        }
        let out: Vec<Output> = (0..self.levels.len())
            .map(|meal| {
                let mask = masks[meal];
                self.output(meal, mask, meal as i32 == discounted_meal)
            })
            .collect();
        let ranks = out.iter().map(|output| output.ranks).sum();
        let progress = out.iter().map(|output| output.progress).sum();
        Placement {
            assignments: assignments.to_vec(),
            masks,
            out,
            ranks,
            progress,
            discounted_meal,
        }
    }

    /// `replace`: one meal's outcome swapped in, keeping the totals.
    fn replace(
        &mut self,
        state: &Placement,
        meal: usize,
        value: Output,
        discounted_meal: i32,
    ) -> Placement {
        let old = state.out[meal];
        let mut out = state.out.clone();
        out[meal] = value;
        Placement {
            assignments: state.assignments.clone(),
            masks: state.masks.clone(),
            ranks: state.ranks + value.ranks - old.ranks,
            progress: state.progress + value.progress - old.progress,
            out,
            discounted_meal,
        }
    }

    /// `move`: one kitchen pointed at `meal`, both meals re-evaluated.
    fn move_kitchen(&mut self, state: &Placement, position: usize, meal: i32) -> Placement {
        let old = state.assignments[position];
        if old == meal {
            return state.clone();
        }
        let mut masks = state.masks.clone();
        let mut assignments = state.assignments.clone();
        assignments[position] = meal;
        let mut next = Placement {
            assignments,
            masks: masks.clone(),
            out: state.out.clone(),
            ranks: state.ranks,
            progress: state.progress,
            discounted_meal: -1,
        };
        if old >= 0 {
            masks[old as usize] &= !(1 << position);
            let value = self.output(old as usize, masks[old as usize], false);
            next = self.replace(&next, old as usize, value, -1);
        }
        if meal >= 0 {
            masks[meal as usize] |= 1 << position;
            let value = self.output(meal as usize, masks[meal as usize], false);
            next = self.replace(&next, meal as usize, value, -1);
        }
        next.masks = masks;
        next
    }
}
