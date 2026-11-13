//! FN-04: the marble upgrade planner and optimizer.
//!
//! Ported from `_renderMarblePlanner` and `_renderMarbleOptimizer` in
//! `fountain-calc.html`, with `marbleBon`, `marbleCost` and `canMarble` from
//! `js/stats/systems/w5/fountain.js`. Only an upgrade the save owns can be
//! marbled, and a marble level multiplies that upgrade's bonus by
//! `1.5 + 0.5 × level`; both planners raise one marble level on a copy of the
//! cavern and read the same goal the upgrade planner reads.

use super::data::UPGRADES_PER_WATER;
use super::planner::pct_gain;
use super::{Fountain, Goal, WATERS};
use idleon_common::{Control, Progress};

/// The marble planner and optimizer controls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarbleRequest {
    pub goal: Goal,
    /// The simulated desired currency; `None` reads the save's choice.
    pub desired: Option<usize>,
    /// The optimizer's step cap.
    pub max_steps: usize,
    /// Skip an upgrade the marble balance cannot pay for.
    pub affordable: bool,
}

impl Default for MarbleRequest {
    fn default() -> Self {
        Self {
            goal: Goal::Currency,
            desired: None,
            max_steps: 50,
            affordable: true,
        }
    }
}

/// One next-marble-level candidate on the planner's table.
#[derive(Clone, Debug, PartialEq)]
pub struct MarbleRow {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    /// The saved marble level, which the row buys one above.
    pub marble_level: f64,
    pub cost: f64,
    pub affordable: bool,
    pub delta: f64,
    pub pct_gain: f64,
    /// The ranking key: percentage gain per marble.
    pub efficiency: f64,
    pub seconds_to_afford: f64,
    /// The coin fill time this marble level removes; only `fill` measures it.
    pub fill_delta: f64,
    pub marble_fill_delta: f64,
    /// `None` while the duck bar is locked.
    pub duck_fill_delta: Option<f64>,
    pub royal_chance: f64,
    pub royal_delta: f64,
    pub duck_chance: f64,
    pub duck_delta: f64,
    pub lucky_delta: f64,
    /// For `optimal`: the fill-speed and stack-value halves of the gain.
    pub speed_pct: f64,
    pub value_pct: f64,
}

/// One purchased marble level in the optimizer's plan.
#[derive(Clone, Debug, PartialEq)]
pub struct MarbleStep {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    /// The marble level after the purchase.
    pub marble_level: f64,
    pub cost: f64,
    pub delta: f64,
    pub pct_gain: f64,
    /// The goal's value after this step.
    pub value: f64,
    /// Marble seconds needed to reach this step, including any waits.
    pub cumulative_seconds: f64,
}

/// Consecutive purchases of one upgrade, as the optimizer's table lists them.
#[derive(Clone, Debug, PartialEq)]
pub struct MarbleGroup {
    pub water: usize,
    pub upgrade: usize,
    pub name: &'static str,
    pub count: usize,
    pub start_level: f64,
    pub end_level: f64,
    pub total_cost: f64,
    /// The elapsed time when the group's last level is reached.
    pub cumulative_seconds: f64,
}

/// The marble optimizer's whole answer.
#[derive(Clone, Debug, PartialEq)]
pub struct MarblePlan {
    pub goal: Goal,
    pub start_value: f64,
    pub end_value: f64,
    pub gain_pct: f64,
    pub steps: Vec<MarbleStep>,
    pub groups: Vec<MarbleGroup>,
    /// The marble the plan leaves.
    pub remaining: f64,
}

impl Fountain {
    /// `_renderMarblePlanner`'s candidates, ranked by gain per marble.
    ///
    /// One marble level of every upgrade the save owns, when the upgrade has a
    /// marble track, is measured on a copy of the cavern. A goal the marble
    /// level does not move is skipped, and so is an unaffordable upgrade when
    /// the caller asked for affordable steps only.
    pub fn marble_planner(&self, request: MarbleRequest) -> Vec<MarbleRow> {
        let desired = request.desired.unwrap_or_else(|| self.desired_currency());
        let goal = request.goal;
        let baseline = self.measure_goal(goal, desired);
        let base_fill = self.active_fill_time();
        let base_marble_fill = Self::marble_bar_fill_time() / self.active_speed();
        let base_duck_fill = self
            .duck_bar_unlocked()
            .then(|| Self::duck_bar_fill_time() / self.active_speed());
        let base_royal = self.royal_chance();
        let base_duck = if self.duck_bar_unlocked() {
            self.duck_chance()
        } else {
            0.0
        };
        let base_lucky = self.lucky_coin_base_chance();
        let (base_stacks, base_stack_value) = if goal == Goal::Optimal {
            (
                self.optimal_stacks_per_hr(),
                self.optimal_stack_value(desired),
            )
        } else {
            (0.0, 0.0)
        };
        let available = self.marble();
        let marble_per_hr = self.marble_per_hr();
        let mut rows = Vec::new();
        for water in 0..WATERS {
            for upgrade in 0..UPGRADES_PER_WATER {
                if self.upgrade_level(water, upgrade) <= 0.0 {
                    continue;
                }
                if !Self::can_marble(water, upgrade) {
                    continue;
                }
                let Some(entry) = super::upgrade(water, upgrade) else {
                    continue;
                };
                let level = self.marble_level(water, upgrade);
                let cost = Self::marble_cost(water, upgrade, level);
                let affordable = available >= cost;
                if request.affordable && !affordable {
                    continue;
                }
                let simulated = self.with_marble_level(water, upgrade, level + 1.0);
                let value = simulated.measure_goal(goal, desired);
                let delta = value - baseline;
                if delta <= 0.0 {
                    continue;
                }
                let pct_gain = pct_gain(baseline, delta);
                let efficiency = if cost > 0.0 { pct_gain / cost } else { 0.0 };
                let remaining = (cost - available).max(0.0);
                let seconds_to_afford = if remaining <= 0.0 {
                    0.0
                } else if marble_per_hr > 0.0 {
                    remaining / marble_per_hr * 3600.0
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
                        super::planner::ratio_pct(base_stacks, simulated.optimal_stacks_per_hr()),
                        super::planner::ratio_pct(
                            base_stack_value,
                            simulated.optimal_stack_value(desired),
                        ),
                    )
                } else {
                    (0.0, 0.0)
                };
                rows.push(MarbleRow {
                    water,
                    upgrade,
                    name: entry.name.as_str(),
                    marble_level: level,
                    cost,
                    affordable,
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

    /// `_renderMarbleOptimizer` without a progress report.
    pub fn marble_optimize(&self, request: MarbleRequest) -> MarblePlan {
        self.marble_optimize_with(request, |_| Control::Continue)
            .expect("a plan without reports is never asked to stop")
    }

    /// `_renderMarbleOptimizer`: greedily buy the best marble level.
    ///
    /// The saved marble balance is the budget. With `affordable` only levels
    /// the balance covers are considered. Without it a level may wait for the
    /// marble income to cover the rest, and the wait is added to the cumulative
    /// ETA; a level whose marble income is still zero is skipped instead.
    pub fn marble_optimize_with(
        &self,
        request: MarbleRequest,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<MarblePlan> {
        let desired = request.desired.unwrap_or_else(|| self.desired_currency());
        let goal = request.goal;
        let mut simulated = self.clone();
        let mut budget = self.marble();
        let start_value = self.measure_goal(goal, desired);
        let mut baseline = start_value;
        let mut clock = 0.0;
        let mut steps: Vec<MarbleStep> = Vec::new();
        let max_steps = request.max_steps;
        for step in 0..max_steps {
            let marble_per_hr = simulated.marble_per_hr();
            let mut best: Option<MarbleCandidate> = None;
            for water in 0..WATERS {
                for upgrade in 0..UPGRADES_PER_WATER {
                    if simulated.upgrade_level(water, upgrade) <= 0.0 {
                        continue;
                    }
                    if !Self::can_marble(water, upgrade) {
                        continue;
                    }
                    let level = simulated.marble_level(water, upgrade);
                    let cost = Self::marble_cost(water, upgrade, level);
                    let affordable = budget >= cost;
                    if request.affordable && !affordable {
                        continue;
                    }
                    if !request.affordable && !affordable && marble_per_hr <= 0.0 {
                        continue;
                    }
                    let candidate = simulated.with_marble_level(water, upgrade, level + 1.0);
                    let value = candidate.measure_goal(goal, desired);
                    let delta = value - baseline;
                    if delta <= 0.0 {
                        continue;
                    }
                    let pct_gain = pct_gain(baseline, delta);
                    let efficiency = if cost > 0.0 { pct_gain / cost } else { 0.0 };
                    let row = MarbleCandidate {
                        water,
                        upgrade,
                        name: super::upgrade(water, upgrade)
                            .map(|entry| entry.name.as_str())
                            .unwrap_or("Unknown"),
                        cost,
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
            let Some(best) = best else {
                break;
            };
            if !request.affordable && budget < best.cost {
                let marble_per_hr = simulated.marble_per_hr();
                if marble_per_hr <= 0.0 {
                    break;
                }
                let wait = (best.cost - budget) / marble_per_hr * 3600.0;
                budget += marble_per_hr * wait / 3600.0;
                clock += wait;
            }
            budget = (budget - best.cost).max(0.0);
            let level = simulated.marble_level(best.water, best.upgrade) + 1.0;
            simulated = simulated.with_marble_level(best.water, best.upgrade, level);
            baseline += best.delta;
            steps.push(MarbleStep {
                water: best.water,
                upgrade: best.upgrade,
                name: best.name,
                marble_level: level,
                cost: best.cost,
                delta: best.delta,
                pct_gain: best.pct_gain,
                value: baseline,
                cumulative_seconds: clock,
            });
            let progress = Progress {
                fraction: (step + 1) as f32 / max_steps.max(1) as f32,
                message: format!(
                    "Step {} of {max_steps}: {} → M{}",
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
            message: format!("Planned {} marble level(s).", steps.len()),
        }) == Control::Stop
        {
            return None;
        }
        Some(MarblePlan {
            goal,
            start_value,
            end_value: baseline,
            gain_pct: pct_gain(start_value, baseline - start_value),
            groups: groups(&steps),
            steps,
            remaining: budget,
        })
    }

    /// One marble level above the saved one, on a copy of the cavern.
    pub(super) fn with_marble_level(&self, water: usize, upgrade: usize, level: f64) -> Fountain {
        let mut simulated = self.clone();
        if let Some(cells) = simulated.marble_lvs.get_mut(water)
            && let Some(cell) = cells.get_mut(upgrade)
        {
            *cell = level;
        }
        simulated
    }
}

/// One simulated marble level, before it is written into the plan.
struct MarbleCandidate {
    water: usize,
    upgrade: usize,
    name: &'static str,
    cost: f64,
    delta: f64,
    pct_gain: f64,
    efficiency: f64,
}

/// The plan's per-upgrade groups, ordered by when their last level lands.
fn groups(steps: &[MarbleStep]) -> Vec<MarbleGroup> {
    let mut grouped: Vec<MarbleGroup> = Vec::new();
    for step in steps {
        if let Some(group) = grouped
            .iter_mut()
            .find(|group| group.water == step.water && group.upgrade == step.upgrade)
        {
            group.count += 1;
            group.total_cost += step.cost;
            group.end_level = step.marble_level;
            group.cumulative_seconds = step.cumulative_seconds;
        } else {
            grouped.push(MarbleGroup {
                water: step.water,
                upgrade: step.upgrade,
                name: step.name,
                count: 1,
                start_level: step.marble_level - 1.0,
                end_level: step.marble_level,
                total_cost: step.cost,
                cumulative_seconds: step.cumulative_seconds,
            });
        }
    }
    grouped.sort_by(|left, right| left.cumulative_seconds.total_cmp(&right.cumulative_seconds));
    grouped
}
