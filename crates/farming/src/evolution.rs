//! The evolution chain to a target crop, from `evolutionChainPlan` and
//! `evolutionStepPlan`: the chance every eligible plot of the family rolls the
//! next depth, and the growth cycles each step takes.
//!
//! A step plants the crop before its target on every eligible plot of the
//! family and rolls each plot independently, so the step's chance is the chance
//! at least one of them advances. The chain prices every remaining depth from
//! the deepest eligible plot to the target; a field with no eligible plot still
//! plans the whole chain, at the family's own first depth.

use super::account::Snapshot;
use super::allocation::at_least_one_chance;
use super::{Farm, Plot, SeedFamily, data};

impl Farm {
    /// `evolutionChainPlan(targetCropId)`: every remaining step to the target.
    ///
    /// `None` only when the target belongs to no seed family. A target at the
    /// family's first crop is a chain of no steps: it has no predecessor to
    /// evolve out of.
    pub fn evolution_chain(&self, target_crop: i64, snapshot: &Snapshot) -> Option<EvolutionChain> {
        let family = data().family_of(target_crop)?;
        let current_depth = self
            .plots
            .iter()
            .filter(|plot| plot.seed_family == (family.id as i64) && !plot.evolution_blocked)
            .map(|plot| plot.evolution)
            .max();
        let target_depth = (target_crop - family.start) as u32;
        let steps: Vec<EvolutionStep> = (current_depth.unwrap_or(0)..target_depth)
            .filter_map(|depth| self.evolution_step(family.start + i64::from(depth) + 1, snapshot))
            .collect();
        let plot_count = steps.first().map_or(0, |step| step.plot_count);
        Some(EvolutionChain {
            target_crop,
            family: family.id,
            current_depth: current_depth.map_or(-1, i64::from),
            target_depth,
            plot_count,
            steps,
        })
    }

    /// `evolutionStepPlan(targetCropId)`: one depth's chance and cost.
    ///
    /// Every eligible plot of the family rolls it after growing the crop before
    /// the target, whatever depth it sits at now, so the plot's own depth only
    /// decides where the chain starts.
    pub fn evolution_step(&self, target_crop: i64, snapshot: &Snapshot) -> Option<EvolutionStep> {
        let family = data().family_of(target_crop)?;
        if target_crop <= family.start {
            return None;
        }
        let predecessor_depth = (target_crop - family.start - 1) as u32;
        let chances: Vec<f64> = self
            .plots
            .iter()
            .enumerate()
            .filter(|(_, plot)| plot.seed_family == (family.id as i64) && !plot.evolution_blocked)
            .map(|(index, plot)| {
                let simulated = Plot {
                    evolution: predecessor_depth,
                    ..*plot
                };
                self.next_crop_chance(index, &simulated, snapshot)
            })
            .collect();
        let per_plot_min = chances.iter().copied().reduce(f64::min).unwrap_or(0.0);
        let per_plot_max = chances.iter().copied().reduce(f64::max).unwrap_or(0.0);
        Some(EvolutionStep {
            target_crop,
            family: family.id,
            predecessor_depth,
            plot_count: chances.len(),
            per_plot_min,
            per_plot_max,
            plan: probability_plan(
                at_least_one_chance(chances),
                cycle_seconds(family, snapshot),
            ),
        })
    }
}

/// `growthCycleSeconds` at the family's own requirement: the Medal family runs a
/// fixed seven-hour cycle, the rest scale with the growth rate, and the active
/// cadence floors them at one second.
fn cycle_seconds(family: &SeedFamily, snapshot: &Snapshot) -> f64 {
    if family.id == 6 {
        25_200.0
    } else {
        (14_400.0 * 1.5_f64.powf(family.id as f64) / snapshot.growth_rate).max(1.0)
    }
}

/// `probabilityPlan`: one stage's chance, its cycle and the attempts each
/// quantile needs.
fn probability_plan(chance: f64, cycle_seconds: f64) -> ProbabilityPlan {
    let chance = if chance.is_nan() {
        0.0
    } else {
        chance.clamp(0.0, 1.0)
    };
    let cycle = if cycle_seconds.is_nan() {
        0.0
    } else {
        cycle_seconds.max(0.0)
    };
    let expected_attempts = if chance > 0.0 {
        1.0 / chance
    } else {
        f64::INFINITY
    };
    let (median, p90, p95) = (
        attempts_for(chance, 0.5),
        attempts_for(chance, 0.9),
        attempts_for(chance, 0.95),
    );
    ProbabilityPlan {
        chance,
        cycle_seconds: cycle,
        expected_attempts,
        expected_seconds: expected_attempts * cycle,
        median_attempts: median,
        median_seconds: median * cycle,
        p90_attempts: p90,
        p90_seconds: p90 * cycle,
        p95_attempts: p95,
        p95_seconds: p95 * cycle,
    }
}

/// The attempts a quantile takes: one when the roll is certain, endless when it
/// never lands, else the ceiling of the geometric distribution's quantile.
fn attempts_for(chance: f64, quantile: f64) -> f64 {
    if chance <= 0.0 {
        f64::INFINITY
    } else if chance >= 1.0 {
        1.0
    } else {
        ((-quantile).ln_1p() / (-chance).ln_1p()).ceil()
    }
}

/// One stage of a chain, as `evolutionStepPlan` builds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EvolutionStep {
    /// The crop this step evolves into.
    pub target_crop: i64,
    pub family: usize,
    /// The depth every eligible plot grows before rolling.
    pub predecessor_depth: u32,
    /// How many plots of the family roll this step.
    pub plot_count: usize,
    /// The weakest and strongest plot's own chance.
    pub per_plot_min: f64,
    pub per_plot_max: f64,
    pub plan: ProbabilityPlan,
}

/// A stage's chance and the time its attempts take.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ProbabilityPlan {
    pub chance: f64,
    pub cycle_seconds: f64,
    pub expected_attempts: f64,
    pub expected_seconds: f64,
    pub median_attempts: f64,
    pub median_seconds: f64,
    pub p90_attempts: f64,
    pub p90_seconds: f64,
    pub p95_attempts: f64,
    pub p95_seconds: f64,
}

/// The remaining stages between the field and the target crop.
#[derive(Clone, Debug, PartialEq)]
pub struct EvolutionChain {
    pub target_crop: i64,
    pub family: usize,
    /// The deepest eligible plot's depth, or `-1` when the field carries none.
    pub current_depth: i64,
    pub target_depth: u32,
    /// The plots rolling the first step.
    pub plot_count: usize,
    pub steps: Vec<EvolutionStep>,
}

impl EvolutionChain {
    /// How many evolution rolls are left.
    pub fn step_count(&self) -> usize {
        self.steps.len()
    }

    /// The summed expectation of every stage, which the page reports as the
    /// chain's ETA.
    pub fn expected_seconds(&self) -> f64 {
        self.total(|plan| plan.expected_seconds)
    }

    pub fn median_seconds(&self) -> f64 {
        self.total(|plan| plan.median_seconds)
    }

    pub fn p90_seconds(&self) -> f64 {
        self.total(|plan| plan.p90_seconds)
    }

    pub fn p95_seconds(&self) -> f64 {
        self.total(|plan| plan.p95_seconds)
    }

    fn total(&self, field: fn(&ProbabilityPlan) -> f64) -> f64 {
        self.steps.iter().map(|step| field(&step.plan)).sum()
    }

    /// The upstream "Hardest Remaining Steps": the slowest stages first.
    pub fn hardest_steps(&self, limit: usize) -> Vec<&EvolutionStep> {
        let mut steps: Vec<&EvolutionStep> = self.steps.iter().collect();
        steps.sort_by(|a, b| b.plan.expected_seconds.total_cmp(&a.plan.expected_seconds));
        steps.truncate(limit);
        steps
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Plot;

    /// A farm that can roll evolutions: the upstream gate is Farming level 2.
    fn planted(plots: &[(usize, i64, u32, bool)]) -> Farm {
        let mut farm = Farm {
            farm_level: 2,
            ..Farm::default()
        };
        for (index, family, evolution, blocked) in plots {
            farm.plots[*index] = Plot {
                seed_family: *family,
                evolution: *evolution,
                evolution_blocked: *blocked,
                ..Plot::default()
            };
        }
        farm
    }

    #[test]
    fn the_chain_prices_every_remaining_depth() {
        // Family 2 runs crops 46-60, so a plot at depth 1 leaves 13 steps.
        let farm = planted(&[(0, 2, 1, false)]);
        let chain = farm
            .evolution_chain(60, &farm.snapshot())
            .expect("crop 60 is in a family");
        assert_eq!(chain.family, 2);
        assert_eq!(chain.current_depth, 1);
        assert_eq!(chain.target_depth, 14);
        assert_eq!(chain.step_count(), 13);
        assert_eq!(chain.steps.first().unwrap().target_crop, 48);
        assert_eq!(chain.steps.last().unwrap().target_crop, 60);
        assert_eq!(chain.target_crop, 60);
        assert_eq!(chain.plot_count, 1);
        assert!(chain.steps.iter().all(|step| step.family == 2));
    }

    #[test]
    fn a_field_without_the_family_still_plans_the_whole_chain() {
        let farm = planted(&[]);
        let chain = farm
            .evolution_chain(3, &farm.snapshot())
            .expect("crop 3 is in a family");
        assert_eq!(chain.current_depth, -1);
        assert_eq!(chain.step_count(), 3);
        assert_eq!(chain.plot_count, 0);
        // No plot rolls it, so the stage never lands and its ETA is endless.
        assert_eq!(chain.steps[0].plan.chance, 0.0);
        assert!(chain.expected_seconds().is_infinite());
        assert!(chain.median_seconds().is_infinite());
    }

    #[test]
    fn a_target_without_a_predecessor_is_an_empty_chain() {
        let farm = planted(&[(0, 0, 2, false)]);
        for start in [0, 21, 46, 61, 84, 107, 230] {
            let chain = farm
                .evolution_chain(start, &farm.snapshot())
                .expect("a family's first crop is still a chain");
            assert_eq!(chain.target_depth, 0);
            assert_eq!(chain.step_count(), 0);
            assert_eq!(chain.expected_seconds(), 0.0);
        }
        // A target the field has already passed leaves no steps either.
        let chain = farm.evolution_chain(1, &farm.snapshot()).unwrap();
        assert_eq!(chain.current_depth, 2);
        assert_eq!(chain.step_count(), 0);
        assert_eq!(chain.p90_seconds(), 0.0);
    }

    #[test]
    fn a_crop_outside_the_families_has_no_chain() {
        let farm = planted(&[(0, 0, 0, false)]);
        assert!(farm.evolution_chain(330, &farm.snapshot()).is_none());
        assert!(farm.evolution_chain(-1, &farm.snapshot()).is_none());
        assert!(farm.evolution_step(330, &farm.snapshot()).is_none());
        assert!(farm.evolution_step(0, &farm.snapshot()).is_none());
    }

    #[test]
    fn only_unblocked_plots_of_the_family_roll() {
        let farm = planted(&[
            (0, 1, 0, false),
            (1, 1, 0, false),
            (2, 1, 0, true),
            (3, 0, 0, false),
        ]);
        let step = farm
            .evolution_step(22, &farm.snapshot())
            .expect("crop 22 is family 1's second crop");
        assert_eq!(step.plot_count, 2);
        assert_eq!(step.predecessor_depth, 0);
        assert_eq!(step.per_plot_min, step.per_plot_max);
        let single = step.per_plot_min;
        let any = 1.0 - (1.0 - single) * (1.0 - single);
        assert!(
            (step.plan.chance - any).abs() < 1e-12,
            "{} vs {any}",
            step.plan.chance
        );
    }

    #[test]
    fn every_plot_rolls_at_the_predecessor_depth() {
        // The 9th crop of family 0 needs eight predecessors, so a plot saved
        // at depth 0 and one saved at depth 5 both roll the depth-8 chance.
        let farm = planted(&[(0, 0, 0, false), (1, 0, 5, false)]);
        let snapshot = farm.snapshot();
        let step = farm.evolution_step(9, &snapshot).unwrap();
        assert_eq!(step.predecessor_depth, 8);
        let deep = Plot {
            seed_family: 0,
            evolution: 8,
            ..Plot::default()
        };
        let chance = farm.next_crop_chance(0, &deep, &snapshot);
        assert!(chance > 0.0);
        assert!((step.per_plot_min - chance).abs() < 1e-12);
        assert!((step.plan.chance - (1.0 - (1.0 - chance).powi(2))).abs() < 1e-12);
    }

    #[test]
    fn a_probability_plan_reports_its_attempt_quantiles() {
        let plan = probability_plan(0.5, 100.0);
        assert_eq!(plan.expected_attempts, 2.0);
        assert_eq!(plan.expected_seconds, 200.0);
        assert_eq!(plan.median_attempts, 1.0);
        assert_eq!(plan.median_seconds, 100.0);
        assert_eq!(plan.p90_attempts, 4.0);
        assert_eq!(plan.p90_seconds, 400.0);
        assert_eq!(plan.p95_attempts, 5.0);
        assert_eq!(plan.p95_seconds, 500.0);

        let certain = probability_plan(2.0, 7.0);
        assert_eq!(certain.chance, 1.0);
        assert_eq!(certain.p95_attempts, 1.0);
        assert_eq!(certain.p95_seconds, 7.0);

        let never = probability_plan(0.0, 7.0);
        assert_eq!(never.expected_attempts, f64::INFINITY);
        assert!(never.p90_seconds.is_infinite());
        assert_eq!(never.cycle_seconds, 7.0);
    }

    #[test]
    fn the_medal_family_keeps_its_fixed_cycle() {
        // Growth rate zero stops every ordinary family's cycle, but the Medal
        // family's seven hours never scale.
        let farm = Farm::default();
        let mut snapshot = farm.snapshot();
        snapshot.growth_rate = 0.0;
        let ordinary = planted(&[(0, 1, 0, false)])
            .evolution_step(22, &snapshot)
            .unwrap();
        assert!(ordinary.plan.cycle_seconds.is_infinite());
        let medal = planted(&[(0, 6, 0, false)])
            .evolution_step(231, &snapshot)
            .unwrap();
        assert_eq!(medal.plan.cycle_seconds, 25_200.0);
    }

    #[test]
    fn the_slowest_steps_rank_first() {
        let farm = planted(&[(0, 5, 0, false)]);
        let chain = farm.evolution_chain(137, &farm.snapshot()).unwrap();
        assert_eq!(chain.step_count(), 30);
        let hardest = chain.hardest_steps(5);
        assert_eq!(hardest.len(), 5);
        assert_eq!(hardest[0].target_crop, 137);
        assert!(
            hardest
                .windows(2)
                .all(|pair| { pair[0].plan.expected_seconds >= pair[1].plan.expected_seconds })
        );
        assert_eq!(chain.hardest_steps(100).len(), 30);
        let total: f64 = chain
            .steps
            .iter()
            .map(|step| step.plan.expected_seconds)
            .sum();
        assert_eq!(chain.expected_seconds(), total);
        assert_eq!(
            chain.median_seconds(),
            chain
                .steps
                .iter()
                .map(|s| s.plan.median_seconds)
                .sum::<f64>()
        );
        assert_eq!(
            chain.p90_seconds(),
            chain.steps.iter().map(|s| s.plan.p90_seconds).sum::<f64>()
        );
        assert_eq!(
            chain.p95_seconds(),
            chain.steps.iter().map(|s| s.plan.p95_seconds).sum::<f64>()
        );
    }
}
