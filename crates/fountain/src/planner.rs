//! FN-03: the upgrade planner and the greedy upgrade optimizer.
//!
//! Ported from `_renderPlanner` and `_renderOptimizer` in `fountain-calc.html`
//! and `measureGoal`/`optimalStackValue` in
//! `js/stats/systems/w5/fountain.js`. Both simulate one or more upgrades on a
//! cloned cavern, so every measured value comes from the same accessors as the
//! page instead of a second implementation of the fountain's maths.
//!
//! The planner ranks the next level of every accessible upgrade by the gain it
//! makes toward one goal. The optimizer repeats that ranking, buys the best
//! affordable step, and ranks again at the levels the purchase reached; its
//! budget is the saved opal balance and every step is paid from the currency
//! the upgrade costs.

use super::data::UPGRADES_PER_WATER;
use super::{CURRENCIES, Fountain, WATERS};
use idleon_common::{Control, Progress};

/// The goals `measureGoal` implements and the page's planner lists.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    /// The whole currency multiplier, the royal multiplier and the desire bonus.
    #[default]
    Currency,
    /// Fills per second while standing at the fountain.
    Fill,
    /// One of the six coin types, valued as both the coin and the desired type.
    Bronze,
    Silver,
    Golden,
    Dollar,
    Credit,
    Treasury,
    /// The chance a fill crowns a royal stack.
    Royal,
    /// The chance a duck bar fill gives a duck.
    Duck,
    /// The base chance a collected coin is lucky.
    Lucky,
    /// The marble an hour of active play gives.
    MarbleRate,
    /// Optimal stacks per hour times one stack's value.
    Optimal,
}

impl Goal {
    /// Every goal the page's selector lists, in page order.
    pub const ALL: [Goal; 13] = [
        Goal::Currency,
        Goal::Fill,
        Goal::Bronze,
        Goal::Silver,
        Goal::Golden,
        Goal::Dollar,
        Goal::Credit,
        Goal::Treasury,
        Goal::Royal,
        Goal::Duck,
        Goal::Lucky,
        Goal::MarbleRate,
        Goal::Optimal,
    ];

    /// The `measureGoal` case name.
    pub fn id(self) -> &'static str {
        match self {
            Goal::Currency => "currency",
            Goal::Fill => "fill",
            Goal::Bronze => "bronze",
            Goal::Silver => "silver",
            Goal::Golden => "golden",
            Goal::Dollar => "dollar",
            Goal::Credit => "credit",
            Goal::Treasury => "treasury",
            Goal::Royal => "royal",
            Goal::Duck => "duck",
            Goal::Lucky => "lucky",
            Goal::MarbleRate => "marble-rate",
            Goal::Optimal => "optimal",
        }
    }

    /// The option text the page prints.
    pub fn label(self) -> &'static str {
        match self {
            Goal::Currency => "Total Currency Value",
            Goal::Fill => "Fill Speed",
            Goal::Bronze => "Bronze Coins",
            Goal::Silver => "Silver Coins",
            Goal::Golden => "Golden Coins",
            Goal::Dollar => "Dollar Bills",
            Goal::Credit => "Credit Bills",
            Goal::Treasury => "Treasury Bills",
            Goal::Royal => "Royal Chance",
            Goal::Duck => "Duck Chance",
            Goal::Lucky => "Lucky Coin Chance",
            Goal::MarbleRate => "Marble /hr",
            Goal::Optimal => "Est. Optimal /hr (Desired)",
        }
    }

    /// Resolve the `value` of the page's goal selector.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|goal| goal.id() == id)
    }

    /// The coin type a `bronze`…`treasury` goal measures, if any.
    pub fn currency(self) -> Option<usize> {
        match self {
            Goal::Bronze => Some(0),
            Goal::Silver => Some(1),
            Goal::Golden => Some(2),
            Goal::Dollar => Some(3),
            Goal::Credit => Some(4),
            Goal::Treasury => Some(5),
            _ => None,
        }
    }
}

/// The planner and optimizer controls the page shares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlanRequest {
    pub goal: Goal,
    /// The simulated desired currency; `None` reads the save's choice.
    pub desired: Option<usize>,
    /// The optimizer's step cap.
    pub max_steps: usize,
    /// Skip an upgrade the saved balance cannot pay for.
    pub affordable: bool,
    /// Divide the cost by the currency's own value before ranking.
    pub normalize: bool,
    /// Rank by the hours needed to earn the cost instead of its raw size.
    pub per_hr: bool,
}

impl Default for PlanRequest {
    fn default() -> Self {
        Self {
            goal: Goal::Currency,
            desired: None,
            max_steps: 50,
            affordable: false,
            normalize: false,
            per_hr: false,
        }
    }
}

/// One next-level candidate on the planner's table.
#[derive(Clone, Debug, PartialEq)]
pub struct PlannerRow {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    pub level: f64,
    pub cost: f64,
    pub currency: usize,
    pub affordable: bool,
    /// The goal's value once this upgrade gains one level.
    pub value: f64,
    pub delta: f64,
    pub pct_gain: f64,
    /// The ranking key: percentage gain per unit (or per hour of) cost.
    pub efficiency: f64,
    pub seconds_to_afford: f64,
    /// The coin fill time this level removes; only `fill` goals measure it.
    pub fill_delta: f64,
    pub marble_fill_delta: f64,
    /// `None` while the duck bar is locked.
    pub duck_fill_delta: Option<f64>,
    pub royal_chance: f64,
    pub royal_delta: f64,
    pub duck_chance: f64,
    pub duck_delta: f64,
    pub lucky_chance: f64,
    pub lucky_delta: f64,
    /// For `optimal`: the fill-speed and stack-value halves of the gain.
    pub speed_pct: f64,
    pub value_pct: f64,
}

/// One purchased level in the optimizer's plan.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanStep {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    /// The level after the purchase.
    pub level: f64,
    pub cost: f64,
    pub currency: usize,
    pub delta: f64,
    pub pct_gain: f64,
    /// The goal's value after this step.
    pub value: f64,
    /// Active-play seconds needed to reach this step, including any waits.
    pub cumulative_seconds: f64,
}

/// Consecutive purchases of one upgrade, as the optimizer's table lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanGroup {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    pub count: usize,
    pub start_level: f64,
    pub end_level: f64,
    pub total_cost: f64,
    pub currency: usize,
    /// The elapsed time when the group's last level is reached.
    pub cumulative_seconds: f64,
}

/// The optimizer's whole answer.
#[derive(Clone, Debug, PartialEq)]
pub struct OptimizerPlan {
    pub goal: Goal,
    pub start_value: f64,
    pub end_value: f64,
    pub gain_pct: f64,
    pub steps: Vec<PlanStep>,
    pub groups: Vec<PlanGroup>,
    /// The opal balances the plan leaves.
    pub remaining: [f64; CURRENCIES],
}

impl Fountain {
    /// `measureGoal`: the value one goal reads at the current levels.
    pub fn measure_goal(&self, goal: Goal, desired: usize) -> f64 {
        if let Some(currency) = goal.currency() {
            return self.currency_total_value_with(currency, currency)
                * self.effective_royal_multi();
        }
        match goal {
            Goal::Currency => {
                self.currency_all_multi() * self.effective_royal_multi() * self.desire_multi()
            }
            Goal::Fill => self.active_speed() / self.bar_fill_time(),
            Goal::Royal => self.royal_chance(),
            Goal::Duck => self.duck_chance(),
            Goal::Lucky => self.lucky_coin_base_chance(),
            Goal::MarbleRate => self.marble_per_hr(),
            Goal::Optimal => self.optimal_stacks_per_hr() * self.optimal_stack_value(desired),
            Goal::Bronze
            | Goal::Silver
            | Goal::Golden
            | Goal::Dollar
            | Goal::Credit
            | Goal::Treasury => unreachable!("coin goals measured above"),
        }
    }

    /// `_renderPlanner`'s candidates, ranked by efficiency.
    ///
    /// One level of every accessible upgrade is measured on a copy of the
    /// cavern. A goal the upgrade does not move is skipped, and so is an
    /// unaffordable upgrade when the caller asked for affordable steps only.
    pub fn planner(&self, request: PlanRequest) -> Vec<PlannerRow> {
        let desired = request.desired.unwrap_or_else(|| self.desired_currency());
        let goal = request.goal;
        let baseline = self.measure_goal(goal, desired);
        let base_fill = self.active_fill_time();
        let base_marble_fill = Self::marble_bar_fill_time() / self.active_speed();
        let base_duck_fill = self.duck_bar_fill();
        let base_royal = self.royal_chance();
        let base_duck = if self.duck_bar_unlocked() {
            self.duck_chance()
        } else {
            0.0
        };
        let base_lucky = self.lucky_coin_base_chance();
        let base_stacks = self.optimal_stacks_per_hr();
        let base_stack_value = self.optimal_stack_value(desired);
        let rates = self.earn_rates();
        let mut rows = Vec::new();
        for water in 0..WATERS {
            for upgrade in 0..UPGRADES_PER_WATER {
                if !self.upgrade_accessible(water, upgrade) {
                    continue;
                }
                let Some(entry) = super::upgrade(water, upgrade) else {
                    continue;
                };
                let level = self.upgrade_level(water, upgrade);
                let cost = Self::upgrade_cost(water, upgrade, level);
                let currency = entry.currency;
                let affordable = self.level_currency_balance(currency) >= cost;
                if request.affordable && !affordable {
                    continue;
                }
                let simulated = self.with_upgrade_level(water, upgrade, level + 1.0);
                let value = simulated.measure_goal(goal, desired);
                let delta = value - baseline;
                if delta <= 0.0 {
                    continue;
                }
                let pct_gain = pct_gain(baseline, delta);
                let efficiency = efficiency(request, cost, pct_gain, currency, &rates, self);
                let rate = rates.get(currency).copied().unwrap_or(0.0);
                let remaining = (cost - self.level_currency_balance(currency)).max(0.0);
                let seconds_to_afford = if remaining <= 0.0 {
                    0.0
                } else if rate > 0.0 {
                    remaining / rate * 3600.0
                } else {
                    f64::INFINITY
                };
                let (fill_delta, marble_fill_delta, duck_fill_delta) = if goal == Goal::Fill {
                    (
                        base_fill - simulated.active_fill_time(),
                        base_marble_fill - Self::marble_bar_fill_time() / simulated.active_speed(),
                        base_duck_fill.map(|base| {
                            base - Self::duck_bar_fill_time() / simulated.active_speed()
                        }),
                    )
                } else {
                    (0.0, 0.0, None)
                };
                let (speed_pct, value_pct) = if goal == Goal::Optimal {
                    (
                        ratio_pct(base_stacks, simulated.optimal_stacks_per_hr()),
                        ratio_pct(base_stack_value, simulated.optimal_stack_value(desired)),
                    )
                } else {
                    (0.0, 0.0)
                };
                rows.push(PlannerRow {
                    water,
                    upgrade,
                    name: entry.name.as_str(),
                    level,
                    cost,
                    currency,
                    affordable,
                    value,
                    delta,
                    pct_gain,
                    efficiency,
                    seconds_to_afford,
                    fill_delta,
                    marble_fill_delta,
                    duck_fill_delta,
                    royal_chance: simulated.royal_chance(),
                    royal_delta: simulated.royal_chance() - base_royal,
                    duck_chance: simulated.duck_chance(),
                    duck_delta: simulated.duck_chance() - base_duck,
                    lucky_chance: simulated.lucky_coin_base_chance(),
                    lucky_delta: simulated.lucky_coin_base_chance() - base_lucky,
                    speed_pct,
                    value_pct,
                });
            }
        }
        // JS `Array.prototype.sort` is stable, so equal efficiencies keep the
        // water-major scan order; `sort_by` is stable too.
        rows.sort_by(|left, right| right.efficiency.total_cmp(&left.efficiency));
        rows
    }

    /// `_renderOptimizer` without a progress report.
    pub fn optimize(&self, request: PlanRequest) -> OptimizerPlan {
        self.optimize_with(request, |_| Control::Continue)
            .expect("a plan without reports is never asked to stop")
    }

    /// `_renderOptimizer`: greedily buy the best step for the goal.
    ///
    /// The saved opal balances are the budget. With `affordable` only steps the
    /// current balance covers are considered. Without it a step may wait for
    /// the currency to earn the rest, and the wait is added to the cumulative
    /// ETA; a step whose currency earns nothing is skipped instead.
    pub fn optimize_with(
        &self,
        request: PlanRequest,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<OptimizerPlan> {
        let desired = request.desired.unwrap_or_else(|| self.desired_currency());
        let goal = request.goal;
        let mut simulated = self.clone();
        let mut budgets = [0.0; CURRENCIES];
        for (currency, budget) in budgets.iter_mut().enumerate() {
            *budget = self.level_currency_balance(currency);
        }
        let start_value = self.measure_goal(goal, desired);
        let mut baseline = start_value;
        let mut steps: Vec<PlanStep> = Vec::new();
        let mut clock = 0.0;
        let active = self.active_currencies();
        let mut optimizer_desired = desired;
        let mut optimizer_focus = if self.ignore_unlocked() && active.len() == 1 {
            Some(active[0])
        } else {
            None
        };
        let max_steps = request.max_steps;
        for step in 0..max_steps {
            let rates = simulated.earn_rates();
            let best = best_step(&simulated, request, goal, desired, &budgets, &rates);
            let Some(best) = best else {
                break;
            };
            if !request.affordable && budgets[best.currency] < best.cost {
                let can_change_desired = simulated.bon_tot(1, 11) > 0.0;
                let target_desired = if can_change_desired {
                    best.currency
                } else {
                    optimizer_desired
                };
                let target_focus = if simulated.ignore_unlocked() {
                    Some(best.currency)
                } else {
                    optimizer_focus
                };
                if target_focus != optimizer_focus {
                    let flush = simulated.flush_time();
                    let flush_rates =
                        simulated.active_earn_rates_with(optimizer_desired, optimizer_focus);
                    for (budget, rate) in budgets.iter_mut().zip(flush_rates) {
                        *budget += rate * flush / 3600.0;
                    }
                    clock += flush;
                }
                optimizer_desired = target_desired;
                optimizer_focus = target_focus;
                let remaining = (best.cost - budgets[best.currency]).max(0.0);
                let target_rates =
                    simulated.active_earn_rates_with(optimizer_desired, optimizer_focus);
                let wait = if target_rates[best.currency] > 0.0 {
                    remaining / target_rates[best.currency] * 3600.0
                } else {
                    f64::INFINITY
                };
                if !wait.is_finite() {
                    break;
                }
                for (budget, rate) in budgets.iter_mut().zip(target_rates) {
                    *budget += rate * wait / 3600.0;
                }
                clock += wait;
            }
            budgets[best.currency] = (budgets[best.currency] - best.cost).max(0.0);
            let level = simulated.upgrade_level(best.water, best.upgrade) + 1.0;
            simulated = simulated.with_upgrade_level(best.water, best.upgrade, level);
            baseline += best.delta;
            steps.push(PlanStep {
                water: best.water,
                upgrade: best.upgrade,
                name: best.name,
                level,
                cost: best.cost,
                currency: best.currency,
                delta: best.delta,
                pct_gain: best.pct_gain,
                value: baseline,
                cumulative_seconds: clock,
            });
            let progress = Progress {
                fraction: (step + 1) as f32 / max_steps.max(1) as f32,
                message: format!(
                    "Step {} of {max_steps}: {} → level {}",
                    step + 1,
                    best.name,
                    super::short_number(level)
                ),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }
        if report(Progress {
            fraction: 1.0,
            message: format!("Planned {} step(s).", steps.len()),
        }) == Control::Stop
        {
            return None;
        }
        Some(OptimizerPlan {
            goal,
            start_value,
            end_value: baseline,
            gain_pct: pct_gain(start_value, baseline - start_value),
            groups: groups(&steps),
            steps,
            remaining: budgets,
        })
    }

    /// One upgrade level above the saved one, on a copy of the cavern.
    fn with_upgrade_level(&self, water: usize, upgrade: usize, level: f64) -> Fountain {
        let mut simulated = self.clone();
        if let Some(cells) = simulated.upg_lvs.get_mut(water)
            && let Some(cell) = cells.get_mut(upgrade)
        {
            *cell = level;
        }
        simulated
    }

    /// `duckBarFillTime` at the active rate, or `None` while the bar is locked.
    fn duck_bar_fill(&self) -> Option<f64> {
        self.duck_bar_unlocked()
            .then(|| Self::duck_bar_fill_time() / self.active_speed())
    }
}

/// The best affordable next step at the simulated levels.
fn best_step(
    simulated: &Fountain,
    request: PlanRequest,
    goal: Goal,
    desired: usize,
    budgets: &[f64; CURRENCIES],
    rates: &[f64; CURRENCIES],
) -> Option<Candidate> {
    let baseline = simulated.measure_goal(goal, desired);
    let mut best: Option<Candidate> = None;
    for water in 0..WATERS {
        for upgrade in 0..UPGRADES_PER_WATER {
            if !simulated.upgrade_accessible(water, upgrade) {
                continue;
            }
            let Some(entry) = super::upgrade(water, upgrade) else {
                continue;
            };
            let level = simulated.upgrade_level(water, upgrade);
            let cost = Fountain::upgrade_cost(water, upgrade, level);
            let currency = entry.currency;
            let budget = budgets.get(currency).copied().unwrap_or(0.0);
            let affordable = budget >= cost;
            if request.affordable && !affordable {
                continue;
            }
            if !request.affordable
                && !affordable
                && rates.get(currency).copied().unwrap_or(0.0) <= 0.0
            {
                continue;
            }
            let candidate = simulated.with_upgrade_level(water, upgrade, level + 1.0);
            let value = candidate.measure_goal(goal, desired);
            let delta = value - baseline;
            if delta <= 0.0 {
                continue;
            }
            let pct_gain = pct_gain(baseline, delta);
            let efficiency = efficiency(request, cost, pct_gain, currency, rates, simulated);
            let row = Candidate {
                water,
                upgrade,
                name: entry.name.as_str(),
                cost,
                currency,
                delta,
                pct_gain,
                efficiency,
            };
            let better = best
                .as_ref()
                .is_none_or(|best| row.efficiency > best.efficiency);
            if better {
                best = Some(row);
            }
        }
    }
    best
}

/// One simulated next level, before it is written into the plan.
struct Candidate {
    water: usize,
    upgrade: usize,
    name: &'static str,
    cost: f64,
    currency: usize,
    delta: f64,
    pct_gain: f64,
    efficiency: f64,
}

/// `pctGainFrom`: the gain as a percentage of the baseline.
fn pct_gain(baseline: f64, delta: f64) -> f64 {
    if delta <= 0.0 {
        return 0.0;
    }
    if baseline > 0.0 {
        delta / baseline * 100.0
    } else {
        100.0
    }
}

/// The gain relative to a starting figure, in percent.
fn ratio_pct(start: f64, end: f64) -> f64 {
    if start > 0.0 {
        (end / start - 1.0) * 100.0
    } else {
        0.0
    }
}

/// The planner's efficiency key: gain per cost, or per hour needed to earn it.
fn efficiency(
    request: PlanRequest,
    cost: f64,
    pct_gain: f64,
    currency: usize,
    rates: &[f64; CURRENCIES],
    fountain: &Fountain,
) -> f64 {
    if request.per_hr {
        let rate = rates.get(currency).copied().unwrap_or(0.0);
        if rate <= 0.0 {
            return 0.0;
        }
        let hours = cost / rate;
        return if hours > 0.0 && hours.is_finite() {
            pct_gain / hours
        } else {
            0.0
        };
    }
    let cost = if request.normalize {
        let value = fountain.currency_total_value_with(currency, currency);
        if value > 0.0 { cost / value } else { cost }
    } else {
        cost
    };
    if cost > 0.0 { pct_gain / cost } else { 0.0 }
}

/// The plan's per-upgrade groups, ordered by when their last level lands.
fn groups(steps: &[PlanStep]) -> Vec<PlanGroup> {
    let mut grouped: Vec<PlanGroup> = Vec::new();
    for step in steps {
        if let Some(group) = grouped
            .iter_mut()
            .find(|group| group.water == step.water && group.upgrade == step.upgrade)
        {
            group.count += 1;
            group.total_cost += step.cost;
            group.end_level = step.level;
            group.cumulative_seconds = step.cumulative_seconds;
        } else {
            grouped.push(PlanGroup {
                water: step.water,
                upgrade: step.upgrade,
                name: step.name,
                count: 1,
                start_level: step.level - 1.0,
                end_level: step.level,
                total_cost: step.cost,
                currency: step.currency,
                cumulative_seconds: step.cumulative_seconds,
            });
        }
    }
    grouped.sort_by(|left, right| left.cumulative_seconds.total_cmp(&right.cumulative_seconds));
    grouped
}
