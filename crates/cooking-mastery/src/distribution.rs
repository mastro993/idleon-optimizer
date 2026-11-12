//! The daily distribution: the mastery point budget spent across the upgrades.
//!
//! The Mastery EXP tab keeps its "Optimal Distribution (Daily Reset)" maths
//! inline too, so this module names the page function it rewrites:
//! [`Mastery::distribution`] is `optimizeDistribution`: every mastery point the
//! rank and the Rift Spooker companion grant (`totalPts`) goes to the unlocked
//! EXP upgrade with the biggest marginal multiplier, starting from zero, so the
//! plan is a full reallocation rather than an increment on the saved levels.
//!
//! The page picks that upgrade with `newMulti / curMulti` and a strict `>`, so a
//! tie keeps the game's own upgrade order; this port does the same, which is why
//! the oracle compares the whole plan and not only its rate.
//!
//! The page has one objective, the EXP rate. [`Goal`] adds the second choice the
//! section's inputs ask for, derived from the page's own tables: the same budget
//! spent at the rank that unlocks the next upgrade. `RANK_REQ` decides that rank
//! and `scalingFactor(5)` grows with it, so the plan changes once SOUR (rank 25)
//! or SAVORY (rank 100) joins the comparison; a save with every upgrade unlocked
//! gets the same plan either way.

use super::{BASE_RATE, COMPANION_POINTS, EXP_UPGRADES, Mastery, REQUIRED_LEVEL, UPGRADE_NAMES};
use idleon_common::formula::ValueFormat;

/// What the daily distribution plans for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Goal {
    /// The rate the upgrades give at the current mastery rank, the page's own
    /// `optimizeDistribution` plan.
    #[default]
    Now,
    /// The rate the same budget gives at the rank that unlocks the next upgrade:
    /// holding the points until then buys more.
    NextUnlock,
}

impl Goal {
    pub const ALL: [Goal; 2] = [Goal::Now, Goal::NextUnlock];

    pub fn id(self) -> &'static str {
        match self {
            Goal::Now => "now",
            Goal::NextUnlock => "next-unlock",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Goal::Now => "EXP rate now",
            Goal::NextUnlock => "EXP rate at the next unlock",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|goal| goal.id() == id)
    }
}

/// One row of the distribution table: the saved level and the plan's level.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The upgrade's index in [`UPGRADE_NAMES`].
    pub index: usize,
    pub name: &'static str,
    /// `getUpgLevel(t)`: the level the save holds.
    pub current: f64,
    /// The level the plan reaches.
    pub planned: f64,
    /// `planned − current`; negative when the plan moves points away.
    pub change: f64,
    /// Whether the plan's rank still leaves the upgrade locked.
    pub locked: bool,
    /// The mastery rank the upgrade needs.
    pub required_level: u32,
    /// `bonusTotal(t, planned)`: the planned bonus, as a percentage.
    pub bonus: f64,
    /// `expMultiplier(t, planned)`: the factor the plan reaches.
    pub multiplier: f64,
}

/// The optimal distribution of one budget, as the section shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Plan {
    pub goal: Goal,
    /// The mastery rank the save holds.
    pub current_rank: f64,
    /// The mastery rank the plan is computed at.
    pub rank: f64,
    /// The points the rank and the companion grant there (`totalPts`).
    pub available: f64,
    /// The budget asked for, before clamping.
    pub requested: f64,
    /// The points the plan actually spends: the request clamped to the budget.
    pub budget: f64,
    /// The planned level of each upgrade, by [`UPGRADE_NAMES`] index.
    pub levels: [f64; 6],
    /// The EXP rate the planned levels give, external multipliers included.
    pub rate: f64,
    /// The rate the save's own levels give.
    pub current_rate: f64,
    /// `(rate − current_rate) / current_rate`, as a percentage.
    pub improvement: f64,
    /// The page's `optTimeToLevel`: the saved level's requirement over the
    /// planned rate, so the goal-switch plans stay comparable.
    pub hours: f64,
    /// Hours to the next mastery level at the saved rate.
    pub current_hours: f64,
    /// One row per EXP upgrade, in the game's order.
    pub rows: Vec<Row>,
}

impl Plan {
    /// The rank the plan holds the budget for, when it is not the current one.
    pub fn waits_for_rank(&self) -> Option<f64> {
        (self.rank > self.current_rank).then_some(self.rank)
    }

    /// The plan as plain text: the section's copy action.
    pub fn copy_text(&self) -> String {
        let points = if self.budget >= self.available {
            format!("All {} points", whole(self.budget))
        } else {
            format!("{} of {} points", whole(self.budget), whole(self.available))
        };
        let mut text = format!(
            "Cooking Mastery · optimal distribution ({})\n{} at mastery rank {}\n",
            self.goal.label(),
            points,
            whole(self.rank),
        );
        if let Some(rank) = self.waits_for_rank() {
            text.push_str(&format!(
                "Held until rank {rank}: the plan spends the same points once the upgrade unlocks.\n"
            ));
        }
        text.push_str(&format!(
            "EXP rate {} ({:+.2}% vs current) · {:.1} h to the next level\n",
            ValueFormat::Rate.format(self.rate),
            self.improvement,
            self.hours,
        ));
        text.push('\n');
        for row in &self.rows {
            text.push_str(&format!(
                "{:<22} {} → {} ({:+})  {:.2}% bonus{}\n",
                row.name,
                whole(row.current),
                whole(row.planned),
                row.change,
                row.bonus,
                if row.locked {
                    format!("  locked: needs rank {}", row.required_level)
                } else {
                    String::new()
                },
            ));
        }
        text
    }
}

/// A point count as the page prints it: whole numbers only.
fn whole(value: f64) -> String {
    format!("{value:.0}")
}

impl Mastery {
    /// `totalPts`: the points the mastery rank and the companion grant.
    ///
    /// The page adds the companion's five points but not the research grid,
    /// event shop or jelly sources the yellow point allocator counts.
    pub(crate) fn points_earned_at(&self, rank: f64) -> f64 {
        rank + 1.0
            + if self.companion_87_owned {
                COMPANION_POINTS
            } else {
                0.0
            }
    }

    /// The rank [`Goal::NextUnlock`] plans for: the lowest requirement above the
    /// mastery rank, or the rank itself once every upgrade is unlocked.
    pub(crate) fn next_unlock_rank(&self) -> f64 {
        REQUIRED_LEVEL
            .iter()
            .map(|level| f64::from(*level))
            .filter(|level| *level > self.level)
            .fold(None::<f64>, |lowest, level| {
                Some(lowest.map_or(level, |lowest| lowest.min(level)))
            })
            .unwrap_or(self.level)
    }

    /// The rank a goal plans for.
    pub(crate) fn plan_rank(&self, goal: Goal) -> f64 {
        match goal {
            Goal::Now => self.level,
            Goal::NextUnlock => self.next_unlock_rank(),
        }
    }

    /// `scalingFactor(t)` at another mastery rank: only SAVORY reads the rank.
    pub(crate) fn scaling_at(&self, index: usize, rank: f64) -> f64 {
        if index == 5 {
            rank + 1.0
        } else {
            self.scaling(index)
        }
    }

    /// `bonusTotal(t, level)` at another mastery rank, as a percentage.
    pub(crate) fn bonus_at(&self, index: usize, level: f64, rank: f64) -> f64 {
        self.per_lv(index) * level * self.scaling_at(index, rank)
    }

    /// `expMultiplier(t, level)` at another mastery rank.
    pub(crate) fn multiplier_at(&self, index: usize, level: f64, rank: f64) -> f64 {
        1.0 + self.bonus_at(index, level, rank) / 100.0
    }

    /// `computeExpRate` for the given levels at the given mastery rank.
    pub(crate) fn rate_at(&self, levels: &[f64; 6], rank: f64) -> f64 {
        EXP_UPGRADES.iter().fold(BASE_RATE, |rate, index| {
            rate * self.multiplier_at(*index, levels[*index], rank)
        }) * self.external.total()
    }

    /// `optimizeDistribution(totalPoints)`: the budget across the upgrades.
    ///
    /// The request is clamped to `[0, totalPts]` the way the page's input is, and
    /// floored to whole points because the page spends one point per iteration.
    pub fn distribution(&self, goal: Goal, budget: f64) -> Plan {
        let rank = self.plan_rank(goal);
        let available = self.points_earned_at(rank);
        let requested = budget;
        let budget = budget.max(0.0).min(available).floor();
        let mut levels = [0.0; 6];
        let mut spent = 0.0;
        while spent < budget {
            let mut best: Option<usize> = None;
            let mut best_ratio = 0.0;
            for index in EXP_UPGRADES {
                if rank < f64::from(REQUIRED_LEVEL[index]) {
                    continue;
                }
                let current = self.multiplier_at(index, levels[index], rank);
                let next = self.multiplier_at(index, levels[index] + 1.0, rank);
                let ratio = next / current;
                if ratio > best_ratio {
                    best_ratio = ratio;
                    best = Some(index);
                }
            }
            let Some(index) = best else {
                break;
            };
            levels[index] += 1.0;
            spent += 1.0;
        }

        let rate = self.rate_at(&levels, rank);
        let current_rate = self.exp_rate();
        let rows = EXP_UPGRADES
            .iter()
            .map(|index| {
                let index = *index;
                let current = self.upgrades[index];
                let planned = levels[index];
                Row {
                    index,
                    name: UPGRADE_NAMES[index],
                    current,
                    planned,
                    change: planned - current,
                    locked: rank < f64::from(REQUIRED_LEVEL[index]),
                    required_level: REQUIRED_LEVEL[index],
                    bonus: self.bonus_at(index, planned, rank),
                    multiplier: self.multiplier_at(index, planned, rank),
                }
            })
            .collect();
        Plan {
            goal,
            current_rank: self.level,
            rank,
            available,
            requested,
            budget,
            levels,
            rate,
            current_rate,
            improvement: if current_rate > 0.0 {
                (rate - current_rate) / current_rate * 100.0
            } else {
                0.0
            },
            hours: if rate > 0.0 {
                self.exp_required() / rate
            } else {
                f64::INFINITY
            },
            current_hours: self.hours_to_next_level(),
            rows,
        }
    }
}
