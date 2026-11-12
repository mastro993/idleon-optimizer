//! Route maths: loot value, speed, chest tiers, artifact odds and cadence.

use super::{
    ArtifactUnlocks, Boat, Fleet, MinimumTravel, RouteContext, ServerVars, SpeedBonuses,
    artifact_base_odds, artifact_tier_bonus, island_artifact_count, island_artifact_offset,
    island_distance,
};

/// The six chest tiers' cumulative thresholds, as upstream stores them.
const CHEST_CUMULATIVE_THRESHOLDS: [f64; 6] = [1.0, 0.2, 0.04, 0.006, 0.00023, 0.00000001];
/// Chest loot multipliers for tiers 0 through 5.
const CHEST_LOOT_MULTIS: [f64; 6] = [
    1.0,
    1.85,
    1.85 * 1.85,
    1.85 * 1.85 * 1.85,
    1.85 * 1.85 * 1.85 * 1.85,
    20.0,
];
/// Chest artifact multipliers for tiers 0 through 5.
const CHEST_ARTIFACT_MULTIS: [f64; 6] = [
    1.0,
    1.4,
    1.4 * 1.4,
    1.4 * 1.4 * 1.4,
    1.4 * 1.4 * 1.4 * 1.4,
    30.0,
];

/// A boat's loot value and the factors behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LootValue {
    pub value: f64,
    pub base: f64,
    pub captain: f64,
    /// Additive account bonus pool, including legendary captains.
    pub additive: f64,
    pub multiplier: f64,
    pub davey: f64,
}

/// A boat's speed and the factors behind it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpeedValue {
    pub value: f64,
    pub base: f64,
    pub captain: f64,
    pub first: f64,
    pub davey: f64,
    pub bless4: f64,
    pub bless6: f64,
    pub vote: f64,
    pub second: f64,
}

/// A route's trip cadence.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RouteTiming {
    pub distance: f64,
    pub raw_speed: f64,
    pub effective_speed: f64,
    pub capped: bool,
    pub minimum_minutes: f64,
    pub trip_seconds: f64,
    pub trips_per_hour: f64,
}

/// One chest tier's probability and multipliers.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChestTier {
    pub tier: usize,
    pub probability: f64,
    pub cumulative: f64,
    pub loot_multiplier: f64,
    pub artifact_multiplier: f64,
}

/// A captain's chest-rarity distribution.
#[derive(Clone, Debug, PartialEq)]
pub struct ChestDistribution {
    pub rarity_scale: f64,
    pub captain: f64,
    pub artifact: f64,
    pub tiers: Vec<ChestTier>,
}

/// A boat's artifact-find multiplier.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ArtifactMultiplier {
    pub value: f64,
    pub captain: f64,
    pub additive: f64,
    /// `max(1, 1 + (additive + captain) / 100)`, before the account factors.
    pub additive_factor: f64,
    /// The product of the multiplicative account sources.
    pub multiplier: f64,
}

/// One artifact's odds on an island.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactOutcome {
    pub artifact: usize,
    pub name: String,
    pub current_tier: u32,
    pub next_tier: u32,
    pub unlocked: bool,
    pub denominator: f64,
    pub chance: f64,
    pub exact_chance: f64,
    pub chance_per_trip: f64,
    pub finds_per_hour: f64,
}

/// Every artifact an island can drop, and the chance of any drop.
#[derive(Clone, Debug, PartialEq)]
pub struct ArtifactOdds {
    pub any_chance: f64,
    pub artifacts: Vec<ArtifactOutcome>,
}

/// One tier's contribution to a route.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TierRow {
    pub tier: usize,
    pub probability: f64,
    pub kept: bool,
    pub chest_value: f64,
    pub rolls: u32,
    pub treasure_value: f64,
    pub artifact_multiplier: f64,
    pub artifact_chance: f64,
}

/// How a route's treasure splits across the three sailing currencies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TreasureShares {
    pub gold: f64,
    pub primary: f64,
    pub rare: f64,
}

impl Default for TreasureShares {
    /// Below Sailing level 5 every payout is gold bars, as upstream splits it.
    fn default() -> Self {
        Self {
            gold: 1.0,
            primary: 0.0,
            rare: 0.0,
        }
    }
}

/// Everything one boat/captain pairing on one island produces.
#[derive(Clone, Debug, PartialEq)]
pub struct RouteMetrics {
    pub boat: usize,
    pub captain: Option<usize>,
    pub island: usize,
    pub minimum_tier: u32,
    pub timing: RouteTiming,
    /// The raw speed and the factors behind it.
    pub speed: SpeedValue,
    pub loot: LootValue,
    pub rarity: ChestDistribution,
    pub artifact_base: ArtifactMultiplier,
    pub tiers: Vec<TierRow>,
    pub kept_probability: f64,
    pub generated_chests_per_hour: f64,
    pub kept_chests_per_hour: f64,
    pub chest_value_per_hour: f64,
    pub treasure_per_hour: f64,
    pub artifact_chance_per_trip: f64,
    pub artifact_finds_per_hour: f64,
    pub artifact_outcomes: Vec<ArtifactOutcome>,
    pub treasure_shares: TreasureShares,
}

impl RouteMetrics {
    /// The score a goal maximises.
    pub fn score(&self, goal: super::Goal) -> f64 {
        match goal {
            super::Goal::Treasure => self.treasure_per_hour,
            super::Goal::Artifact => self.artifact_finds_per_hour,
            super::Goal::Chests => self.kept_chests_per_hour,
        }
    }
}

/// A captain's bonus of `bonus_type`, in percent.
pub fn captain_bonus(fleet: &Fleet, captain: Option<usize>, bonus_type: i32) -> f64 {
    let Some(captain) = captain.and_then(|idx| fleet.captains.get(idx)) else {
        return 0.0;
    };
    let mut total = 0.0;
    for slot in 0..2 {
        if captain.bonus_types[slot] == bonus_type {
            total += captain.level as f64 * captain.rolls[slot];
        }
    }
    total
}

/// Davey Jones Training and Returns: `(1 + (50 * gems + legend) / 100) *
/// (1 + 2 * active)`, where active needs research grid 105 and a level-400 boat.
pub(crate) fn davey_jones_bonus(boat: &Boat, context: &RouteContext) -> f64 {
    let training = 1.0 + (50.0 * context.travel.gem_purchases + context.travel.legend) / 100.0;
    let active = context.davey_unlocked && boat.loot_level + boat.speed_level >= 400;
    training * (1.0 + if active { 2.0 } else { 0.0 })
}

/// A boat's loot value, matching upstream `boatLootValue`.
pub(crate) fn boat_loot_value(
    fleet: &Fleet,
    boat_idx: usize,
    captain: Option<usize>,
    context: &RouteContext,
) -> LootValue {
    let Some(boat) = fleet.boats.get(boat_idx) else {
        return LootValue {
            value: 0.0,
            base: 0.0,
            captain: 0.0,
            additive: 0.0,
            multiplier: 1.0,
            davey: 1.0,
        };
    };
    let base = boat_base_loot(boat.loot_level);
    let captain = captain_bonus(fleet, captain, 1);
    let legendary = 25.0 * fleet.legendary_captains() as f64;
    let additive = context.loot.additive + legendary;
    // Unending Loot Search (talent 325) is also a passive chest-loot bonus, so
    // it multiplies the boat value beside the account's own factors.
    let multiplier = (1.0 + additive / 100.0)
        * context.loot.multiplier
        * (1.0 + context.unending_loot_search / 100.0);
    let davey = davey_jones_bonus(boat, context);
    LootValue {
        value: base * (1.0 + captain / 100.0) * davey * multiplier,
        base,
        captain,
        additive,
        multiplier,
        davey,
    }
}

/// A boat's speed, matching upstream `boatSpeed`.
pub(crate) fn boat_speed(
    fleet: &Fleet,
    boat_idx: usize,
    captain: Option<usize>,
    context: &RouteContext,
) -> SpeedValue {
    let Some(boat) = fleet.boats.get(boat_idx) else {
        return SpeedValue {
            value: 0.0,
            base: 0.0,
            captain: 0.0,
            first: 1.0,
            davey: 1.0,
            bless4: 0.0,
            bless6: 0.0,
            vote: 0.0,
            second: 1.0,
        };
    };
    let base = boat_base_speed(boat.speed_level);
    let captain = captain_bonus(fleet, captain, 0);
    let davey = davey_jones_bonus(boat, context);
    let SpeedBonuses {
        first,
        bless4,
        bless6,
        vote,
        second,
    } = context.speed;
    SpeedValue {
        value: base
            * (1.0 + captain / 100.0)
            * first
            * davey
            * (1.0 + bless4 / 100.0)
            * (1.0 + bless6 / 100.0)
            * (1.0 + vote / 100.0)
            * second,
        base,
        captain,
        first,
        davey,
        bless4,
        bless6,
        vote,
        second,
    }
}

/// Boat loot at a level, upstream `boatBaseLoot`.
pub(crate) fn boat_base_loot(level: u32) -> f64 {
    let level = level as f64;
    5.0 + (2.0 + (level / 8.0).floor().powi(2)) * level
}

/// Boat speed at a level, upstream `boatBaseSpeed`.
pub(crate) fn boat_base_speed(level: u32) -> f64 {
    let level = level as f64;
    10.0 + (5.0 + (level / 7.0).floor().powi(2)) * level
}

/// The shortest-trip breakdown the save supplies.
pub(crate) fn minimum_travel_details(context: &RouteContext) -> MinimumTravel {
    context.travel
}

/// A route's distance, speed cap and trip cadence.
pub(crate) fn route_timing(
    fleet: &Fleet,
    boat_idx: usize,
    island: usize,
    captain: Option<usize>,
    context: &RouteContext,
) -> RouteTiming {
    let raw = boat_speed(fleet, boat_idx, captain, context);
    let distance = island_distance(island);
    let minimum_minutes = minimum_travel_details(context).minutes();
    let cap = if minimum_minutes > 0.0 {
        60.0 * distance / minimum_minutes
    } else {
        f64::INFINITY
    };
    let effective_speed = raw.value.min(cap);
    let trip_seconds = if effective_speed > 0.0 {
        3600.0 * distance / effective_speed
    } else {
        f64::INFINITY
    };
    let trips_per_hour = if trip_seconds.is_finite() && trip_seconds > 0.0 {
        3600.0 / trip_seconds
    } else {
        0.0
    };
    RouteTiming {
        distance,
        raw_speed: raw.value,
        effective_speed,
        capped: raw.value >= cap,
        minimum_minutes,
        trip_seconds,
        trips_per_hour,
    }
}

/// A captain's chest-rarity scale: captain rarity bonus and artifact 9.
pub(crate) fn chest_rarity_scale(fleet: &Fleet, captain: Option<usize>) -> (f64, f64, f64) {
    let captain = captain_bonus(fleet, captain, 4);
    let artifact = artifact_tier_bonus(9, fleet.artifact_tier(9));
    (1.0 + (captain + artifact) / 200.0, captain, artifact)
}

/// The six chest tiers' probabilities, which always sum to one.
pub fn chest_tier_distribution(fleet: &Fleet, captain: Option<usize>) -> ChestDistribution {
    let (rarity_scale, captain, artifact) = chest_rarity_scale(fleet, captain);
    let mut cumulative = [0.0; 6];
    for tier in 0..6 {
        cumulative[tier] = (CHEST_CUMULATIVE_THRESHOLDS[tier] * rarity_scale).min(1.0);
    }
    let mut tiers = Vec::with_capacity(6);
    for tier in 0..6 {
        let probability = if tier == 5 {
            cumulative[5]
        } else {
            cumulative[tier] - cumulative[tier + 1]
        };
        tiers.push(ChestTier {
            tier,
            probability: probability.max(0.0),
            cumulative: cumulative[tier],
            loot_multiplier: CHEST_LOOT_MULTIS[tier],
            artifact_multiplier: CHEST_ARTIFACT_MULTIS[tier],
        });
    }
    ChestDistribution {
        rarity_scale,
        captain,
        artifact,
        tiers,
    }
}

/// How many loot rolls a chest of `tier` makes.
pub(crate) fn chest_loot_rolls(tier: usize, context: &RouteContext) -> u32 {
    1 + tier.min(4) as u32 + context.chest_rolls
}

/// The stored-value multiplier a chest of `tier` applies, `1` for tier zero.
pub fn chest_loot_multiplier(tier: usize) -> f64 {
    CHEST_LOOT_MULTIS.get(tier).copied().unwrap_or(0.0)
}

/// The artifact-find multiplier a chest of `tier` applies, `1` for tier zero.
pub(crate) fn chest_artifact_multiplier(tier: usize) -> f64 {
    CHEST_ARTIFACT_MULTIS.get(tier).copied().unwrap_or(0.0)
}

/// A boat's artifact-find multiplier, matching upstream `boatArtifactMultiplier`.
pub(crate) fn boat_artifact_multiplier(
    fleet: &Fleet,
    boat_idx: usize,
    captain: Option<usize>,
    context: &RouteContext,
) -> ArtifactMultiplier {
    let _ = boat_idx;
    let captain = captain_bonus(fleet, captain, 3);
    let legendary = 25.0 * fleet.legendary_captains() as f64;
    let additive = context.artifact.additive + legendary;
    let additive_factor = (1.0 + (additive + captain) / 100.0).max(1.0);
    ArtifactMultiplier {
        value: additive_factor * context.artifact.multiplier,
        captain,
        additive,
        additive_factor,
        multiplier: context.artifact.multiplier,
    }
}

/// The denominator for one artifact at its current tier.
pub(crate) fn artifact_odds_denominator(
    island: usize,
    artifact: usize,
    current_tier: u32,
    server_vars: ServerVars,
) -> f64 {
    let island = island as f64;
    let ancient_step = server_vars.ancient_odd_per_island;
    let server_multi = 1.0 + server_vars.ancient_arti_pct / 100.0;
    match current_tier {
        0 => artifact_base_odds(artifact),
        1 => {
            if island < 3.0 {
                850.0
            } else {
                (1000.0 + (island - 3.0) * ancient_step) / server_multi
            }
        }
        2 => {
            if island < 3.0 {
                900.0 + 250.0 * island
            } else {
                (1000.0 + (island - 3.0) * ancient_step) / server_multi * 4.0
            }
        }
        3 => {
            if island < 5.0 {
                9000.0 + 2000.0 * island
            } else {
                (1000.0 + 1.25 * (island - 3.0) * ancient_step) / server_multi * 180.0
            }
        }
        4 => {
            if island < 6.0 {
                120000.0 + 40000.0 * island
            } else {
                10000.0
                    * (1.0 + 0.5 * (island - 5.0))
                    * ((3000.0 + 3.0 * (island - 5.0) * ancient_step) / server_multi)
            }
        }
        5 => {
            if island < 6.0 {
                40000000.0 + 60000000.0 * island
            } else {
                1000000.0
                    * (1.0 + island - 5.0)
                    * (1.7_f64).powf((island - 7.0).max(0.0))
                    * (1.0 + 0.5 * (island - 9.0).max(0.0))
                    * ((10000.0 + 5000.0 * (island - 5.0) * (ancient_step / 960.0)) / server_multi)
            }
        }
        6 => {
            if island < 6.0 {
                5.0 * 1e14 * 3_f64.powf(island)
            } else {
                1e16 * (1.0 + 10.0 * (island - 5.0) / 100.0)
                    * (1.4_f64).powf((island - 7.0).max(0.0))
                    * (1.0 + 10.0 * (island - 9.0).max(0.0) / 100.0)
                    * ((10000.0 + 1000.0 * (island - 5.0) * (ancient_step / 960.0)) / server_multi)
            }
        }
        _ => f64::INFINITY,
    }
}

/// Whether an artifact's next tier is unlocked by the save.
pub(crate) fn artifact_tier_unlocked(current_tier: u32, unlocks: ArtifactUnlocks) -> bool {
    match current_tier {
        0 | 1 => true,
        2 => unlocks.rift,
        3 => unlocks.ninja,
        4 => unlocks.spelunk,
        5 => unlocks.grid_109,
        6 => unlocks.verisimilitude,
        _ => false,
    }
}

/// One island's artifact odds at `artifact_multiplier`.
pub(crate) fn island_artifact_odds(
    fleet: &Fleet,
    island: usize,
    artifact_multiplier: f64,
    context: &RouteContext,
) -> ArtifactOdds {
    let offset = island_artifact_offset(island);
    let count = island_artifact_count(island);
    let mut fail = 1.0;
    let mut artifacts = Vec::with_capacity(count);
    for index in 0..count {
        let artifact = offset + index;
        let current_tier = fleet.artifact_tier(artifact);
        let unlocked = current_tier < 7 && artifact_tier_unlocked(current_tier, context.unlocks);
        let denominator = if unlocked {
            artifact_odds_denominator(island, artifact, current_tier, context.server_vars)
        } else {
            f64::INFINITY
        };
        let chance = if denominator.is_finite() {
            (artifact_multiplier / denominator).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let exact_chance = fail * chance;
        fail *= 1.0 - chance;
        artifacts.push(ArtifactOutcome {
            artifact,
            name: super::artifact(artifact)
                .map(|artifact| artifact.name.clone())
                .unwrap_or_else(|| format!("Artifact {artifact}")),
            current_tier,
            next_tier: current_tier + 1,
            unlocked,
            denominator,
            chance,
            exact_chance,
            chance_per_trip: 0.0,
            finds_per_hour: 0.0,
        });
    }
    ArtifactOdds {
        any_chance: 1.0 - fail,
        artifacts,
    }
}

/// Everything one boat/captain pairing on one island produces.
pub fn route_metrics(
    fleet: &Fleet,
    boat_idx: usize,
    island: usize,
    captain: Option<usize>,
    context: &RouteContext,
) -> RouteMetrics {
    let minimum_tier = context.minimum_tier.min(5);
    let loot = boat_loot_value(fleet, boat_idx, captain, context);
    let speed = boat_speed(fleet, boat_idx, captain, context);
    let timing = route_timing(fleet, boat_idx, island, captain, context);
    let rarity = chest_tier_distribution(fleet, captain);
    let artifact_base = boat_artifact_multiplier(fleet, boat_idx, captain, context);
    let mut kept_probability = 0.0;
    let mut chest_value_per_trip = 0.0;
    let mut treasure_per_trip = 0.0;
    let mut artifact_chance_per_trip = 0.0;
    let mut artifact_outcomes: Vec<ArtifactOutcome> = Vec::new();
    let mut tiers = Vec::with_capacity(6);

    for tier in &rarity.tiers {
        let kept = tier.tier as u32 >= minimum_tier;
        let chest_value = loot.value * tier.loot_multiplier;
        let rolls = chest_loot_rolls(tier.tier, context);
        let artifact_multiplier = artifact_base.value * tier.artifact_multiplier;
        let odds = island_artifact_odds(fleet, island, artifact_multiplier, context);
        if kept {
            kept_probability += tier.probability;
            chest_value_per_trip += tier.probability * chest_value;
            treasure_per_trip += tier.probability * chest_value * rolls as f64;
            artifact_chance_per_trip += tier.probability * odds.any_chance;
            for row in &odds.artifacts {
                let position = artifact_outcomes
                    .iter()
                    .position(|existing| existing.artifact == row.artifact);
                let position = position.unwrap_or_else(|| {
                    artifact_outcomes.push(ArtifactOutcome {
                        chance_per_trip: 0.0,
                        finds_per_hour: 0.0,
                        ..row.clone()
                    });
                    artifact_outcomes.len() - 1
                });
                artifact_outcomes[position].chance_per_trip += tier.probability * row.exact_chance;
            }
        }
        tiers.push(TierRow {
            tier: tier.tier,
            probability: tier.probability,
            kept,
            chest_value,
            rolls,
            treasure_value: chest_value * rolls as f64,
            artifact_multiplier,
            artifact_chance: odds.any_chance,
        });
    }

    let trips = timing.trips_per_hour;
    let (gold, primary, rare) = if fleet.sailing_level < 5 {
        (1.0, 0.0, 0.0)
    } else {
        (0.5, 0.475, 0.025)
    };
    for outcome in &mut artifact_outcomes {
        outcome.finds_per_hour = trips * outcome.chance_per_trip;
    }
    RouteMetrics {
        boat: boat_idx,
        captain,
        island,
        minimum_tier,
        timing,
        speed,
        loot,
        rarity,
        artifact_base,
        tiers,
        kept_probability,
        generated_chests_per_hour: trips,
        kept_chests_per_hour: trips * kept_probability,
        chest_value_per_hour: trips * chest_value_per_trip,
        treasure_per_hour: trips * treasure_per_trip,
        artifact_chance_per_trip,
        artifact_finds_per_hour: trips * artifact_chance_per_trip,
        artifact_outcomes,
        treasure_shares: TreasureShares {
            gold,
            primary,
            rare,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Boat, Captain, Fleet, Goal, RouteContext, artifact_count};

    fn fleet() -> Fleet {
        Fleet {
            boats: vec![Boat {
                captain: Some(0),
                loot_level: 100,
                speed_level: 100,
                ..Boat::default()
            }],
            captains: vec![Captain {
                rarity: 3,
                bonus_types: [0, 1],
                level: 10,
                exp: 0.0,
                rolls: [20.0, 10.0],
            }],
            sailing_level: 5,
            artifact_tiers: vec![0; artifact_count()],
            island_progress: Vec::new(),
            currency_balances: Vec::new(),
        }
    }

    #[test]
    fn chest_tiers_always_sum_to_one() {
        for roll in [0.0, 10.0, 40.0, 100.0, 400.0] {
            let fleet = Fleet {
                captains: vec![Captain {
                    rarity: 6,
                    bonus_types: [4, -1],
                    level: 100,
                    exp: 0.0,
                    rolls: [roll, 0.0],
                }],
                ..fleet()
            };
            let distribution = chest_tier_distribution(&fleet, Some(0));
            let sum: f64 = distribution.tiers.iter().map(|tier| tier.probability).sum();
            assert!((sum - 1.0).abs() < 1e-12, "roll {roll}: {sum}");
        }
    }

    #[test]
    fn captain_bonuses_read_both_rolled_slots() {
        let fleet = fleet();
        assert_eq!(captain_bonus(&fleet, Some(0), 0), 200.0);
        assert_eq!(captain_bonus(&fleet, Some(0), 1), 100.0);
        assert_eq!(captain_bonus(&fleet, Some(0), 3), 0.0);
        assert_eq!(captain_bonus(&fleet, None, 0), 0.0);
    }

    #[test]
    fn the_goal_score_reads_its_own_metric() {
        let route = route_metrics(&fleet(), 0, 0, Some(0), &RouteContext::default());
        assert_eq!(route.score(Goal::Treasure), route.treasure_per_hour);
        assert_eq!(route.score(Goal::Artifact), route.artifact_finds_per_hour);
        assert_eq!(route.score(Goal::Chests), route.kept_chests_per_hour);
    }
}
