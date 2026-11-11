//! Break chances: opals, collectibles and enchants, their tier scaling and the
//! time each event takes.
//!
//! Ported from `opalChance`, `collectibleChance`, `enchantChance`,
//! `enchantChanceAt`, `enchantMulti`, `expectedBreaksToEvent` and
//! `expectedHoursToEvent` in `js/stats/systems/w5/jar.js`.

use super::{Jar, upgrade};

/// One chance jar's event, as the chances tab lists it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChanceEvent {
    /// The event's name, e.g. `Opal`.
    pub name: &'static str,
    /// The jar type whose breaks roll it.
    pub jar_type: usize,
    /// The chance at tier one, as a fraction of one.
    pub chance: f64,
}

impl Jar {
    /// The three chance jars, in save order, at the current balances.
    pub fn chance_events(&self) -> [ChanceEvent; 3] {
        [
            ChanceEvent {
                name: "Opal",
                jar_type: 1,
                chance: self.opal_chance(),
            },
            ChanceEvent {
                name: "Collectible",
                jar_type: 2,
                chance: self.collectible_chance(),
            },
            ChanceEvent {
                name: "Enchant",
                jar_type: 4,
                chance: self.enchant_chance(),
            },
        ]
    }

    /// `opalChance`: 25% × 0.43 per opal already found, raised by the three
    /// opal collectibles.
    pub fn opal_chance(&self) -> f64 {
        let legend = self.ext.legend_pts;
        0.25 * 0.43_f64.powf(self.opals())
            * (1.0 + self.collectible_bonus(2, legend) / 100.0)
            * (1.0 + self.collectible_bonus(14, legend) / 100.0)
            * (1.0 + self.collectible_bonus(27, legend) / 100.0)
    }

    /// `collectibleChance`: each collectible found makes the next one harder,
    /// and page two adds 1.5× per find past the sixteenth.
    pub fn collectible_chance(&self) -> f64 {
        let found = self.collectibles_found();
        let legend = self.ext.legend_pts;
        let digits = if self.upgrade(upgrade::COLLECT_ALL) {
            (0..10)
                .map(|rupie| super::log10(self.rupies(rupie)).ceil())
                .sum::<f64>()
        } else {
            0.0
        };
        let multi = (1.0 + self.collectible_bonus(7, legend) / 100.0)
            * 1.02_f64.powf(digits).max(1.0)
            * (1.0 + self.collectible_bonus(25, legend) / 100.0)
            * (1.0 + self.ext.palette / 100.0)
            * (1.0 + self.ext.exotic / 100.0)
            * (1.05_f64.powf(self.ext.lore_episodes) * self.ext.super_bit).max(1.0);
        if found == 0 {
            return 0.25;
        }
        let scale = 0.2 * multi / (1.0 + (found as f64).powf(1.9));
        if found < super::PAGE_TWO_COLLECTIBLES {
            scale
        } else {
            scale / 1.5_f64.powf((found - super::PAGE_TWO_COLLECTIBLES) as f64)
        }
    }

    /// `enchantChance`: the base curve falls with every enchant level, and the
    /// multipliers raise it back.
    pub fn enchant_chance(&self) -> f64 {
        Self::enchant_chance_at(self.enchant_levels(), self.enchant_multi())
    }

    /// `enchantChanceAt`: the base curve alone, for the simulator to reuse.
    pub fn enchant_chance_at(levels: u32, multi: f64) -> f64 {
        let levels = f64::from(levels);
        0.35 / (1.0 + levels.powf(1.23) + 1.1_f64.powf(levels)) * multi
    }

    /// `enchantMulti`: everything the enchant chance multiplies the curve by.
    pub fn enchant_multi(&self) -> f64 {
        let legend = self.ext.legend_pts;
        let dark_luck = (self.upgrade_value(upgrade::DARK_LUCK, 1.0)
            * 1.1_f64.powf(super::log10(self.dark_rupies())))
        .max(1.0);
        (1.0 + self.collectible_bonus(9, legend) / 100.0)
            * dark_luck
            * (1.0 + self.collectible_bonus(18, legend) / 100.0)
            * (1.0 + self.ext.bolaia / 100.0)
            * (1.0 + self.collectible_bonus(26, legend) / 100.0)
            * (1.0 + self.collectible_bonus(34, legend) / 100.0)
            * (1.0 + self.ext.fountain / 100.0)
    }

    /// The chance at `tier`, counting the tiers below it: each tier multiplies
    /// by ten.
    pub fn chance_at_tier(chance: f64, tier: u32) -> f64 {
        chance * 10_f64.powi(tier as i32)
    }

    /// The tier at which the chance is certain, or `None` when it never can be.
    pub fn tier_for_certainty(chance: f64) -> Option<u32> {
        (chance > 0.0).then(|| (1.0 / chance).log10().ceil() as u32 + 1)
    }

    /// `expectedBreaksToEvent`: how many breaks the event takes on average.
    pub fn expected_breaks(chance: f64) -> f64 {
        if chance <= 0.0 {
            f64::INFINITY
        } else if chance >= 1.0 {
            1.0
        } else {
            1.0 / chance
        }
    }

    /// `expectedHoursToEvent`: breaks at the jar type's rate, one slot at a time.
    pub fn expected_hours(&self, jar_type: usize, chance: f64) -> f64 {
        let breaks = Self::expected_breaks(chance);
        if !breaks.is_finite() {
            return f64::INFINITY;
        }
        let jars_per_hour = self.production_per_hour() / self.production_requirement(jar_type);
        breaks / jars_per_hour
    }
}
