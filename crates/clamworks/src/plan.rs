//! The upgrade plan: what a pearl budget buys, in the order the optimizer buys it.
//!
//! The upstream calculator ranks the upgrades once (`rankUpgrades` in
//! `js/stats/systems/w7/clamworks.js`). A plan repeats that ranking: each
//! purchase is the best marginal pearls-per-kill gain per pearl the budget can
//! afford at the levels the plan has already reached, so every step pays the
//! `upgNextCost` the game would charge at that point — FRUGALITY and ANTI
//! INFLATION included, never a cheaper earlier price.
//!
//! Utility upgrades rank at zero pearls per kill, so the greedy pass leaves them
//! alone; the upstream table lists them separately for exactly that reason.

use super::Clamworks;
use crate::data;
use idleon_common::Control;
use idleon_common::Progress;

/// The most purchases one plan may make, so a huge balance stays a bounded run.
pub const MAX_STEPS: usize = 500;

/// What a plan may spend and the multikill total it values pearls at.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Input {
    /// Pearls available for the whole plan.
    pub budget: f64,
    /// The multikill total Multi-Scalping reads.
    pub multikill_total: f64,
}

impl Input {
    /// The input with both values finite and non-negative.
    pub fn normalized(self) -> Self {
        Self {
            budget: non_negative(self.budget),
            multikill_total: non_negative(self.multikill_total),
        }
    }
}

/// A finite value clamped at zero; NaN and the infinities read as zero.
fn non_negative(value: f64) -> f64 {
    if value.is_finite() {
        value.max(0.0)
    } else {
        0.0
    }
}

/// One purchase: `index` goes from `from_level` to `from_level + 1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    /// The pearl upgrade bought, in the game's own order.
    pub index: usize,
    /// The level it was at before this purchase.
    pub from_level: u32,
    /// What the level costs at the plan's levels when it is bought.
    pub cost: f64,
    /// The fractional pearls-per-kill gain the upstream rank priced it at.
    pub gain: f64,
}

impl Step {
    /// The level the purchase reaches.
    pub fn to_level(&self) -> u32 {
        self.from_level + 1
    }
}

/// The purchases one budget buys.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    /// The levels the plan starts from.
    pub start: Clamworks,
    /// The levels after the last purchase.
    pub end: Clamworks,
    pub steps: Vec<Step>,
    /// The sum of every step's cost.
    pub spent: f64,
    pub budget: f64,
    pub multikill_total: f64,
    /// Whether the step cap, not the budget, ended the plan.
    pub capped: bool,
}

impl Plan {
    /// The budget left after every purchase.
    pub fn remaining(&self) -> f64 {
        (self.budget - self.spent).max(0.0)
    }

    /// Pearls per kill at the levels the plan starts from.
    pub fn pearls_per_kill_before(&self) -> f64 {
        self.start.pearls_per_kill(self.multikill_total)
    }

    /// Pearls per kill at the levels the plan reaches.
    pub fn pearls_per_kill_after(&self) -> f64 {
        self.end.pearls_per_kill(self.multikill_total)
    }

    /// Fractional pearls-per-kill gain across the whole plan.
    pub fn gain(&self) -> f64 {
        let before = self.pearls_per_kill_before();
        if before <= 0.0 {
            return 0.0;
        }
        self.pearls_per_kill_after() / before - 1.0
    }

    /// Pearl value at the levels the plan starts from.
    pub fn pearl_value_before(&self) -> f64 {
        self.start.pearl_value(self.multikill_total)
    }

    /// Pearl value at the levels the plan reaches.
    pub fn pearl_value_after(&self) -> f64 {
        self.end.pearl_value(self.multikill_total)
    }

    /// Why the plan stopped, as the pane explains it.
    pub fn stop_reason(&self) -> &'static str {
        if self.capped {
            "Stopped at the step cap; buy again to spend the rest."
        } else if self.steps.is_empty() {
            "Nothing affordable moves pearls per kill."
        } else {
            "The budget buys no further pearls-per-kill gain."
        }
    }
}

impl Clamworks {
    /// The plan a budget buys, reporting each purchase; `None` when stopped.
    ///
    /// The first affordable upgrade in the upstream ranking is bought, then the
    /// ranking is recomputed at the new levels: a purchase that costs more than
    /// the remaining budget is skipped in favour of the next one, never
    /// overbought.
    pub fn plan_with(
        &self,
        input: Input,
        mut report: impl FnMut(Progress) -> Control,
    ) -> Option<Plan> {
        let Input {
            budget,
            multikill_total,
        } = input.normalized();
        let mut current = self.clone();
        let mut steps: Vec<Step> = Vec::new();
        let mut spent = 0.0;
        let mut capped = false;
        loop {
            if steps.len() >= MAX_STEPS {
                capped = true;
                break;
            }
            let remaining = budget - spent;
            if remaining <= 0.0 {
                break;
            }
            let Some(rank) = current
                .priority(multikill_total)
                .into_iter()
                .find(|rank| rank.ppk_gain > 0.0 && rank.next_cost <= remaining)
            else {
                break;
            };
            let from_level = current.upgrades[rank.index];
            spent += rank.next_cost;
            current.upgrades[rank.index] = from_level + 1;
            steps.push(Step {
                index: rank.index,
                from_level,
                cost: rank.next_cost,
                gain: rank.ppk_gain,
            });
            let name = &data::clamworks().upgrades[rank.index].name;
            let progress = Progress {
                fraction: if budget > 0.0 {
                    (spent / budget).min(1.0) as f32
                } else {
                    1.0
                },
                message: format!("{name} → Lv {} · {:.0} pearls", from_level + 1, spent),
            };
            if report(progress) == Control::Stop {
                return None;
            }
        }
        Some(Plan {
            start: self.clone(),
            end: current,
            steps,
            spent,
            budget,
            multikill_total,
            capped,
        })
    }

    /// The plan a budget buys, without progress reports.
    pub fn plan(&self, input: Input) -> Plan {
        self.plan_with(input, |_| Control::Continue)
            .expect("a reporter that always continues never stops the plan")
    }
}
