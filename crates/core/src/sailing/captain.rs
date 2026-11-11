//! Captain quality and leveling, from the upstream `sailing-chests.js`.
//!
//! Each bonus a captain rolls is fixed when they are recruited: a level only
//! multiplies the roll. [`roll_bounds`] is the recruitment range the roll sits
//! in, so [`roll_quality`] reads how close to the best possible roll it is.

use super::{
    Boat, Captain, Fleet, RouteContext, davey_jones_bonus, island_captain_exp, route_timing,
};

/// The weakest and strongest value a bonus type can roll at a rarity.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RollBounds {
    pub min: f64,
    pub max: f64,
}

/// One captain bonus slot: its type, its roll, the range it came from and
/// where in that range it landed.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct RollQuality {
    /// The bonus type, `-1` for an empty slot.
    pub bonus_type: i32,
    pub roll: f64,
    pub min: f64,
    pub max: f64,
    /// `0` at the weakest roll, `1` at the strongest.
    pub quality: f64,
}

/// The account-wide captain-EXP factors and their product.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptainExpMultiplier {
    /// Davey Jones Returns plus Davey Jones Training, from the save.
    pub davey: f64,
    /// Talents, achievements, shiny pets and the vault, in percent.
    pub additive: f64,
    /// The product of the vote, companion, sushi and Nightmariner factors.
    pub multiplier: f64,
    /// The EXP multiplier a trip applies.
    pub value: f64,
}

/// One captain's progress toward their next level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CaptainLevelTime {
    pub level: u32,
    /// Experience already banked toward the level.
    pub progress: f64,
    /// Experience the level needs.
    pub requirement: f64,
    /// Experience still missing.
    pub remaining: f64,
    pub exp_per_trip: f64,
    pub exp_per_hour: f64,
    /// Hours to the next level; infinite without trip income.
    pub hours: f64,
}

/// The weakest roll a bonus type can have at `rarity`, upstream
/// `captainRollBounds`.
pub fn roll_bounds(rarity: u32, bonus_type: i32) -> RollBounds {
    if !(0..=4).contains(&bonus_type) {
        return RollBounds::default();
    }
    const MIN_BASE: [f64; 5] = [10.0, 30.0, 30.0, 10.0, 10.0];
    const MAX_BASE: [f64; 5] = [60.0, 100.0, 100.0, 50.0, 40.0];
    let rarity = rarity as f64;
    let max_base = MAX_BASE[bonus_type as usize];
    let max = (0.15 * ((0.5 + 0.8 * rarity) * max_base).floor()).floor();
    // Only a legendary captain lifts its floor; the others start at the type's
    // base roll.
    let min = if rarity >= 6.0 {
        (0.15 * ((0.5 + 0.8 * rarity) * max_base * 0.65).floor()).floor()
    } else {
        (0.15 * MIN_BASE[bonus_type as usize]).floor()
    };
    RollBounds { min, max }
}

/// One captain's rolled bonus in `slot`, upstream `captainRollQuality`.
pub fn roll_quality(captain: &Captain, slot: usize) -> RollQuality {
    let bonus_type = captain.bonus_types.get(slot).copied().unwrap_or(-1);
    if bonus_type < 0 {
        return RollQuality {
            bonus_type: -1,
            ..RollQuality::default()
        };
    }
    let bounds = roll_bounds(captain.rarity, bonus_type);
    let roll = captain.rolls.get(slot).copied().unwrap_or(0.0);
    let span = (bounds.max - bounds.min).max(1.0);
    RollQuality {
        bonus_type,
        roll,
        min: bounds.min,
        max: bounds.max,
        quality: ((roll - bounds.min) / span).clamp(0.0, 1.0),
    }
}

/// The experience a captain level needs, upstream `captainExpRequirement`.
pub fn exp_requirement(level: u32) -> f64 {
    let level = level as f64;
    (9.0 + level.powi(3)) * 1.5_f64.powf(level) * 1.5_f64.powf((level - 10.0).max(0.0))
}

/// The account-wide captain-EXP multiplier, upstream
/// `captainExpMultiplierDetail` with its unmodelled factors left neutral.
///
/// Upstream reads the Davey Jones gate from boat 0 whatever boat the captain
/// sails; this keeps that behaviour rather than guessing a better one.
pub fn exp_multiplier(fleet: &Fleet, context: &RouteContext) -> CaptainExpMultiplier {
    let davey = davey_jones_bonus(fleet.boats.first().unwrap_or(&Boat::default()), context);
    let (additive, multiplier) = (context.captain_exp.additive, context.captain_exp.multiplier);
    CaptainExpMultiplier {
        davey,
        additive,
        multiplier,
        value: davey * (1.0 + additive / 100.0) * multiplier,
    }
}

/// The captain EXP one trip to `island` brings, upstream `captainExpPerTrip`.
pub fn exp_per_trip(island: usize, fleet: &Fleet, context: &RouteContext) -> f64 {
    island_captain_exp(island) * exp_multiplier(fleet, context).value
}

/// One captain's level time on `boat` and `island`, upstream
/// `captainLevelTime`.
pub fn level_time(
    fleet: &Fleet,
    captain: usize,
    boat: usize,
    island: usize,
    context: &RouteContext,
) -> CaptainLevelTime {
    let entry = fleet.captains.get(captain).cloned().unwrap_or_default();
    let requirement = exp_requirement(entry.level);
    let trips = route_timing(fleet, boat, island, Some(captain), context).trips_per_hour;
    let per_trip = exp_per_trip(island, fleet, context);
    let per_hour = trips * per_trip;
    let remaining = (requirement - entry.exp).max(0.0);
    CaptainLevelTime {
        level: entry.level,
        progress: entry.exp,
        requirement,
        remaining,
        exp_per_trip: per_trip,
        exp_per_hour: per_hour,
        hours: if per_hour > 0.0 {
            remaining / per_hour
        } else {
            f64::INFINITY
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn captain(rarity: u32, bonus_types: [i32; 2], rolls: [f64; 2]) -> Captain {
        Captain {
            rarity,
            bonus_types,
            level: 10,
            exp: 0.0,
            rolls,
        }
    }

    #[test]
    fn the_roll_range_matches_upstream_bounds() {
        // A common loot captain rolls 4 to 7; a legendary one 51 to 79.
        assert_eq!(roll_bounds(0, 1), RollBounds { min: 4.0, max: 7.0 });
        assert_eq!(
            roll_bounds(6, 1),
            RollBounds {
                min: 51.0,
                max: 79.0
            }
        );
        assert_eq!(roll_bounds(0, 0), RollBounds { min: 1.0, max: 4.0 });
        assert_eq!(roll_bounds(0, -1), RollBounds::default());
        assert_eq!(roll_bounds(0, 5), RollBounds::default());
    }

    #[test]
    fn quality_reads_the_roll_inside_its_range() {
        let worst = roll_quality(&captain(0, [1, -1], [4.0, 0.0]), 0);
        assert_eq!(worst.bonus_type, 1);
        assert_eq!(worst.quality, 0.0);
        let best = roll_quality(&captain(0, [1, -1], [7.0, 0.0]), 0);
        assert_eq!(best.quality, 1.0);
        let middle = roll_quality(&captain(0, [1, -1], [5.5, 0.0]), 0);
        assert!((middle.quality - 0.5).abs() < 1e-12);
        assert_eq!(middle.roll, 5.5);
        assert_eq!(middle.min, 4.0);
        assert_eq!(middle.max, 7.0);
        // A roll outside the range is clamped to the ends.
        assert_eq!(
            roll_quality(&captain(0, [1, -1], [99.0, 0.0]), 0).quality,
            1.0
        );
    }

    #[test]
    fn an_empty_slot_has_no_bonus() {
        let empty = roll_quality(&captain(0, [1, -1], [6.0, 0.0]), 1);
        assert_eq!(empty.bonus_type, -1);
        assert_eq!(empty.quality, 0.0);
    }

    #[test]
    fn the_exp_requirement_grows_with_level() {
        assert_eq!(exp_requirement(0), 9.0);
        assert_eq!(exp_requirement(1), (9.0 + 1.0) * 1.5);
        let twenty = exp_requirement(20);
        assert!(twenty > exp_requirement(19));
        close(exp_requirement(10), (9.0 + 1000.0) * 1.5_f64.powf(10.0));
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs().max(1.0) * 1e-12,
            "expected {expected}, got {actual}"
        );
    }
}
