//! Per-plot formulas: growth, evolution, overgrowth, value, EXP and rank XP.

use super::account::Snapshot;
use super::{Farm, PLOTS, Plot, data};

impl Farm {
    /// Every plot's metrics under one snapshot.
    pub fn all_plot_metrics(&self) -> Vec<PlotMetrics> {
        let snapshot = self.snapshot();
        (0..PLOTS)
            .map(|index| self.plot_metrics_with(index, &snapshot))
            .collect()
    }

    /// `plotMetrics(plotIdx)` from the planner.
    pub fn plot_metrics_with(&self, index: usize, snapshot: &Snapshot) -> PlotMetrics {
        let plot = self.plots.get(index).copied().unwrap_or_default();
        let crop_value = self.crop_value(index, &plot, snapshot);
        let rank = self.rank_ranks.get(index).copied().unwrap_or(0);
        let rank_progress = self.rank_progress.get(index).copied().unwrap_or(0.0);
        PlotMetrics {
            plot: index,
            seed_family: plot.seed_family,
            crop_id: self.crop_type(&plot),
            growth_progress: plot.growth_progress,
            evolution: plot.evolution,
            evolution_blocked: plot.evolution_blocked,
            crops_on_vine: plot.crops_on_vine,
            overgrowth: plot.overgrowth,
            overflow_progress: plot.overflow_progress,
            growth_requirement: self.growth_requirement(&plot, snapshot),
            growth_seconds: self.growth_seconds(&plot, snapshot),
            next_crop_chance: self.next_crop_chance(index, &plot, snapshot),
            next_overgrowth_chance: self.next_overgrowth_chance(&plot, snapshot),
            crop_value,
            farming_exp: self.farming_exp_per_harvest(&plot, snapshot),
            rank,
            rank_progress,
            rank_requirement: land_rank_requirement(rank),
            rank_xp: self.rank_xp_per_harvest(index, snapshot),
        }
    }

    /// `cropType(plotIdx)`: the family start plus its evolution depth.
    pub(crate) fn crop_type(&self, plot: &Plot) -> i64 {
        if plot.seed_family < 0 {
            return -1;
        }
        data()
            .seed(plot.seed_family as usize)
            .map(|family| family.start + i64::from(plot.evolution))
            .unwrap_or(-1)
    }

    /// `growthRequirement`: $14400 \cdot 1.5^{family}$, and the fixed
    /// seven-hour Medal cycle scaled by the growth rate.
    pub fn growth_requirement(&self, plot: &Plot, snapshot: &Snapshot) -> f64 {
        if plot.seed_family < 0 {
            return 0.0;
        }
        if plot.seed_family == 6 {
            25_200.0 * snapshot.growth_rate
        } else {
            14_400.0 * 1.5_f64.powf(plot.seed_family as f64)
        }
    }

    /// `growthSeconds`: the remaining requirement over the growth rate.
    pub fn growth_seconds(&self, plot: &Plot, snapshot: &Snapshot) -> f64 {
        let required = self.growth_requirement(plot, snapshot);
        if required <= 0.0 {
            return f64::INFINITY;
        }
        (required - plot.growth_progress).max(0.0) / snapshot.growth_rate
    }

    /// `growthCycleSeconds`: the whole cycle at the active cadence, floored at
    /// one second so an instantaneous cycle stays finite.
    pub(crate) fn growth_cycle_seconds(&self, plot: &Plot, snapshot: &Snapshot) -> f64 {
        let required = self.growth_requirement(plot, snapshot);
        if required <= 0.0 || snapshot.growth_rate <= 0.0 {
            return f64::INFINITY;
        }
        (required / snapshot.growth_rate).max(1.0)
    }

    /// `nextCropChance`: the per-growth-tick evolution roll.
    pub fn next_crop_chance(&self, index: usize, plot: &Plot, snapshot: &Snapshot) -> f64 {
        if plot.seed_family < 0 {
            return 0.0;
        }
        let Some(family) = data().seed(plot.seed_family as usize) else {
            return 0.0;
        };
        if snapshot.farm_level < 2 || family.start + i64::from(plot.evolution) >= family.end {
            return 0.0;
        }
        let rank = self.rank_ranks.get(index).copied().unwrap_or(0);
        // The upstream guards a denormal data entry with an effectively zero
        // denominator; keep that guard.
        let denom = if family.evolution_denom == 0.000_069_42 {
            1e-110
        } else {
            family.evolution_denom
        };
        0.3 * denom.powf(f64::from(plot.evolution))
            * snapshot.next_crop_common
            * (1.0 + (snapshot.rank.bonuses[0] * f64::from(rank) + snapshot.voting29) / 100.0)
    }

    /// `nextOvergrowthChance`: the halving roll per overgrowth level.
    pub fn next_overgrowth_chance(&self, plot: &Plot, snapshot: &Snapshot) -> f64 {
        0.4_f64.powf(f64::from(plot.overgrowth) + 1.0) * snapshot.overgrowth_common
    }

    /// `cropValueDetails(plotIdx)`: base value, the cap, the doubled-harvest
    /// roll and the expected harvest.
    pub fn crop_value(&self, index: usize, plot: &Plot, snapshot: &Snapshot) -> CropValue {
        let rank = self.rank_ranks.get(index).copied().unwrap_or(0);
        let base = (1.0 + snapshot.rank.crop_value / 100.0)
            * self.basket_upgrade(99, 6).max(1.0)
            * (1.0 + (snapshot.rank.bonuses[1] * f64::from(rank) + snapshot.voting29) / 100.0);
        let raw = base.round();
        let cap = snapshot.crop_value_cap;
        let value = raw.min(cap);
        let product = (snapshot.product_bonus / 100.0).max(0.0);
        let product_low = 1.0 + product.floor();
        let product_high_chance = product - product.floor();
        let low = (product_low * base).round().min(cap);
        let high = ((product_low + 1.0) * base).round().min(cap);
        let expected = low * (1.0 - product_high_chance) + high * product_high_chance;
        CropValue {
            base,
            raw,
            cap,
            value,
            capped: raw >= cap,
            expected,
            product_low,
            product_high: product_low + 1.0,
            product_high_chance,
            expected_vines: snapshot.expected_vines,
            expected_harvest: snapshot.expected_vines
                * overgrowth_multiplier(plot.overgrowth)
                * expected,
        }
    }

    /// `farmingExpPerHarvest`: family depth, evolution and overgrowth.
    pub(crate) fn farming_exp_per_harvest(&self, plot: &Plot, snapshot: &Snapshot) -> f64 {
        if plot.seed_family < 0 {
            return 0.0;
        }
        let family = plot.seed_family as f64;
        (5.0 + 25.0 * family * 2_f64.powf(family) * 1.25_f64.powf(f64::from(plot.evolution)))
            * snapshot.farming_exp
            * overgrowth_multiplier(plot.overgrowth)
    }

    /// `rankXpPerHarvest`: the previous plot's rank and the Land Rank bonuses.
    pub(crate) fn rank_xp_per_harvest(&self, index: usize, snapshot: &Snapshot) -> f64 {
        let plot = self.plots.get(index).copied().unwrap_or_default();
        self.rank_xp_for(index, &plot, snapshot)
    }

    /// `rankXpPerHarvest` for a plot the farm may not carry, which the
    /// assignment scores as a hypothetical crop.
    pub(crate) fn rank_xp_for(&self, index: usize, plot: &Plot, snapshot: &Snapshot) -> f64 {
        if self.basket_upgrade(99, 5) < 1.0 {
            return 0.0;
        }
        let previous = self.rank_ranks.get(index.max(1) - 1).copied().unwrap_or(0);
        if index > 0 && previous < 1 {
            return 0.0;
        }
        (1.0 + self.basket_upgrade(0, 7) / 100.0)
            * (1.0 + snapshot.rank.bonuses[2] * f64::from(previous) / 100.0)
            * (plot.seed_family + 1) as f64
            * overgrowth_multiplier(plot.overgrowth)
            * (1.0 + snapshot.rank.rank_xp / 100.0)
            * snapshot.plot_rank_display.max(1.0)
    }
}

/// One plot's rendered metrics.
#[derive(Clone, Debug)]
pub struct PlotMetrics {
    pub plot: usize,
    pub seed_family: i64,
    pub crop_id: i64,
    pub growth_progress: f64,
    pub evolution: u32,
    pub evolution_blocked: bool,
    pub crops_on_vine: u32,
    pub overgrowth: u32,
    pub overflow_progress: f64,
    pub growth_requirement: f64,
    pub growth_seconds: f64,
    pub next_crop_chance: f64,
    pub next_overgrowth_chance: f64,
    pub crop_value: CropValue,
    pub farming_exp: f64,
    pub rank: u32,
    pub rank_progress: f64,
    pub rank_requirement: f64,
    pub rank_xp: f64,
}

/// `cropValueDetails` result.
#[derive(Clone, Debug)]
pub struct CropValue {
    pub base: f64,
    pub raw: f64,
    pub cap: f64,
    pub value: f64,
    pub capped: bool,
    pub expected: f64,
    pub product_low: f64,
    pub product_high: f64,
    pub product_high_chance: f64,
    pub expected_vines: f64,
    pub expected_harvest: f64,
}

/// Each overgrowth level doubles value and EXP, capped at 1B.
pub fn overgrowth_multiplier(overgrowth: u32) -> f64 {
    (2_f64.powi(overgrowth as i32)).clamp(1.0, 1e9)
}

/// `landRankRequirement(rank)`: the rank XP the next plot rank needs.
pub fn land_rank_requirement(rank: u32) -> f64 {
    let rank = f64::from(rank);
    (7.0 * rank + 25.0 * (rank / 5.0).floor() + 10.0) * 1.11_f64.powf(rank)
}
