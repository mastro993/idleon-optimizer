//! `planMealLeveling`: the daily ladle investment behind the Kitchens &
//! Jellyfish tab.
//!
//! Each day the plan claims the guaranteed ladles, donates the policy's share to
//! the Blue Jellyfish, spends what is left on the cheapest meal level (whole
//! ladles first, then passive kitchen time), banks the leftover into the meal it
//! was cooking, lets No Meal Left Behind raise its target, and finally buys the
//! levels the reset batch's stock covers. Sub-level stock carries forward, so
//! failing to buy a rank today does not force a donation.
//!
//! Upstream picks the best speed-per-hour meal when the objective is kitchen
//! speed; that ranking reads the meal-level speed engine this build does not have,
//! so the plan always spends like upstream's `cheapest` strategy. Steering, the
//! NMLB floor guard and the new-donation search belong to the NMLB planner.

use super::layout::{self, Layout, LayoutMeal};
use super::{
    CostInputs, DEFAULT_DAYS, MAX_DAYS, Problem, Reset, donation, meal_level_cost, meal_name,
    meal_requirement, nmlb_result_level, nmlb_target,
};

/// What a daily reset plan is asked to do.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Inputs {
    /// The horizon in resets, clamped to 1..=[`MAX_DAYS`].
    pub days: u32,
    /// The hours until the next reset, which bounds the first day's claim.
    pub hours_to_reset: f64,
    /// The guaranteed ladles a day the checked characters claim.
    pub ladles_per_day: f64,
    /// The ladles the save already carries.
    pub start_ladles: f64,
    /// Talent 148: ladle hours per ladle.
    pub ladle_multi: f64,
    /// Whether the kitchens cook passively for 24h a day.
    pub passive: bool,
    /// Whether only meals whose next level fits in a day of kitchen time are
    /// levelled directly, as upstream defaults to.
    pub ladleable_only: bool,
    /// The highest meal level the Cooking panel allows.
    pub max_level: f64,
    /// The Cooking speed /hr a kitchen has at Speed Lv 0.
    pub base_speed: f64,
    pub cost: CostInputs,
    pub policy: Policy,
    /// `share`: the fraction of the day's guaranteed income donated.
    pub income_fraction: f64,
    /// `fixed`: the ladles donated a day.
    pub donation_per_day: f64,
    /// The fraction of the owned ladles donated up front.
    pub owned_fraction: f64,
    pub layout: layout::LayoutChoice,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            days: DEFAULT_DAYS,
            hours_to_reset: 24.0,
            ladles_per_day: 0.0,
            start_ladles: 0.0,
            ladle_multi: 1.0,
            passive: true,
            ladleable_only: true,
            max_level: 30.0,
            base_speed: 10.0,
            cost: CostInputs::default(),
            policy: Policy::None,
            income_fraction: 1.0,
            donation_per_day: 0.0,
            owned_fraction: 1.0,
            layout: layout::LayoutChoice::default(),
        }
    }
}

/// How much of the daily ladle income the plan donates.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Policy {
    /// Keep every ladle for meals.
    #[default]
    None,
    /// Donate every guaranteed ladle.
    All,
    /// Donate a fraction of the guaranteed income.
    Share,
    /// Donate a fixed number of ladles a day.
    Fixed,
}

/// One phase's kind.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PhaseKind {
    /// The owned ladles spent before the first claim.
    #[default]
    Now,
    /// The claim's ladles, spent as they arrive.
    Claim,
    /// The reset batch: the Blue Jellyfish grant cooking in every kitchen.
    Jelly,
}

/// One level bought in spend order.
#[derive(Clone, Debug, PartialEq)]
pub struct Spend {
    pub meal: usize,
    pub from: f64,
    pub to: f64,
    /// The levels this row bought, including the stock-covered ones folded in.
    pub levels: usize,
    pub ladles: f64,
    pub hours: f64,
    /// Ladles spent through this row since the plan started.
    pub cumulative_ladles: f64,
    /// Whether the row raises the Turkey stock past the cached bonus.
    pub map_refresh: bool,
}

/// One meal's purchases over a day.
#[derive(Clone, Debug, PartialEq)]
pub struct MealGain {
    pub meal: usize,
    pub from: f64,
    pub to: f64,
    pub levels: usize,
    pub ladles: f64,
    pub hours: f64,
}

/// Kitchen time the plan could not spend, banked into one meal's stock.
#[derive(Clone, Debug, PartialEq)]
pub struct Saving {
    pub meal: usize,
    /// The level the stock is working toward.
    pub to_level: f64,
    pub hours: f64,
    pub ladles: f64,
    /// How far the banked stock is toward that level.
    pub progress: f64,
    /// No ladleable level was left, so the stock is NMLB-target progress.
    pub target: bool,
    pub map_refresh: bool,
}

/// One phase of a reset day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Phase {
    pub kind: PhaseKind,
    /// The hours the phase's income covers: the reset countdown on day one.
    pub window_hours: f64,
    /// The guaranteed ladles the phase may spend.
    pub ladles: f64,
    /// The ladles the Post Office 19c roll may add; equal without claimers.
    pub expected_ladles: f64,
    /// The passive kitchen hours the phase's window carries.
    pub passive_hours: f64,
    /// The reset grant cooking in every kitchen, for the jelly phase.
    pub jelly_hours: f64,
    /// The purchases in spend order.
    pub rows: Vec<Spend>,
    pub ladles_used: f64,
    pub passive_hours_used: f64,
    pub levels_bought: usize,
    pub saving: Option<Saving>,
    /// The batch's meals still below the cap, for the jelly phase.
    pub savings: Vec<LayoutMeal>,
    pub layout: Option<Layout>,
    /// The meal the grant banks into, for a grant with no layout, else -1.
    pub target_meal: i32,
    /// Whether the phase's own production raised the Turkey stock.
    pub map_refresh: bool,
}

/// One reset day.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Day {
    pub day: u32,
    pub window_hours: f64,
    pub phases: Vec<Phase>,
    /// The guaranteed ladles the day claimed.
    pub ladle_income: f64,
    /// What the claim could give with the doubling rolls.
    pub expected_income: f64,
    /// The ladles donated, including the up-front owned share on day one.
    pub jelly_donation: f64,
    /// The reset grant's hours per kitchen.
    pub jelly_hours: f64,
    /// The day's purchases grouped per meal, in first-spend order.
    pub spend: Vec<MealGain>,
    pub saving: Option<Saving>,
    pub savings: Vec<Saving>,
    pub passive_hours_used: f64,
    pub levels_bought: usize,
    pub ladles_used: f64,
    /// No Meal Left Behind's target this reset, or -1 when it had none.
    pub nmlb_meal: i32,
    pub nmlb_from: f64,
    pub nmlb_to: f64,
    /// The ladles No Meal Left Behind saved.
    pub nmlb_saved: f64,
    pub total_levels: f64,
}

/// The plan's Blue Jellyfish state.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JellyPlan {
    pub available: bool,
    pub unlocked: bool,
    /// The save's saved grant, before donations.
    pub daily_hours: f64,
    /// The ladles donated over the horizon.
    pub donated: f64,
    /// The grant's hours per kitchen after the last donation.
    pub daily_hours_end: f64,
    /// The hours the plan used across every reset.
    pub total_hours: f64,
    /// The up-front share of the owned ladles donated.
    pub owned_donation: f64,
}

/// Which source bought a level.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BySource {
    pub ladle: f64,
    pub jelly: f64,
    pub nmlb: f64,
}

/// A reset horizon's plan.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Plan {
    pub days: u32,
    pub hours_to_reset: f64,
    pub daily_income: f64,
    pub ladle_multi: f64,
    pub max_level: f64,
    /// The total speed of every unlocked kitchen.
    pub speed: f64,
    pub jelly: JellyPlan,
    pub start_levels: Vec<f64>,
    pub levels: Vec<f64>,
    pub stocks: Vec<f64>,
    pub gained: Vec<f64>,
    pub by_source: BySource,
    pub ladles_used: f64,
    /// The meal the next reset's No Meal Left Behind raises, when it has one.
    pub next_nmlb: Option<usize>,
    pub timeline: Vec<Day>,
}

/// `planMealLeveling`: the reset horizon the inputs describe.
pub fn plan(reset: &Reset, inputs: &Inputs) -> Result<Plan, Problem> {
    let days = inputs.days.clamp(1, MAX_DAYS);
    let ladle_multi = inputs.ladle_multi.max(0.0);
    let max_level = inputs.max_level.max(0.0);
    let jelly = reset.jelly;
    let owned_donation = match inputs.policy {
        Policy::None => 0.0,
        _ => (inputs.start_ladles.max(0.0) * inputs.owned_fraction.clamp(0.0, 1.0)).floor(),
    };
    donation(owned_donation, inputs.start_ladles.max(0.0), jelly.unlocked)?;
    let start_ladles = inputs.start_ladles.max(0.0) - owned_donation;
    let speed = reset.speed(inputs.base_speed);
    let passive_hours = if inputs.passive { 24.0 } else { 0.0 };
    let valid = reset
        .meals
        .levels
        .iter()
        .copied()
        .chain(reset.meals.stocks.iter().copied())
        .chain([ladle_multi, max_level, speed])
        .all(f64::is_finite);
    if !valid {
        return Err(Problem::Numbers);
    }
    let mut run = Run {
        reset,
        inputs,
        levels: reset.meals.levels.clone(),
        stocks: reset.meals.stocks.clone(),
        gained: vec![0.0; reset.meals.levels.len()],
        max_level,
        ladle_multi,
        speed,
        ola: inputs.cost.discount_days,
        jelly_hours: jelly.daily_hours + owned_donation,
        total_donated: owned_donation,
        total_jelly_hours: 0.0,
        ladles_used: 0.0,
        by_source: BySource::default(),
        cumulative_ladles: 0.0,
        source: Source::Ladle,
        cached_turkey: reset.meals.stocks.first().copied().unwrap_or(0.0),
        passive_hours,
        class_hours: 0.0,
        day_class_hours: 0.0,
        timeline: Vec::new(),
        start_ladles,
        owned_donation,
    };
    let full_day_income = run.claim_income(24.0).0;
    run.class_hours = passive_hours + full_day_income * ladle_multi;
    run.day_class_hours = run.class_hours;
    for day in 1..=days {
        run.day(day)?;
    }
    let next_nmlb = match reset.nmlb_active() {
        true => nmlb_target(&run.levels, max_level),
        false => None,
    };
    Ok(Plan {
        days,
        hours_to_reset: inputs.hours_to_reset.clamp(0.0, 24.0),
        daily_income: full_day_income,
        ladle_multi,
        max_level,
        speed,
        jelly: JellyPlan {
            available: jelly.available,
            unlocked: jelly.unlocked,
            daily_hours: jelly.daily_hours,
            donated: run.total_donated,
            daily_hours_end: run.jelly_hours,
            total_hours: run.total_jelly_hours,
            owned_donation,
        },
        start_levels: reset.meals.levels.clone(),
        levels: run.levels,
        stocks: run.stocks,
        gained: run.gained,
        by_source: run.by_source,
        ladles_used: run.ladles_used,
        next_nmlb,
        timeline: run.timeline,
    })
}

/// Which pool a purchase is charged to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Source {
    Ladle,
    Jelly,
}

/// The ladle hours a phase may still spend.
struct Pool {
    ladle: f64,
}

/// The simulation's own state while the horizon runs.
struct Run<'a> {
    reset: &'a Reset,
    inputs: &'a Inputs,
    levels: Vec<f64>,
    stocks: Vec<f64>,
    gained: Vec<f64>,
    max_level: f64,
    ladle_multi: f64,
    speed: f64,
    ola: f64,
    jelly_hours: f64,
    total_donated: f64,
    total_jelly_hours: f64,
    ladles_used: f64,
    by_source: BySource,
    cumulative_ladles: f64,
    source: Source,
    cached_turkey: f64,
    passive_hours: f64,
    class_hours: f64,
    day_class_hours: f64,
    timeline: Vec<Day>,
    start_ladles: f64,
    owned_donation: f64,
}

impl Run<'_> {
    /// The day's guaranteed ladles for a window of `hours`.
    fn claim_income(&self, hours: f64) -> (f64, f64) {
        let flat = self.inputs.ladles_per_day.max(0.0) * hours / 24.0;
        (flat, flat)
    }

    /// A meal the plan may level: discovered, below the cap and cookable.
    fn open(&self, meal: usize) -> bool {
        self.levels
            .get(meal)
            .is_some_and(|level| *level >= 1.0 && *level < self.max_level)
            && meal_requirement(meal) > 0.0
    }

    /// The kitchen hours raising `meal` to `target` needs, existing stock first.
    fn bundle_hours(&self, meal: usize, target: f64) -> f64 {
        let mut need = 0.0;
        let mut discount = self.ola;
        let mut level = self.levels[meal];
        while level < target {
            need += meal_level_cost(level, self.inputs.cost, discount);
            discount = 0.0;
            level += 1.0;
        }
        let deficit = (need - self.stocks[meal]).max(0.0);
        match (deficit, self.speed) {
            (deficit, _) if deficit <= 0.0 => 0.0,
            (_, speed) if speed <= 0.0 => f64::INFINITY,
            (deficit, speed) => deficit * meal_requirement(meal) / speed,
        }
    }

    /// Whether the plan levels this meal directly rather than banking for it.
    fn ladleable(&self, meal: usize) -> bool {
        !self.inputs.ladleable_only
            || self.bundle_hours(meal, self.levels[meal] + 1.0) <= self.day_class_hours
    }

    /// Meals one ladle cooks: `speed × ladle multi / requirement`.
    fn meals_per_ladle(&self, meal: usize) -> f64 {
        let requirement = meal_requirement(meal);
        match requirement > 0.0 {
            true => self.speed * self.ladle_multi / requirement,
            false => 0.0,
        }
    }

    /// `ladlesForLevels`: the ladles raising `meal` from `from` to `to`.
    fn ladles_for_levels(&self, meal: usize, from: f64, to: f64, stock: f64) -> f64 {
        let mut total = 0.0;
        let mut stock = stock;
        let mut level = from;
        while level < to {
            let cost = meal_level_cost(level, self.inputs.cost, 0.0);
            let per_ladle = self.meals_per_ladle(meal);
            let deficit = (cost - stock).max(0.0);
            total += match per_ladle > 0.0 {
                true => deficit / per_ladle,
                false => f64::INFINITY,
            };
            stock = (stock - cost).max(0.0);
            level += 1.0;
        }
        total
    }

    /// `refreshTurkey`: the Turkey stock passed the cached Artifact 13 bonus.
    fn refresh_turkey(&mut self) -> bool {
        let stock = self.stocks.first().copied().unwrap_or(0.0);
        if stock <= self.cached_turkey {
            return false;
        }
        self.cached_turkey = stock;
        true
    }

    /// `pickCheapest`: the meal whose next level costs the fewest kitchen hours.
    fn pick_cheapest(&self) -> (Option<usize>, f64) {
        let mut pick = None;
        let mut cheapest = f64::INFINITY;
        for meal in 0..self.levels.len() {
            if !self.open(meal) || !self.ladleable(meal) {
                continue;
            }
            let cost = meal_level_cost(self.levels[meal], self.inputs.cost, self.ola);
            let hours = match self.speed > 0.0 {
                true => (cost - self.stocks[meal]).max(0.0) * meal_requirement(meal) / self.speed,
                false => f64::INFINITY,
            };
            if hours < cheapest {
                pick = Some(meal);
                cheapest = hours;
            }
        }
        (pick, cheapest)
    }

    /// `ladlesFor`: draw `hours` from the ladle pool first, then passive time.
    fn ladles_for(&mut self, hours: f64, pool: &mut Pool) -> f64 {
        let from_ladles = hours.min(pool.ladle);
        pool.ladle -= from_ladles;
        match self.ladle_multi > 0.0 {
            true => from_ladles / self.ladle_multi,
            false => 0.0,
        }
    }

    /// `buy`: level `meal` once, charging whole ladles to the pool.
    fn buy(
        &mut self,
        meal: usize,
        hours: f64,
        pool: &mut Pool,
        rows: &mut Vec<Spend>,
        chained: bool,
    ) -> f64 {
        let from_pool = hours.min(pool.ladle);
        let mut used = hours;
        let mut step_ladles = 0.0;
        if from_pool > 0.0 && self.ladle_multi > 0.0 {
            let whole = (from_pool / self.ladle_multi - 1e-9).ceil();
            let whole_hours = pool.ladle.min(whole * self.ladle_multi);
            used = hours + whole_hours - from_pool;
            pool.ladle -= whole_hours;
            step_ladles = match whole_hours < whole * self.ladle_multi {
                true => whole_hours / self.ladle_multi,
                false => whole,
            };
        }
        let paid = meal_level_cost(self.levels[meal], self.inputs.cost, self.ola);
        let requirement = meal_requirement(meal);
        self.stocks[meal] = (self.stocks[meal] + used * self.speed / requirement - paid).max(0.0);
        self.levels[meal] += 1.0;
        self.gained[meal] += 1.0;
        match self.source {
            Source::Ladle => self.by_source.ladle += 1.0,
            Source::Jelly => self.by_source.jelly += 1.0,
        }
        self.ola = 0.0;
        self.cumulative_ladles += step_ladles;
        let position = rows.len();
        rows.push(Spend {
            meal,
            from: self.levels[meal] - 1.0,
            to: self.levels[meal],
            levels: 1,
            ladles: step_ladles,
            hours: used,
            cumulative_ladles: self.cumulative_ladles,
            map_refresh: false,
        });
        // The overflow from whole ladles can already cover the next level.
        while step_ladles > 0.0
            && self.levels[meal] < self.max_level
            && self.stocks[meal] >= meal_level_cost(self.levels[meal], self.inputs.cost, self.ola)
        {
            self.buy(meal, 0.0, pool, rows, true);
        }
        if !chained
            && meal == 0
            && self.refresh_turkey()
            && let Some(row) = rows.get_mut(position)
        {
            row.map_refresh = true;
        }
        used
    }

    /// `runPhase`: spend one phase's ladles and passive hours.
    fn run_phase(&mut self, phase: &mut Phase) {
        let mut hours = phase.passive_hours + phase.ladles * self.ladle_multi;
        let start_hours = hours;
        let mut pool = Pool {
            ladle: phase.ladles * self.ladle_multi,
        };
        let mut rows: Vec<Spend> = Vec::new();
        let mut saving: Option<Saving> = None;
        let mut guard = 0;
        loop {
            if hours <= 1e-12 || guard >= 40_000 {
                break;
            }
            guard += 1;
            let (pick, pick_hours) = self.pick_cheapest();
            let mut meal = pick;
            let mut best_hours = pick_hours;
            let mut to_target = false;
            if meal.is_none() {
                for index in 0..self.levels.len() {
                    if !self.open(index) {
                        continue;
                    }
                    let hours = self.bundle_hours(index, self.levels[index] + 1.0);
                    if meal.is_none() || hours < best_hours {
                        meal = Some(index);
                        best_hours = hours;
                    }
                }
                if meal.is_none() {
                    break;
                }
                to_target = true;
            }
            let Some(meal) = meal else { break };
            if best_hours > hours {
                let saved_ladles = self.ladles_for(hours, &mut pool);
                let requirement = meal_requirement(meal);
                self.stocks[meal] += hours * self.speed / requirement;
                let need = meal_level_cost(self.levels[meal], self.inputs.cost, self.ola);
                let progress = match need > 0.0 {
                    true => (self.stocks[meal] / need).min(1.0),
                    false => 1.0,
                };
                let map_refresh = meal == 0 && self.refresh_turkey();
                self.cumulative_ladles += saved_ladles;
                saving = Some(Saving {
                    meal,
                    to_level: self.levels[meal] + 1.0,
                    hours,
                    ladles: saved_ladles,
                    progress,
                    target: to_target,
                    map_refresh,
                });
                hours = 0.0;
                break;
            }
            hours -= self.buy(meal, best_hours, &mut pool, &mut rows, false);
        }
        let ladle_hours_used = phase.ladles * self.ladle_multi - pool.ladle;
        phase.rows = spend_order_rows(&rows);
        phase.saving = saving;
        phase.levels_bought = phase.rows.iter().map(|row| row.levels).sum();
        phase.ladles_used = match self.ladle_multi > 0.0 {
            true => phase.ladles.min(ladle_hours_used / self.ladle_multi),
            false => 0.0,
        };
        phase.passive_hours_used = ((start_hours - hours) - ladle_hours_used).max(0.0);
    }

    /// `runJellyBatch`: the reset grant cooks one batch, then its stock buys
    /// levels in the layout's purchase order.
    fn run_jelly(&mut self) -> Result<Phase, Problem> {
        let mut phase = Phase {
            kind: PhaseKind::Jelly,
            jelly_hours: self.jelly_hours,
            target_meal: -1,
            ..Phase::default()
        };
        let batch = layout::optimize(&layout::Input {
            levels: &self.levels,
            stocks: &self.stocks,
            kitchens: &self.reset.kitchens,
            hours: self.jelly_hours,
            base_speed: self.inputs.base_speed,
            max_level: self.max_level,
            cost: self.inputs.cost,
            discount_days: self.ola,
            choice: &self.inputs.layout,
        })?;
        for meal in &batch.meals {
            self.stocks[meal.meal] += meal.produced;
        }
        self.source = Source::Jelly;
        for meal in &batch.purchase_order {
            let mut guard = 0;
            while self.levels[*meal] < self.max_level
                && self.stocks[*meal]
                    >= meal_level_cost(self.levels[*meal], self.inputs.cost, self.ola)
                && guard < 100_000
            {
                guard += 1;
                let mut pool = Pool { ladle: 0.0 };
                self.buy(*meal, 0.0, &mut pool, &mut phase.rows, false);
            }
        }
        self.source = Source::Ladle;
        phase.map_refresh = self.refresh_turkey();
        phase.rows = spend_order_rows(&phase.rows);
        phase.levels_bought = phase.rows.iter().map(|row| row.levels).sum();
        phase.savings = batch
            .meals
            .iter()
            .filter(|meal| meal.to < self.max_level)
            .cloned()
            .collect();
        phase.layout = Some(batch);
        Ok(phase)
    }

    /// One day of the horizon: claim, donate, spend, No Meal Left Behind, batch.
    fn day(&mut self, day: u32) -> Result<(), Problem> {
        let first = day == 1;
        let window = match first {
            true => self.inputs.hours_to_reset.clamp(0.0, 24.0),
            false => 24.0,
        };
        let (income, expected) = self.claim_income(window);
        let target = match self.inputs.policy {
            Policy::None => 0.0,
            Policy::All => income,
            Policy::Share => income * self.inputs.income_fraction.clamp(0.0, 1.0),
            Policy::Fixed => income.min(self.inputs.donation_per_day.max(0.0)),
        };
        let daily_donation = match self.inputs.policy {
            Policy::None => 0.0,
            _ => target.floor(),
        };
        donation(daily_donation, income, self.reset.jelly.unlocked)?;
        self.jelly_hours += daily_donation;
        self.total_donated += daily_donation;
        let mut phases: Vec<Phase> = Vec::new();
        if first && self.start_ladles > 0.0 {
            phases.push(Phase {
                kind: PhaseKind::Now,
                ladles: self.start_ladles,
                expected_ladles: self.start_ladles,
                ..Phase::default()
            });
        }
        phases.push(Phase {
            kind: PhaseKind::Claim,
            window_hours: window,
            ladles: income - daily_donation,
            expected_ladles: expected - daily_donation,
            passive_hours: self.passive_hours * window / 24.0,
            ..Phase::default()
        });
        let day_hours: f64 = phases
            .iter()
            .map(|phase| phase.passive_hours + phase.ladles * self.ladle_multi)
            .sum();
        self.day_class_hours = match self.class_hours > 0.0 {
            true => self.class_hours,
            false => day_hours,
        };
        let mut ladles_used = 0.0;
        let mut passive_used = 0.0;
        let mut saving: Option<Saving> = None;
        for phase in phases.iter_mut() {
            self.run_phase(phase);
            ladles_used += phase.ladles_used;
            passive_used += phase.passive_hours_used;
            if let Some(banked) = phase.saving.clone() {
                saving = Some(banked);
            }
        }
        self.ladles_used += ladles_used;
        let mut nmlb_meal = -1;
        let mut nmlb_from = 0.0;
        let mut nmlb_saved = 0.0;
        if self.reset.nmlb_active()
            && let Some(meal) = nmlb_target(&self.levels, self.max_level)
        {
            nmlb_from = self.levels[meal];
            let to = nmlb_result_level(nmlb_from, self.reset.nmlb, self.max_level);
            nmlb_saved = self.ladles_for_levels(meal, nmlb_from, to, 0.0);
            self.by_source.nmlb += to - nmlb_from;
            self.gained[meal] += to - nmlb_from;
            self.levels[meal] = to;
            if to > nmlb_from {
                nmlb_meal = meal as i32;
            } else {
                nmlb_from = 0.0;
            }
        }
        if self.inputs.cost.dream_11 > 0.0 {
            self.ola += 1.0;
        }
        let nmlb_to = match nmlb_meal >= 0 {
            true => self.levels[nmlb_meal as usize],
            false => 0.0,
        };
        let jelly_phase = self.run_jelly()?;
        self.total_jelly_hours += self.jelly_hours;
        phases.push(jelly_phase);
        let spend: Vec<Spend> = phases
            .iter()
            .flat_map(|phase| phase.rows.iter().cloned())
            .collect();
        let savings: Vec<Saving> = phases
            .iter()
            .filter_map(|phase| phase.saving.clone())
            .collect();
        let levels_bought = spend.iter().map(|row| row.levels).sum();
        let total_levels: f64 = self.levels.iter().sum();
        self.timeline.push(Day {
            day,
            window_hours: window,
            phases,
            ladle_income: income,
            expected_income: expected,
            jelly_donation: daily_donation + if first { self.owned_donation } else { 0.0 },
            jelly_hours: self.jelly_hours,
            spend: group_day_steps(&spend),
            saving,
            savings,
            passive_hours_used: passive_used,
            levels_bought,
            ladles_used,
            nmlb_meal,
            nmlb_from,
            nmlb_to,
            nmlb_saved,
            total_levels,
        });
        Ok(())
    }
}

/// `_spendOrderRows`: one row per purchase order, stock-covered levels folded in.
fn spend_order_rows(steps: &[Spend]) -> Vec<Spend> {
    let mut rows: Vec<Spend> = Vec::new();
    for step in steps {
        match rows.last_mut() {
            Some(last) if last.meal == step.meal && step.hours <= 0.0 => {
                last.to = step.to;
                last.levels += 1;
                last.map_refresh |= step.map_refresh;
            }
            _ => rows.push(step.clone()),
        }
    }
    rows
}

/// `_groupDaySteps`: the day's purchases per meal, in first-spend order.
fn group_day_steps(steps: &[Spend]) -> Vec<MealGain> {
    let mut groups: Vec<MealGain> = Vec::new();
    for step in steps {
        match groups.iter_mut().find(|group| group.meal == step.meal) {
            Some(group) => {
                group.to = step.to;
                group.levels += step.levels;
                group.ladles += step.ladles;
                group.hours += step.hours;
            }
            None => groups.push(MealGain {
                meal: step.meal,
                from: step.from,
                to: step.to,
                levels: step.levels,
                ladles: step.ladles,
                hours: step.hours,
            }),
        }
    }
    groups
}

impl Phase {
    /// The phase's heading, as the upstream reset instructions open each block.
    pub fn title(&self, day: u32) -> String {
        match self.kind {
            PhaseKind::Jelly => format!(
                "After reset — {} jellyfish hours per kitchen",
                hours(self.jelly_hours)
            ),
            PhaseKind::Now => format!("Now — spend {} owned ladles", count(self.ladles)),
            PhaseKind::Claim => {
                let when = match day {
                    1 => format!("Claim before reset (in {})", hours(self.window_hours)),
                    _ => "Claims through the day".to_owned(),
                };
                format!("{when} — {} ladles allocated to meals", count(self.ladles))
            }
        }
    }

    /// The phase's lines: its heading, one line per purchase, its banked time.
    fn instructions(&self, day: u32) -> Vec<String> {
        let mut lines = vec![self.title(day)];
        let offset = match self.rows.first() {
            Some(row) => row.cumulative_ladles - row.ladles,
            None => 0.0,
        };
        for (index, row) in self.rows.iter().enumerate() {
            let levels = match row.levels {
                1 => format!("Lv {} → {}", number(row.from), number(row.to)),
                levels => format!("Lv {} → {} ({levels})", number(row.from), number(row.to)),
            };
            let charge = match self.kind {
                PhaseKind::Jelly => "jelly grant".to_owned(),
                _ => match (row.ladles <= 0.0, row.hours <= 0.0) {
                    (true, false) => "passive time".to_owned(),
                    (true, true) => "stock".to_owned(),
                    _ => format!("{} ladles / {}", count(row.ladles), hours(row.hours)),
                },
            };
            lines.push(format!(
                "  {}. {} · {levels} · {charge} · {} used so far{}",
                index + 1,
                meal_name(row.meal),
                count(row.cumulative_ladles - offset),
                match row.map_refresh {
                    true => " · change map before the next spend",
                    false => "",
                }
            ));
        }
        if let Some(saving) = &self.saving {
            lines.push(format!("  {}", saving.line(self.kind)));
        }
        if self.rows.is_empty() && self.saving.is_none() {
            lines.push(
                match self.kind == PhaseKind::Jelly && self.target_meal >= 0 {
                    true => format!(
                        "  The grant banks into {} stock; no level is affordable yet.",
                        meal_name(self.target_meal as usize)
                    ),
                    false => "  No purchases.".to_owned(),
                },
            );
        }
        lines
    }
}

impl Saving {
    /// The banked-time line, naming what the leftover stock is working toward.
    fn line(&self, kind: PhaseKind) -> String {
        let what = match kind {
            PhaseKind::Jelly => "Leftover jellyfish time".to_owned(),
            _ => match (self.ladles > 0.0, self.hours > 0.0) {
                (true, _) => format!("Leftover {} ladles", count(self.ladles)),
                (false, true) => "Leftover passive time".to_owned(),
                (false, false) => "Leftover time".to_owned(),
            },
        };
        let mut line = format!(
            "{what} banks into {} toward Lv {} ({}% stocked)",
            meal_name(self.meal),
            number(self.to_level),
            (self.progress * 100.0).round()
        );
        if self.target {
            line.push_str(" — no ladleable level is left, so this is NMLB-target stock");
        }
        if self.map_refresh {
            line.push_str(
                " — this raises your Turkey a la Thank stock, so change map before spending more ladles",
            );
        }
        line.push('.');
        line
    }
}

impl Day {
    /// The one-line reset summary the forecast list shows.
    pub fn headline(&self) -> String {
        format!(
            "Reset {} · {} earned · {} donated · {} spent directly · {} grant · +{} ranks",
            self.day,
            count(self.ladle_income),
            count(self.jelly_donation),
            count(self.ladles_used),
            hours(self.jelly_hours),
            number(self.levels_bought as f64 + self.nmlb_to - self.nmlb_from)
        )
    }

    /// The reset's instructions, as the plain text the copy button takes.
    pub fn instructions(&self) -> String {
        let mut lines = vec![
            self.headline(),
            String::new(),
            format!(
                "Before reset: follow the pre-reset spending below, donate {} ladles, then arrange \
                 kitchens as shown. The donation button takes all accessible ladles; separate any \
                 surplus first.",
                count(self.jelly_donation)
            ),
            String::new(),
        ];
        for phase in &self.phases {
            if phase.kind == PhaseKind::Jelly {
                continue;
            }
            lines.extend(phase.instructions(self.day));
            lines.push(String::new());
        }
        let nmlb = match self.nmlb_meal >= 0 {
            true => format!(
                "NMLB {} Lv {} → {}",
                meal_name(self.nmlb_meal as usize),
                number(self.nmlb_from),
                number(self.nmlb_to)
            ),
            false => "NMLB has no hit".to_owned(),
        };
        lines.push(format!(
            "At reset: {nmlb}, then {} hours in each kitchen.",
            hours(self.jelly_hours)
        ));
        for phase in &self.phases {
            let Some(layout) = &phase.layout else {
                continue;
            };
            for kitchen in &layout.kitchens {
                lines.push(format!(
                    "  Kitchen {} · {} · {} cooking speed/hr{}",
                    kitchen.index + 1,
                    meal_label(kitchen.meal),
                    number(kitchen.speed),
                    match kitchen.pinned {
                        true => " · pinned",
                        false => "",
                    }
                ));
                match kitchen.meal >= 0 {
                    true => lines.push(format!(
                        "    saves {} · cooks {}, produces {} stock",
                        meal_label(kitchen.saved_meal),
                        meal_name(kitchen.meal as usize),
                        number(kitchen.produced)
                    )),
                    false => lines.push(format!(
                        "    saves {} · stays idle",
                        meal_label(kitchen.saved_meal)
                    )),
                }
            }
            for meal in &layout.meals {
                let combined = layout
                    .meals
                    .iter()
                    .filter(|other| other.meal == meal.meal)
                    .count();
                lines.push(format!(
                    "  {} Lv {} → {} · {} stock{}{}",
                    meal_name(meal.meal),
                    number(meal.from),
                    number(meal.to),
                    number(meal.stock),
                    match meal.progress > 0.0 && meal.to < f64::INFINITY {
                        true => format!(
                            " ({}% toward Lv {})",
                            (meal.progress * 100.0).round(),
                            number(meal.to + 1.0)
                        ),
                        false => String::new(),
                    },
                    match combined > 1 {
                        true => " · kitchens share this result",
                        false => "",
                    }
                ));
            }
        }
        lines.push(String::new());
        for phase in &self.phases {
            if phase.kind != PhaseKind::Jelly {
                continue;
            }
            lines.push(
                "After reset: buy the following levels from the batch stock. Kitchen cards share a \
                 combined result when several kitchens cook the same meal."
                    .to_owned(),
            );
            lines.extend(phase.instructions(self.day));
        }
        let carried: Vec<String> = self
            .phases
            .iter()
            .filter(|phase| phase.kind == PhaseKind::Jelly)
            .flat_map(|phase| phase.savings.iter())
            .map(|meal| {
                format!(
                    "{}: {} stock ({}% toward Lv {})",
                    meal_name(meal.meal),
                    number(meal.stock),
                    (meal.progress * 100.0).round(),
                    number(meal.to + 1.0)
                )
            })
            .collect();
        if !carried.is_empty() {
            lines.push(format!("Stock carried forward: {}.", carried.join(" · ")));
        }
        let refresh = self
            .phases
            .iter()
            .any(|phase| phase.map_refresh || phase.rows.iter().any(|row| row.map_refresh));
        if refresh {
            lines.push(
                "Change map before the next ladle spend to refresh the increased Turkey stock \
                 bonus."
                    .to_owned(),
            );
        }
        lines.join("\n")
    }
}

/// A meal name or `Idle` for a kitchen with no meal.
fn meal_label(meal: i32) -> String {
    match meal >= 0 {
        true => meal_name(meal as usize).to_owned(),
        false => "Idle".to_owned(),
    }
}

/// `fullCount`: a whole number of ladles or levels.
fn count(value: f64) -> String {
    match (value.is_finite(), value.abs() < 1e21) {
        (false, _) => "∞".to_owned(),
        (true, true) => format!("{:.0}", value.round()),
        (true, false) => number(value),
    }
}

/// `num`: a number without trailing zeros, in the game's own scale.
fn number(value: f64) -> String {
    if !value.is_finite() {
        return "∞".to_owned();
    }
    if value.abs() >= 1e6 {
        return format!("{value:.3e}");
    }
    let rounded = (value * 100.0).round() / 100.0;
    match rounded.fract() == 0.0 {
        true => format!("{rounded:.0}"),
        false => format!("{rounded}"),
    }
}

/// `fmtHours`: a duration in the unit a player reads.
fn hours(value: f64) -> String {
    if !value.is_finite() {
        return "∞".to_owned();
    }
    match value {
        value if value < 1.0 => format!("{:.0}m", (value * 60.0).round()),
        value if value < 48.0 => format!("{value:.1}h"),
        value if value < 24.0 * 3650.0 => format!("{}d", number(value / 24.0)),
        value => format!("{}y", number(value / 8760.0)),
    }
}
