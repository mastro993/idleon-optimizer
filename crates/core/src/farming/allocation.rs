//! Land Rank allocation: spend the account's unspent points for one objective.
//!
//! Ported from `optimizeLandRank` and `_rankObjectiveEvaluator` in
//! `js/stats/systems/w6/farming-planner.js`. The allocator is a greedy marginal
//! search: every point goes to the unlocked upgrade that raises the objective's
//! score the most, and it stops as soon as no upgrade helps, so the result can
//! never score below the levels it started from.

use super::account::{RankTotals, Snapshot};
use super::assignment::{Objective, crop_weight};
use super::{Farm, PLOTS, RANK_UPGRADES, data, plot::overgrowth_multiplier};
use crate::model::{Control, Progress};

/// How many progress lines a long allocation reports.
const PROGRESS_STEPS: u32 = 50;

/// Whether the allocator keeps the saved levels or reallocates every point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Mode {
    #[default]
    Unspent,
    Reset,
}

impl Mode {
    /// Both modes, in the upstream order.
    pub const ALL: [Mode; 2] = [Mode::Unspent, Mode::Reset];
    /// The upstream id, used by the project document.
    pub fn id(self) -> &'static str {
        match self {
            Mode::Unspent => "unspent",
            Mode::Reset => "reset",
        }
    }

    /// The label the rank tab shows.
    pub fn label(self) -> &'static str {
        match self {
            Mode::Unspent => "Current levels retained",
            Mode::Reset => "Full reallocation",
        }
    }

    /// The mode behind an upstream id.
    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|mode| mode.id() == id)
    }
}

/// `optimizeLandRank`: the levels, points and scores of one allocation.
#[derive(Clone, Debug, PartialEq)]
pub struct Allocation {
    pub objective: Objective,
    pub mode: Mode,
    pub initial_levels: Vec<u32>,
    pub levels: Vec<u32>,
    pub points_available: u32,
    pub points_spent: u32,
    pub initial_score: f64,
    pub final_score: f64,
}

impl Allocation {
    /// Points the allocation could not spend; no unlocked upgrade helped.
    pub fn points_unused(&self) -> u32 {
        self.points_available.saturating_sub(self.points_spent)
    }

    /// `relativeGain`: the fraction the allocation gains over its baseline.
    pub fn relative_gain(&self) -> f64 {
        if self.initial_score > 0.0 {
            self.final_score / self.initial_score - 1.0
        } else if self.final_score > 0.0 {
            f64::INFINITY
        } else {
            0.0
        }
    }
}

/// `atLeastOneChance`: the chance at least one of the independent rolls lands.
pub(crate) fn at_least_one_chance(chances: impl IntoIterator<Item = f64>) -> f64 {
    let log_miss: f64 = chances
        .into_iter()
        .map(|chance| (-chance.clamp(0.0, 1.0)).ln_1p())
        .sum();
    -log_miss.exp_m1()
}

/// `RANK_OBJECTIVE_IDS`: the upgrade rows each objective can move. Growth has
/// no Land Rank channel at all, so it never spends a point.
fn rank_candidates(objective: Objective) -> &'static [usize] {
    match objective {
        Objective::Evolution => &[0, 3, 10, 15],
        Objective::CropValue | Objective::Beans => &[1, 8, 17],
        Objective::RankXp => &[2, 6, 13],
        Objective::FarmingExp => &[5, 12, 16],
        Objective::Overgrowth => &[7, 11, 18],
        Objective::Growth => &[],
    }
}

/// One objective's score for a candidate Land Rank total.
enum Evaluator {
    /// Growth speed has no Land Rank channel; the score never changes.
    Growth(f64),
    /// The chance at least one eligible plot evolves, per rank.
    Evolution {
        chance_base: f64,
        ranks: Vec<u32>,
        voting: f64,
    },
    /// The common evolution multiplier, when the target is a family's first crop.
    EvolutionMultiplier {
        base: f64,
    },
    Overgrowth {
        weight: f64,
        base: f64,
    },
    FarmingExp {
        weight: f64,
        base: f64,
    },
    RankXp {
        /// The previous plot's rank and this plot's per-second base.
        rows: Vec<(u32, f64)>,
        display: f64,
    },
    CropValue {
        /// The plot's rank and the factor its value multiplies.
        rows: Vec<(u32, f64)>,
        basket: f64,
        factor_low: f64,
        high_chance: f64,
        cap: f64,
        voting: f64,
    },
}

impl Evaluator {
    fn score(&self, rank: &RankTotals) -> f64 {
        match self {
            Evaluator::Growth(score) => *score,
            Evaluator::Evolution {
                chance_base,
                ranks,
                voting,
            } => {
                let common = chance_base * rank.evolution.max(1.0);
                at_least_one_chance(ranks.iter().map(|plot_rank| {
                    common * (1.0 + (rank.bonuses[0] * f64::from(*plot_rank) + voting) / 100.0)
                }))
            }
            Evaluator::EvolutionMultiplier { base } => base * rank.evolution.max(1.0),
            Evaluator::Overgrowth { weight, base } => {
                weight * base * (1.0 + rank.overgrowth / 100.0)
            }
            Evaluator::FarmingExp { weight, base } => {
                weight * base * (1.0 + rank.farming_exp / 100.0)
            }
            Evaluator::RankXp { rows, display } => {
                let sum: f64 = rows
                    .iter()
                    .map(|(previous, base)| {
                        base * (1.0 + rank.bonuses[2] * f64::from(*previous) / 100.0)
                    })
                    .sum();
                sum * (1.0 + rank.rank_xp / 100.0) * display.max(1.0)
            }
            Evaluator::CropValue {
                rows,
                basket,
                factor_low,
                high_chance,
                cap,
                voting,
            } => rows
                .iter()
                .map(|(plot_rank, factor)| {
                    let base = (1.0 + rank.crop_value / 100.0)
                        * basket
                        * (1.0 + (rank.bonuses[1] * f64::from(*plot_rank) + voting) / 100.0);
                    let low = (factor_low * base).round().min(*cap);
                    let high = ((factor_low + 1.0) * base).round().min(*cap);
                    factor * (low * (1.0 - high_chance) + high * high_chance)
                })
                .sum(),
        }
    }
}

impl Farm {
    /// Build the score function for one objective from the saved plots.
    fn evaluator(
        &self,
        snapshot: &Snapshot,
        objective: Objective,
        target_crop: Option<u32>,
    ) -> Evaluator {
        let plots: Vec<_> = (0..PLOTS)
            .map(|index| self.plots.get(index).copied().unwrap_or_default())
            .collect();
        let cycles: Vec<f64> = plots
            .iter()
            .map(|plot| self.growth_cycle_seconds(plot, snapshot))
            .collect();
        let active = |cycle: f64| cycle.is_finite() && cycle > 0.0;
        match objective {
            Objective::Growth => Evaluator::Growth(
                cycles
                    .iter()
                    .filter(|cycle| active(**cycle))
                    .map(|cycle| 1.0 / cycle)
                    .sum(),
            ),
            Objective::Evolution => {
                let family = target_crop
                    .map(i64::from)
                    .and_then(|target| data().family_of(target).map(|family| (target, family)));
                match family {
                    Some((target, family)) if target > family.start => {
                        // The depth the target's predecessor sits at decides
                        // the roll that has to land.
                        let denom = if family.evolution_denom == 0.000_069_42 {
                            1e-110
                        } else {
                            family.evolution_denom
                        };
                        let chance_base = 0.3
                            * denom.powf((target - family.start - 1) as f64)
                            * snapshot.next_crop_common
                            / snapshot.rank.evolution.max(1.0);
                        let ranks = plots
                            .iter()
                            .enumerate()
                            .filter(|(_, plot)| {
                                plot.seed_family == family.id as i64 && !plot.evolution_blocked
                            })
                            .map(|(index, _)| self.rank_ranks.get(index).copied().unwrap_or(0))
                            .collect();
                        Evaluator::Evolution {
                            chance_base,
                            ranks,
                            voting: snapshot.voting29,
                        }
                    }
                    _ => Evaluator::EvolutionMultiplier {
                        base: snapshot.next_crop_common / snapshot.rank.evolution.max(1.0),
                    },
                }
            }
            Objective::Overgrowth => Evaluator::Overgrowth {
                weight: plots
                    .iter()
                    .zip(&cycles)
                    .filter(|(_, cycle)| active(**cycle))
                    .map(|(plot, cycle)| 0.4_f64.powf(f64::from(plot.overgrowth) + 1.0) / cycle)
                    .sum(),
                base: snapshot.overgrowth_common / (1.0 + snapshot.rank.overgrowth / 100.0),
            },
            Objective::FarmingExp => Evaluator::FarmingExp {
                weight: plots
                    .iter()
                    .zip(&cycles)
                    .filter(|(plot, cycle)| plot.seed_family >= 0 && active(**cycle))
                    .map(|(plot, cycle)| {
                        let family = plot.seed_family as f64;
                        (5.0 + 25.0
                            * family
                            * 2_f64.powf(family)
                            * 1.25_f64.powf(f64::from(plot.evolution)))
                            * overgrowth_multiplier(plot.overgrowth)
                            / cycle
                    })
                    .sum(),
                base: snapshot.farming_exp / (1.0 + snapshot.rank.farming_exp / 100.0),
            },
            Objective::RankXp => {
                let unlocked = self.basket_upgrade(99, 5) >= 1.0;
                let market = 1.0 + self.basket_upgrade(0, 7) / 100.0;
                let mut rows = Vec::new();
                if unlocked {
                    for (index, plot) in plots.iter().enumerate() {
                        let cycle = cycles[index];
                        if !active(cycle) {
                            continue;
                        }
                        // Land Rank XP needs the previous plot ranked first.
                        let previous = self
                            .rank_ranks
                            .get(index.saturating_sub(1))
                            .copied()
                            .unwrap_or(0);
                        if index > 0 && previous < 1 {
                            continue;
                        }
                        rows.push((
                            previous,
                            market
                                * (plot.seed_family + 1) as f64
                                * overgrowth_multiplier(plot.overgrowth)
                                / cycle,
                        ));
                    }
                }
                Evaluator::RankXp {
                    rows,
                    display: snapshot.plot_rank_display,
                }
            }
            Objective::CropValue | Objective::Beans => {
                let product = (snapshot.product_bonus / 100.0).max(0.0);
                let mut rows = Vec::new();
                for (index, plot) in plots.iter().enumerate() {
                    let cycle = cycles[index];
                    if !active(cycle) {
                        continue;
                    }
                    let factor =
                        snapshot.expected_vines * overgrowth_multiplier(plot.overgrowth) / cycle;
                    let factor = if objective == Objective::Beans {
                        factor * crop_weight(self.crop_type(plot))
                    } else {
                        factor
                    };
                    rows.push((self.rank_ranks.get(index).copied().unwrap_or(0), factor));
                }
                Evaluator::CropValue {
                    rows,
                    basket: self.basket_upgrade(99, 6).max(1.0),
                    factor_low: 1.0 + product.floor(),
                    high_chance: product - product.floor(),
                    cap: snapshot.crop_value_cap,
                    voting: snapshot.voting29,
                }
            }
        }
    }

    /// `optimizeLandRank`: spend the available points on one objective.
    ///
    /// Returns `None` when the reporter stopped the run, so a cancelled
    /// allocation stores nothing.
    pub fn allocation(
        &self,
        objective: Objective,
        mode: Mode,
        target_crop: Option<u32>,
        report: &mut dyn FnMut(Progress) -> Control,
    ) -> Option<Allocation> {
        let initial_levels: Vec<u32> = match mode {
            Mode::Reset => vec![0; RANK_UPGRADES],
            Mode::Unspent => (0..RANK_UPGRADES)
                .map(|index| self.rank_levels.get(index).copied().unwrap_or(0))
                .collect(),
        };
        let points_available = match mode {
            Mode::Reset => self.total_land_rank(),
            Mode::Unspent => u32::try_from(self.land_rank_points_left().max(0)).unwrap_or(0),
        };
        let total_rank = self.total_land_rank();
        let fifth_max = self.fifth_column_max();
        let evaluator = self.evaluator(&self.snapshot(), objective, target_crop);
        let candidates = rank_candidates(objective);
        let mut levels = initial_levels.clone();
        let mut score = evaluator.score(&self.rank_totals_at(&levels));
        let initial_score = score;
        let stride = (points_available / PROGRESS_STEPS).max(1);
        let mut spent = 0;
        for point in 0..points_available {
            if point % stride == 0 {
                let progress = Progress {
                    fraction: point as f32 / points_available.max(1) as f32,
                    message: format!("Spending point {} of {points_available}…", point + 1),
                };
                if report(progress) == Control::Stop {
                    return None;
                }
            }
            let mut best: Option<(usize, f64)> = None;
            for index in candidates {
                if total_rank < data().rank_unlocks.get(*index).copied().unwrap_or(0) {
                    continue;
                }
                if index % 5 == 4 && levels[*index] >= fifth_max {
                    continue;
                }
                levels[*index] += 1;
                let candidate = evaluator.score(&self.rank_totals_at(&levels));
                levels[*index] -= 1;
                let threshold = best.map_or(score, |(_, value)| value);
                if candidate > threshold {
                    best = Some((*index, candidate));
                }
            }
            let Some((index, value)) = best else {
                break;
            };
            levels[index] += 1;
            score = value;
            spent += 1;
        }
        report(Progress {
            fraction: 1.0,
            message: format!("Allocated {spent} point(s) for {}.", objective.label()),
        });
        Some(Allocation {
            objective,
            mode,
            initial_levels,
            levels,
            points_available,
            points_spent: spent,
            initial_score,
            final_score: score,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::at_least_one_chance;

    #[test]
    fn at_least_one_chance_keeps_the_rolls_independent() {
        let none: [f64; 0] = [];
        assert_eq!(at_least_one_chance(none), 0.0);
        assert!((at_least_one_chance([0.3]) - 0.3).abs() < 1e-12);
        assert!((at_least_one_chance([0.3, 0.3]) - 0.51).abs() < 1e-12);
        assert!((at_least_one_chance([1.0, 0.0]) - 1.0).abs() < 1e-12);
    }
}
