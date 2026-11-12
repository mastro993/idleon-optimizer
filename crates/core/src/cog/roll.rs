//! The game's cog roll maths: the raw magnitude a roll starts from, the
//! per-roll stat conversions and the roll ranges the section prints.
//!
//! Ported from `_rawMag`, `_maxRolls`, `_minRolls`, `_statFromVal` and
//! `_inferConLvFromD` in `js/cogs/cog-judge.js`, and from `_rollRangeStats` in
//! `cog-optimizer.html`. The range card shows the two rolls the optimizer's
//! targets name, Ulti (tier 3) and Jewelled (tier 4).

use super::Goal;

/// The tiers the roll-range card shows: Ulti and Jewelled.
pub const SHOWN_TIERS: [usize; 2] = [3, 4];

/// `_rawMag`: the value a single roll scales, at a stat tier and construction
/// level.
pub fn raw_magnitude(tier: usize, construction_level: f64) -> f64 {
    let tier = tier as f64;
    3_f64.powf(tier.min(3.4)) + 0.25 * (construction_level / 3.0 + 0.7).powf(1.4 + 0.05 * tier)
}

/// `_maxRolls`: tiers 0-1 roll three stats, tiers 2 and up four.
pub fn max_rolls(tier: usize) -> usize {
    if tier < 2 { 3 } else { 4 }
}

/// `_minRolls`: tiers 0-1 roll at least two stats, tiers 2 and up three.
pub fn min_rolls(tier: usize) -> usize {
    if tier < 2 { 2 } else { 3 }
}

/// `_statFromVal`: one roll's raw value as the goal's stat.
///
/// Build rounds the value, Flaggy rounds its 0.8 power and construction EXP
/// compresses it to the game's small integer with a floor of 2.
pub fn stat_from_value(value: f64, goal: Goal) -> f64 {
    match goal {
        Goal::Build => value.round(),
        Goal::Flaggy => value.powf(0.8).round(),
        Goal::ConExp => con_exp(value),
    }
}

/// The upstream page's truncated ln(10), `2.30259`; the exact constant shifts
/// the last digit of a long construction EXP roll, so the port keeps theirs.
#[allow(clippy::approx_constant)]
const UPSTREAM_LN_10: f64 = 2.30259;

/// The construction EXP a roll value becomes, with the game's floor of 2.
fn con_exp(value: f64) -> f64 {
    if value <= 0.0 {
        return 2.0;
    }
    let value = value.max(1.0);
    (value.powf(0.4) + 10.0 * value.ln() / UPSTREAM_LN_10 - 5.0)
        .floor()
        .max(2.0)
}

/// `_inferConLvFromD`: the construction level whose perfect roll would just
/// reach a saved `d`. A board's own rolls reveal the effective level the save
/// does not carry (talents, stamps and star signs move it).
pub fn infer_construction_level(d: f64, tier: usize) -> u32 {
    if d <= 0.0 {
        return 0;
    }
    let per_roll = d / max_rolls(tier) as f64;
    let perfect = |level: f64| con_exp(3.0 * raw_magnitude(tier, level));
    let (mut low, mut high) = (1.0_f64, 100_000.0_f64);
    for _ in 0..50 {
        let mid = (low + high) / 2.0;
        if perfect(mid) < per_roll {
            low = mid;
        } else {
            high = mid;
        }
    }
    high.ceil() as u32
}

/// One tier's roll range at a construction level, for one goal.
#[derive(Clone, Debug, PartialEq)]
pub struct RollRange {
    /// The stat tier the range is for.
    pub tier: usize,
    /// The most rolls a cog of this tier can carry.
    pub rolls: usize,
    pub raw_magnitude: f64,
    /// The lowest and highest one roll can produce for the goal's stat.
    pub per_roll_min: f64,
    pub per_roll_max: f64,
    /// The stat with every roll at the top of its range.
    pub total_max: f64,
    /// The surround range the tier rolls; it does not follow the construction
    /// level.
    pub min_surround: f64,
    pub max_surround: f64,
    /// The next construction level at which the minimum per-roll stat rises.
    pub next_min_level: u32,
    pub next_min_value: f64,
    /// The next construction level at which the maximum per-roll stat rises.
    pub next_max_level: u32,
    pub next_max_value: f64,
}

/// `_rollRangeStats`: a tier's range at `construction_level`, with the next
/// level each per-roll bound rises at.
pub fn roll_range(goal: Goal, tier: usize, construction_level: u32) -> RollRange {
    let level = f64::from(construction_level);
    let magnitude = raw_magnitude(tier, level);
    let (per_roll_min, per_roll_max) = bounds(goal, tier, level);
    let (min_surround, max_surround) = surround_range(tier);
    let (next_min_level, next_min_value) =
        next_level(goal, tier, construction_level, per_roll_min, false);
    let (next_max_level, next_max_value) =
        next_level(goal, tier, construction_level, per_roll_max, true);
    RollRange {
        tier,
        rolls: max_rolls(tier),
        raw_magnitude: magnitude,
        per_roll_min,
        per_roll_max,
        total_max: per_roll_max * max_rolls(tier) as f64,
        min_surround,
        max_surround,
        next_min_level,
        next_min_value,
        next_max_level,
        next_max_value,
    }
}

/// The range card's rows: one per [`SHOWN_TIERS`] entry.
pub fn roll_ranges(goal: Goal, construction_level: u32) -> Vec<RollRange> {
    SHOWN_TIERS
        .into_iter()
        .map(|tier| roll_range(goal, tier, construction_level))
        .collect()
}

/// One roll's lowest and highest stat, from the game's 0.4x-3.0x multiplier.
fn bounds(goal: Goal, tier: usize, construction_level: f64) -> (f64, f64) {
    let magnitude = raw_magnitude(tier, construction_level);
    let lowest = (0.4 * magnitude).floor();
    let highest = (3.0 * magnitude).floor().max(0.0);
    (
        stat_from_value(lowest, goal),
        stat_from_value(highest, goal),
    )
}

/// The surround range a stat tier rolls, from `_rollRangeStats`.
fn surround_range(tier: usize) -> (f64, f64) {
    match tier {
        0 => (5.0, 10.0),
        1 => (8.0, 15.0),
        2 => (12.0, 40.0),
        3 => (20.0, 65.0),
        // Crystal: the CogCry number is the upgrade level, 1 through 5.
        _ => (30.0 + 23.0, 40.0 + 23.0 * 5.0),
    }
}

/// `_findConForPlus1`: the next construction level within 500 at which the
/// goal's per-roll bound rises, or the cap with a bound one higher.
fn next_level(
    goal: Goal,
    tier: usize,
    construction_level: u32,
    current: f64,
    highest: bool,
) -> (u32, f64) {
    for level in (construction_level + 1)..=(construction_level + 500) {
        let (lowest, top) = bounds(goal, tier, f64::from(level));
        let value = if highest { top } else { lowest };
        if value > current {
            return (level, value);
        }
    }
    (construction_level + 500, current + 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_raw_magnitude_follows_the_tier_and_level() {
        // 3^3 + 0.25 × (0/3 + 0.7)^1.55
        assert!((raw_magnitude(3, 0.0) - (27.0 + 0.25 * 0.7_f64.powf(1.55))).abs() < 1e-12);
        // A crystal cog caps the tier exponent at 3.4.
        assert!(
            (raw_magnitude(4, 0.0) - (3_f64.powf(3.4) + 0.25 * 0.7_f64.powf(1.6))).abs() < 1e-12
        );
        assert!(raw_magnitude(4, 100.0) > raw_magnitude(3, 100.0));
        assert!(raw_magnitude(3, 200.0) > raw_magnitude(3, 100.0));
    }

    #[test]
    fn rolls_and_stat_conversions_match_the_game() {
        assert_eq!(max_rolls(0), 3);
        assert_eq!(max_rolls(1), 3);
        assert_eq!(max_rolls(2), 4);
        assert_eq!(max_rolls(4), 4);
        assert_eq!(min_rolls(1), 2);
        assert_eq!(min_rolls(3), 3);
        assert_eq!(stat_from_value(12.4, Goal::Build), 12.0);
        assert_eq!(stat_from_value(12.6, Goal::Build), 13.0);
        assert_eq!(stat_from_value(100.0, Goal::Flaggy), 40.0);
        assert_eq!(stat_from_value(0.0, Goal::ConExp), 2.0);
        assert_eq!(stat_from_value(100.0, Goal::ConExp), 21.0);
    }

    #[test]
    fn the_roll_range_reports_each_bound_and_its_next_level() {
        let range = roll_range(Goal::ConExp, 3, 8005);
        assert_eq!(range.tier, 3);
        assert_eq!(range.rolls, 4);
        assert!((range.raw_magnitude - 51_170.295_033_475_7).abs() < 1e-6);
        assert_eq!(range.per_roll_min, 91.0);
        assert_eq!(range.per_roll_max, 165.0);
        assert_eq!(range.total_max, 660.0);
        assert_eq!((range.min_surround, range.max_surround), (20.0, 65.0));
        assert_eq!(range.next_min_level, 8183);
        assert_eq!(range.next_min_value, 92.0);
        assert_eq!(range.next_max_level, 8049);
        assert_eq!(range.next_max_value, 166.0);

        let crystal = roll_range(Goal::Build, 4, 8005);
        assert_eq!((crystal.min_surround, crystal.max_surround), (53.0, 155.0));
        assert_eq!(crystal.total_max, 911_016.0);
    }

    #[test]
    fn the_shown_rows_are_ulti_and_jewelled() {
        let ranges = roll_ranges(Goal::Flaggy, 8005);
        assert_eq!(
            ranges.iter().map(|range| range.tier).collect::<Vec<_>>(),
            vec![3, 4]
        );
    }

    #[test]
    fn a_saved_d_infers_the_level_its_perfect_roll_needs() {
        // A tier 3 cog's four rolls at the given level: the level the game
        // would need to roll this `d` as a perfect cog.
        let level = 8005.0;
        let perfect = 4.0 * stat_from_value(3.0 * raw_magnitude(3, level), Goal::ConExp);
        let inferred = infer_construction_level(perfect, 3);
        assert!(
            inferred <= 8005,
            "a perfect roll at 8005 needs no higher level, not {inferred}"
        );
        assert_eq!(
            stat_from_value(3.0 * raw_magnitude(3, f64::from(inferred)), Goal::ConExp),
            perfect / 4.0
        );
        assert!(infer_construction_level(perfect / 2.0, 3) < inferred);
        assert_eq!(infer_construction_level(0.0, 3), 0);
    }
}
