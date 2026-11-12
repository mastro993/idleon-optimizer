//! The overkill tier, ported from `js/stats/systems/common/overkill.js`.
//!
//! The game's `OverkillStuffs("2")` gives the tier the coin multiplier reads:
//!
//! - the exponent is 5 on World 7 maps (index ≥ 300) and 2 elsewhere;
//! - tier 1 is the baseline, and every threshold the damage clears raises it by
//!   one, up to 51;
//! - a target with no monster HP leaves the tier at 1 and marks the result
//!   partial, exactly like upstream's `partial: true` branch.

use crate::stats;

/// The overkill tier the coin multiplier's damage-scaled sources read.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Overkill {
    /// `OverkillStuffs("2")`, clamped to 1..=51 like the page does.
    pub tier: u32,
    /// The monster's HP the thresholds compare against.
    pub monster_hp: f64,
    /// The maximum damage the thresholds are compared with.
    pub damage: f64,
    /// The map exponent: 5 on World 7 maps, 2 elsewhere.
    pub exponent: f64,
    /// Whether the saved target or its monster row is unavailable.
    pub partial: bool,
}

/// The tier one map's monster HP and a damage value produce.
///
/// Ported from `computeOverkillTier`: a target without HP returns tier 1 and
/// `partial`; otherwise the tier is the number of thresholds the damage clears.
pub fn tier(map: usize, monster_hp: f64, damage: f64) -> Overkill {
    let exponent = if map >= 300 { 5.0 } else { 2.0 };
    let monster_hp = monster_hp.max(0.0);
    let damage = damage.max(0.0);
    if monster_hp <= 0.0 {
        return Overkill {
            tier: 1,
            monster_hp: 0.0,
            damage: 0.0,
            exponent,
            partial: true,
        };
    }

    let mut tier = 1;
    for step in 0..50 {
        let threshold = monster_hp * exponent * exponent.powi(step + 1);
        if damage >= threshold {
            tier = step as u32 + 2;
        } else {
            break;
        }
    }

    Overkill {
        tier: tier.clamp(1, 51),
        monster_hp,
        damage,
        exponent,
        partial: false,
    }
}

/// The AFK target's monster HP, `0` when the target is unavailable.
pub fn target_hp(key: &str) -> f64 {
    stats::monster(key).map(|monster| monster.hp).unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_target_without_hp_stays_at_tier_one_and_is_partial() {
        let overkill = tier(1, 0.0, 1e9);
        assert_eq!(overkill.tier, 1);
        assert!(overkill.partial);
        assert_eq!(overkill.monster_hp, 0.0);
    }

    #[test]
    fn the_thresholds_are_the_upstream_geometric_series() {
        // World 2 map, exponent 2: HP 100 → 400, 800, 1600, ...
        let green = tier(1, 100.0, 400.0);
        assert_eq!(green.tier, 2);
        assert_eq!(green.exponent, 2.0);
        assert!(!green.partial);
        assert_eq!(tier(1, 100.0, 399.0).tier, 1);
        assert_eq!(tier(1, 100.0, 800.0).tier, 3);
    }

    #[test]
    fn world_seven_maps_use_the_five_times_exponent() {
        let w7 = tier(300, 100.0, 100.0 * 5.0 * 5.0);
        assert_eq!(w7.exponent, 5.0);
        assert_eq!(w7.tier, 2);
        assert_eq!(tier(299, 100.0, 100.0 * 5.0 * 5.0).tier, 4);
    }

    #[test]
    fn the_tier_is_capped_at_fifty_one() {
        assert_eq!(tier(1, 1.0, f64::MAX).tier, 51);
    }

    #[test]
    fn a_saved_target_resolves_its_monster_row() {
        assert_eq!(target_hp("mushG"), 32.0);
        assert_eq!(target_hp("a target the table does not carry"), 0.0);
    }
}
