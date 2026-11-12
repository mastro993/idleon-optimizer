//! The next-tier tab: the next power-of-ten milestone each jar type should
//! chase, and the off-map accumulation that decides the highest tier it can
//! reach.
//!
//! Ported from `_renderNextTier` in `jar-calc.html`: the `Jar Production Line`
//! ranking and the `_simMaxTier` / `_simTierStats` off-map loop. The game ticks
//! every five seconds and produces one jar per slot per tick; surplus progress
//! accumulates until it clears the next tier's requirement.

use super::{Jar, TYPE_NAMES, upgrade};

/// Seconds between off-map production ticks.
pub const TICK_SECONDS: f64 = 5.0;

/// Ticks the upstream ranking simulates, so the maximum tier is comparable.
const SIM_TICKS: u32 = 500;

/// Ticks the upstream stats loop watches to time the highest-tier jar.
const STAT_TICKS: u32 = 1_000;

/// The slots the upstream ranking assumes for a milestone: all three.
const ASSUMED_SLOTS: f64 = 3.0;

/// One row of the `Next Pow10 Milestone Ranking`.
#[derive(Clone, Debug, PartialEq)]
pub struct MilestoneRow {
    pub jar_type: usize,
    pub name: &'static str,
    /// `jarsProducedOfType`.
    pub count: f64,
    /// The next power of ten above the count.
    pub next: f64,
    pub deficit: f64,
    /// Hours the deficit takes when all three slots work on it.
    pub hours: f64,
    /// The jar type this milestone speeds up.
    pub benefits: Option<&'static str>,
    /// `Jar Production Line` bonus now.
    pub line_now: f64,
    /// The bonus after reaching the next milestone.
    pub line_next: f64,
    pub requirement: f64,
}

/// One row of the off-map tier projection.
#[derive(Clone, Debug, PartialEq)]
pub struct TierProjection {
    pub jar_type: usize,
    pub name: &'static str,
    pub requirement: f64,
    pub base_requirement: f64,
    /// The tier the per-tick progress sustains, or `None` when it cannot even
    /// fill tier one every tick.
    pub sustained: Option<u32>,
    /// `_simMaxTier`: the highest tier the 500-tick simulation reaches.
    pub max_tier: u32,
    /// The ticks between the highest-tier jars, when the loop produced one.
    pub ticks_between_max: Option<f64>,
    /// How long the highest-tier jar takes to appear, in seconds.
    pub seconds_between: Option<f64>,
    /// Surplus progress per tick at the sustained tier.
    pub net_sustained: f64,
    /// The first tier the per-tick progress cannot sustain.
    pub next_tier: u32,
    /// How short the per-tick progress is for that next tier.
    pub net_next: f64,
    /// The previous type's produced count the production line would need to
    /// sustain the next tier.
    pub needed_line: f64,
    /// The production per hour needed to sustain the next tier at the current
    /// requirement.
    pub needed_production: f64,
    /// The previous type's produced count.
    pub previous_count: f64,
}

impl Jar {
    /// `productionLineUnlocked`: whether Big Jar Mach's production line is owned.
    pub fn production_line_unlocked(&self) -> bool {
        self.upgrade(upgrade::PRODUCTION_LINE)
    }

    /// The progress one off-map tick adds.
    pub fn per_tick(&self) -> f64 {
        self.production_per_hour() * TICK_SECONDS / 3_600.0
    }

    /// `_renderNextTier`: every available type ranked by how fast it reaches the
    /// next power of ten.
    pub fn milestone_ranking(&self) -> Vec<MilestoneRow> {
        let per_hour = self.production_per_hour();
        let available = self.types_available();
        let mut rows: Vec<MilestoneRow> = (0..available)
            .map(|jar_type| {
                let count = self.jars_produced(jar_type);
                let log_count = if count > 0.0 {
                    count.log10().floor()
                } else {
                    -1.0
                };
                let next = 10_f64.powf(log_count + 1.0);
                let deficit = next - count;
                let requirement = self.production_requirement(jar_type);
                let jars_per_hour = per_hour / requirement;
                let hours = deficit / (jars_per_hour * ASSUMED_SLOTS);
                MilestoneRow {
                    jar_type,
                    name: TYPE_NAMES[jar_type],
                    count,
                    next,
                    deficit,
                    hours,
                    benefits: (jar_type + 1 < available).then(|| TYPE_NAMES[jar_type + 1]),
                    line_now: 5.0 * count.max(1.0).log10(),
                    line_next: 5.0 * (log_count + 2.0),
                    requirement,
                }
            })
            .collect();
        rows.sort_by(|left, right| left.hours.total_cmp(&right.hours));
        rows
    }

    /// Every available type's off-map projection.
    pub fn tier_projections(&self) -> Vec<TierProjection> {
        (0..self.types_available())
            .map(|jar_type| self.tier_projection(jar_type))
            .collect()
    }

    /// One type's off-map projection, exactly as `_renderNextTier` builds it.
    pub fn tier_projection(&self, jar_type: usize) -> TierProjection {
        let requirement = self.production_requirement(jar_type);
        let base_requirement = 1_000.0 + 2_000.0 * jar_type as f64;
        let per_tick = self.per_tick();
        let previous_count = self.jars_produced(jar_type.saturating_sub(1));

        let sustained =
            (per_tick > requirement).then(|| (per_tick / requirement).log10().floor() as u32);
        let max_tier = Self::sim_max_tier(per_tick, requirement);
        let ticks_between_max = Self::ticks_between_max(per_tick, requirement, max_tier, sustained);
        let net_sustained = match sustained {
            Some(tier) => per_tick - requirement * 10_f64.powi(tier as i32),
            None => per_tick - requirement,
        };
        let next_tier = sustained.map_or(0, |tier| tier + 1);
        let net_next = per_tick - requirement * 10_f64.powi(next_tier as i32);

        // `_renderNextTier` asks what previous-type count the production line
        // would need; a non-positive percentage means the line already has it.
        let mach_seven = 1.0 + self.upgrade_value(upgrade::REQUIREMENT, 30.0) / 100.0;
        let sustain_needed = per_tick / 10_f64.powf(next_tier as f64);
        let line_multiplier = base_requirement / (sustain_needed * mach_seven);
        let line_percent = (line_multiplier - 1.0) * 100.0;
        let needed_line = if line_percent > 0.0 {
            10_f64.powf(line_percent / 5.0)
        } else {
            1.0
        };

        let seconds_between = match ticks_between_max {
            Some(ticks) if per_tick > requirement => Some(ticks * TICK_SECONDS),
            _ => Some((requirement / per_tick).ceil() * TICK_SECONDS),
        };

        TierProjection {
            jar_type,
            name: TYPE_NAMES[jar_type],
            requirement,
            base_requirement,
            sustained,
            max_tier,
            ticks_between_max,
            seconds_between,
            net_sustained,
            next_tier,
            net_next,
            needed_line,
            needed_production: requirement * 10_f64.powf(next_tier as f64) * 3_600.0 / TICK_SECONDS,
            previous_count,
        }
    }

    /// `_simMaxTier`: the highest tier [`SIM_TICKS`] off-map ticks consume.
    fn sim_max_tier(per_tick: f64, requirement: f64) -> u32 {
        if per_tick <= requirement {
            return 0;
        }
        let mut progress = 0.0;
        let mut best = 0;
        for _ in 0..SIM_TICKS {
            if let Some(tier) = Self::tick(&mut progress, per_tick, requirement) {
                best = best.max(tier);
            }
        }
        best
    }

    /// One off-map tick: add `per_tick`, then consume the highest tier the
    /// progress clears and return it, keeping the remainder.
    fn tick(progress: &mut f64, per_tick: f64, requirement: f64) -> Option<u32> {
        *progress += per_tick;
        (*progress >= requirement).then(|| {
            let tier = (*progress / requirement).log10().floor() as u32;
            *progress -= requirement * 10_f64.powi(tier as i32);
            if *progress < 0.0 {
                *progress = 0.0;
            }
            tier
        })
    }

    /// `_simTierStats.ticksBetweenMax`: how many ticks separate highest-tier
    /// jars, when the loop produces more than one.
    fn ticks_between_max(
        per_tick: f64,
        requirement: f64,
        max_tier: u32,
        sustained: Option<u32>,
    ) -> Option<f64> {
        if per_tick <= requirement || max_tier == 0 {
            return None;
        }
        let stable = sustained.unwrap_or(0);
        if max_tier <= stable {
            return None;
        }
        let mut progress = 0.0;
        let mut first = None;
        let mut hits = 0;
        for tick in 0..STAT_TICKS {
            if Self::tick(&mut progress, per_tick, requirement).is_some_and(|tier| tier >= max_tier)
            {
                hits += 1;
                first.get_or_insert(f64::from(tick + 1));
            }
        }
        match hits {
            0 => None,
            1 => first,
            _ => Some((f64::from(STAT_TICKS) / hits as f64).ceil()),
        }
    }
}
