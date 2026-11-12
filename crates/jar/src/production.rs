//! Production: the shared rate every slot fills at, its multipliers and the
//! requirement each jar type takes.
//!
//! Ported from `productionSlots`, `productionPerHR`, `productionPerHRBreakdown`,
//! `productionREQ` and `timeToTier` in `js/stats/systems/w5/jar.js`.

use super::{
    BASE_PRODUCTION_PER_HOUR, Jar, ProductionTerm, TYPE_NAMES, log, measurement, row, state,
    upgrade,
};

impl Jar {
    /// `productionSlots`: one slot, two with Big Jar Mach III, three with VI.
    pub fn production_slots(&self) -> usize {
        if self.upgrade(upgrade::SLOTS_THREE) {
            3
        } else if self.upgrade(upgrade::SLOTS_TWO) {
            2
        } else {
            1
        }
    }

    /// `productionSlotType`: the jar type a slot is filling.
    pub fn slot_type(&self, slot: usize) -> usize {
        (self.cell(row::SLOTS, slot).round().max(0.0) as usize).min(TYPE_NAMES.len() - 1)
    }

    /// `productionSlotProgress`: the progress a slot has banked toward its next jar.
    pub fn slot_progress(&self, slot: usize) -> f64 {
        self.cell(row::SLOTS, slot + 3)
    }

    /// `jarsProducedOfType`: how many jars of a type the account has broken.
    pub fn jars_produced(&self, jar_type: usize) -> f64 {
        self.cell(row::STATE, state::JARS_PRODUCED + jar_type)
    }

    /// `productionPerHR`: progress every slot shares, before any bonus.
    pub fn production_per_hour(&self) -> f64 {
        let legend = self.ext.legend_pts;
        let collectibles = self.collectible_bonus(1, legend)
            + self.collectible_bonus(15, legend)
            + self.collectible_bonus(24, legend);
        let bonus = collectibles + self.measurement_bonus(measurement::PRODUCTION);
        let light_speed =
            self.upgrade_value(upgrade::LIGHT_SPEED, 10.0) * log(self.white_rupies()) / 100.0;
        BASE_PRODUCTION_PER_HOUR * (1.0 + bonus / 100.0) * (1.0 + light_speed)
    }

    /// `productionPerHRBreakdown`: the bonuses [`Self::production_per_hour`]
    /// applies, in the upstream order, without the ones that grant nothing.
    pub fn production_terms(&self) -> Vec<ProductionTerm> {
        let legend = self.ext.legend_pts;
        let mut terms = Vec::new();
        for (label, index) in [
            ("Sapphire Droplet", 1),
            ("Frost Spirestone", 15),
            ("Earthbound Geode", 24),
        ] {
            let bonus = self.collectible_bonus(index, legend);
            if bonus > 0.0 {
                terms.push(ProductionTerm {
                    label,
                    percent: bonus,
                });
            }
        }
        let measurement = self.measurement_bonus(measurement::PRODUCTION);
        if measurement > 0.0 {
            terms.push(ProductionTerm {
                label: "Jar Production Speed Measurement",
                percent: measurement,
            });
        }
        if self.upgrade(upgrade::LIGHT_SPEED) {
            terms.push(ProductionTerm {
                label: "Light Speed",
                percent: self.upgrade_value(upgrade::LIGHT_SPEED, 10.0) * log(self.white_rupies()),
            });
        }
        terms
    }

    /// `productionREQ`: the progress a jar of `jar_type` needs, after Big Jar
    /// Mach VII and the production line.
    pub fn production_requirement(&self, jar_type: usize) -> f64 {
        let (mach_seven, line) = self.requirement_reduction(jar_type);
        (1000.0 + 2000.0 * jar_type as f64) / ((1.0 + mach_seven / 100.0) * (1.0 + line / 100.0))
    }

    /// What Big Jar Mach VII and the production line take off a requirement, as
    /// the requirement table lists them.
    pub fn requirement_reduction(&self, jar_type: usize) -> (f64, f64) {
        let mach_seven = self.upgrade_value(upgrade::REQUIREMENT, 30.0);
        let previous = self.jars_produced(jar_type.saturating_sub(1));
        let line = self.upgrade_value(upgrade::PRODUCTION_LINE, 5.0) * log(previous);
        (mach_seven, line)
    }

    /// `timeToTier`: hours one slot needs to reach `tier` of `jar_type`, counting
    /// the tiers below it.
    pub fn time_to_tier(&self, jar_type: usize, tier: u32) -> f64 {
        let needed = self.production_requirement(jar_type) * 10_f64.powi(tier as i32);
        needed / self.production_per_hour()
    }
}
