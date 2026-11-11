//! Chest outcomes: the fleet's tier chances, what each tier pays and the
//! near-full pile transitions.
//!
//! The upstream chest odds tab reads three things. `aggregateFleetTiers` weights
//! every captain's tier odds by that boat's exact-tier chest rate, so the fleet
//! row is an arrival-weighted average rather than a flat mean.
//! `fleetChestPayoutBreakdown` turns one tier into its stored value, its
//! guaranteed payouts and the expected amount of each treasure currency. The
//! near-full pile block repeats one ordinary chest transition at a time:
//! Unending Loot Search's promotion chance and the opened value it gains.
//!
//! Pile state itself is simulated by the fleet pile simulator; nothing here
//! depends on how full the pile is, which is why these numbers stay a
//! lower-bound view of a held pile.

use super::{
    ArtifactUnlocks, Fleet, RouteContext, RouteMetrics, chest_artifact_multiplier,
    chest_loot_multiplier, chest_loot_rolls, island_artifact_count, island_artifact_offset,
    overflow_upgrade_chance,
};

/// The highest artifact tier the game has.
pub const MAX_ARTIFACT_TIER: u32 = 7;

/// The highest artifact tier this save can reach: tier seven needs
/// Verisimilitude (Menagerie unlock 0), otherwise tier six is the cap.
pub fn max_artifact_tier(unlocks: ArtifactUnlocks) -> u32 {
    if unlocks.verisimilitude {
        MAX_ARTIFACT_TIER
    } else {
        MAX_ARTIFACT_TIER - 1
    }
}

/// How many of an island's artifacts are still below the reachable cap, the
/// count the artifact ranking shows.
pub fn incomplete_artifacts(fleet: &Fleet, island: usize, unlocks: ArtifactUnlocks) -> usize {
    let cap = max_artifact_tier(unlocks);
    let offset = island_artifact_offset(island);
    (0..island_artifact_count(island))
        .filter(|index| fleet.artifact_tier(offset + index) < cap)
        .count()
}

/// One chest tier across the whole fleet.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FleetChestTier {
    pub tier: usize,
    /// Arrival-weighted exact chance that a generated chest is this tier.
    pub probability: f64,
    /// Arrival-weighted chance of this tier or higher.
    pub cumulative: f64,
    /// Chests of this exact tier per hour across the fleet.
    pub expected_per_hour: f64,
    pub loot_multiplier: f64,
    pub artifact_multiplier: f64,
}

/// The fleet's chest tiers and its total chest rate.
#[derive(Clone, Debug, PartialEq)]
pub struct FleetChestOutcomes {
    pub generated_per_hour: f64,
    pub tiers: Vec<FleetChestTier>,
}

/// What one treasure currency expects from one opened chest of a tier.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CurrencyPayout {
    /// The chance one payout lands on this currency.
    pub chance_per_payout: f64,
    /// Expected payouts on this currency per opened chest.
    pub expected_payouts_per_chest: f64,
    /// Expected value paid in this currency per opened chest.
    pub expected_value_per_chest: f64,
    /// The same value per hour at the fleet's exact-tier chest rate.
    pub expected_value_per_hour: f64,
}

/// The three treasure currencies' payouts, as upstream keys them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CurrencyPayouts {
    pub gold: CurrencyPayout,
    pub primary: CurrencyPayout,
    pub rare: CurrencyPayout,
}

/// One chest tier's fleet payout, weighted by which boats generate it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChestPayoutBreakdown {
    pub tier: usize,
    /// The tier's stored-value multiplier.
    pub multiplier: f64,
    /// The guaranteed payouts one chest of this tier rolls.
    pub payouts: u32,
    /// Exact-tier chests per hour across the fleet.
    pub chest_rate: f64,
    /// The stored value per payout, averaged over the boats that generate it.
    pub average_stored_value: f64,
    /// `average_stored_value * payouts`.
    pub total_value_per_chest: f64,
    pub currencies: CurrencyPayouts,
}

/// One ordinary near-full promotion: a banked chest of `tier` promoted to
/// `next_tier` by Unending Loot Search.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NearFullTier {
    pub tier: usize,
    pub next_tier: usize,
    /// The promotion chance per eligible match.
    pub chance: f64,
    pub loot_multiplier: f64,
    pub next_loot_multiplier: f64,
    /// Opened value after the promotion over the value before it.
    pub value_ratio: f64,
    /// The artifact-find multiplier the promoted chest carries.
    pub artifact_multiplier: f64,
}

/// Every captain's tier odds weighted by that boat's exact-tier chest rate,
/// matching upstream `aggregateFleetTiers`.
pub fn aggregate_fleet_tiers(routes: &[RouteMetrics]) -> FleetChestOutcomes {
    let generated: f64 = routes
        .iter()
        .map(|route| route.generated_chests_per_hour)
        .sum();
    let mut tiers = Vec::with_capacity(6);
    for tier in 0..6 {
        let mut exact_per_hour = 0.0;
        let mut cumulative_per_hour = 0.0;
        for route in routes {
            let Some(row) = route.rarity.tiers.get(tier) else {
                continue;
            };
            exact_per_hour += route.generated_chests_per_hour * row.probability;
            cumulative_per_hour += route.generated_chests_per_hour * row.cumulative;
        }
        tiers.push(FleetChestTier {
            tier,
            probability: if generated > 0.0 {
                exact_per_hour / generated
            } else {
                0.0
            },
            cumulative: if generated > 0.0 {
                cumulative_per_hour / generated
            } else {
                0.0
            },
            expected_per_hour: exact_per_hour,
            loot_multiplier: chest_loot_multiplier(tier),
            artifact_multiplier: chest_artifact_multiplier(tier),
        });
    }
    FleetChestOutcomes {
        generated_per_hour: generated,
        tiers,
    }
}

/// One tier's payout across the fleet, matching upstream
/// `fleetChestPayoutBreakdown`.
pub fn fleet_chest_payout_breakdown(
    routes: &[RouteMetrics],
    tier: usize,
    context: &RouteContext,
) -> ChestPayoutBreakdown {
    let tier = tier.min(5);
    let multiplier = chest_loot_multiplier(tier);
    let payouts = chest_loot_rolls(tier, context);
    let mut chest_rate = 0.0;
    let mut stored_value_rate = 0.0;
    let mut payout_rates = [0.0_f64; 3];
    let mut value_rates = [0.0_f64; 3];

    for route in routes {
        let probability = route
            .rarity
            .tiers
            .get(tier)
            .map_or(0.0, |row| row.probability);
        let route_rate = route.generated_chests_per_hour * probability;
        let stored_value = route.loot.value * multiplier;
        let shares = [
            route.treasure_shares.gold,
            route.treasure_shares.primary,
            route.treasure_shares.rare,
        ];
        let rolled = f64::from(payouts);
        chest_rate += route_rate;
        stored_value_rate += route_rate * stored_value;
        for (index, share) in shares.into_iter().enumerate() {
            payout_rates[index] += route_rate * rolled * share;
            value_rates[index] += route_rate * rolled * share * stored_value;
        }
    }

    let average_stored_value = if chest_rate > 0.0 {
        stored_value_rate / chest_rate
    } else {
        0.0
    };
    let currency = |payout_rate: f64, value_rate: f64| CurrencyPayout {
        chance_per_payout: if chest_rate > 0.0 {
            payout_rate / (chest_rate * f64::from(payouts))
        } else {
            0.0
        },
        expected_payouts_per_chest: if chest_rate > 0.0 {
            payout_rate / chest_rate
        } else {
            0.0
        },
        expected_value_per_chest: if chest_rate > 0.0 {
            value_rate / chest_rate
        } else {
            0.0
        },
        expected_value_per_hour: value_rate,
    };

    ChestPayoutBreakdown {
        tier,
        multiplier,
        payouts,
        chest_rate,
        average_stored_value,
        total_value_per_chest: average_stored_value * f64::from(payouts),
        currencies: CurrencyPayouts {
            gold: currency(payout_rates[0], value_rates[0]),
            primary: currency(payout_rates[1], value_rates[1]),
            rare: currency(payout_rates[2], value_rates[2]),
        },
    }
}

/// The four ordinary promotions the near-full scan can roll, tier four to
/// Miracle excluded as upstream does.
pub fn near_full_pile(context: &RouteContext) -> Vec<NearFullTier> {
    (0..4)
        .map(|tier| {
            let loot_multiplier = chest_loot_multiplier(tier);
            let next_loot_multiplier = chest_loot_multiplier(tier + 1);
            let rolls = f64::from(chest_loot_rolls(tier, context));
            let next_rolls = f64::from(chest_loot_rolls(tier + 1, context));
            let before = loot_multiplier * rolls;
            NearFullTier {
                tier,
                next_tier: tier + 1,
                chance: overflow_upgrade_chance(tier as u32, context.unending_loot_search),
                loot_multiplier,
                next_loot_multiplier,
                value_ratio: if before > 0.0 {
                    next_loot_multiplier * next_rolls / before
                } else {
                    0.0
                },
                artifact_multiplier: chest_artifact_multiplier(1),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sailing::{Boat, Captain, Fleet, Goal, artifact_count, saved};

    fn fleet() -> Fleet {
        Fleet {
            boats: vec![
                Boat {
                    captain: Some(0),
                    loot_level: 100,
                    speed_level: 100,
                    ..Boat::default()
                },
                Boat {
                    captain: Some(1),
                    loot_level: 60,
                    speed_level: 60,
                    ..Boat::default()
                },
            ],
            captains: vec![
                Captain {
                    rarity: 3,
                    bonus_types: [0, 1],
                    level: 10,
                    exp: 0.0,
                    rolls: [20.0, 10.0],
                },
                Captain {
                    rarity: 6,
                    bonus_types: [4, 3],
                    level: 40,
                    exp: 0.0,
                    rolls: [5.0, 12.0],
                },
            ],
            sailing_level: 10,
            artifact_tiers: vec![0; artifact_count()],
            island_progress: vec![-1.0; 20],
            currency_balances: Vec::new(),
        }
    }

    fn routes() -> (Fleet, RouteContext, Vec<RouteMetrics>) {
        let fleet = fleet();
        let context = RouteContext::default();
        let plan = saved(&fleet, 0, Goal::Treasure, &context);
        (fleet, context, plan.routes)
    }

    #[test]
    fn the_fleet_tier_chances_sum_to_one() {
        let (_, _, routes) = routes();
        let outcomes = aggregate_fleet_tiers(&routes);
        let sum: f64 = outcomes.tiers.iter().map(|tier| tier.probability).sum();
        assert!((sum - 1.0).abs() < 1e-12, "chances sum to {sum}");
        let cumulative: f64 = outcomes
            .tiers
            .iter()
            .map(|tier| tier.expected_per_hour)
            .sum();
        assert!(
            (cumulative - outcomes.generated_per_hour).abs() <= outcomes.generated_per_hour * 1e-12,
            "every generated chest lands in exactly one tier"
        );
    }

    #[test]
    fn the_fleet_row_is_weighted_by_each_boats_chest_rate() {
        let (_, _, routes) = routes();
        let outcomes = aggregate_fleet_tiers(&routes);
        let expected: Vec<f64> = (0..6)
            .map(|tier| {
                let rate: f64 = routes
                    .iter()
                    .map(|route| {
                        route.generated_chests_per_hour * route.rarity.tiers[tier].probability
                    })
                    .sum();
                rate / outcomes.generated_per_hour
            })
            .collect();
        for (row, expected) in outcomes.tiers.iter().zip(expected) {
            assert!((row.probability - expected).abs() < 1e-12);
            assert!((row.expected_per_hour - expected * outcomes.generated_per_hour).abs() < 1e-9);
        }
    }

    #[test]
    fn the_payout_breakdown_pays_every_roll_in_one_currency() {
        let (_, context, routes) = routes();
        for tier in 0..6 {
            let breakdown = fleet_chest_payout_breakdown(&routes, tier, &context);
            assert_eq!(breakdown.payouts, chest_loot_rolls(tier, &context));
            assert_eq!(breakdown.multiplier, chest_loot_multiplier(tier));
            let shares = breakdown.currencies;
            let chance = shares.gold.chance_per_payout
                + shares.primary.chance_per_payout
                + shares.rare.chance_per_payout;
            assert!((chance - 1.0).abs() < 1e-12, "tier {tier}: {chance}");
            let payouts = shares.gold.expected_payouts_per_chest
                + shares.primary.expected_payouts_per_chest
                + shares.rare.expected_payouts_per_chest;
            assert!((payouts - f64::from(breakdown.payouts)).abs() < 1e-12);
            let value = shares.gold.expected_value_per_chest
                + shares.primary.expected_value_per_chest
                + shares.rare.expected_value_per_chest;
            assert!((value - breakdown.total_value_per_chest).abs() <= value.max(1.0) * 1e-12);
        }
    }

    #[test]
    fn an_empty_fleet_has_no_chance_and_no_payout() {
        let context = RouteContext::default();
        let outcomes = aggregate_fleet_tiers(&[]);
        assert_eq!(outcomes.generated_per_hour, 0.0);
        assert!(outcomes.tiers.iter().all(|tier| tier.probability == 0.0));
        let breakdown = fleet_chest_payout_breakdown(&[], 2, &context);
        assert_eq!(breakdown.chest_rate, 0.0);
        assert_eq!(breakdown.average_stored_value, 0.0);
        assert_eq!(breakdown.currencies.gold.chance_per_payout, 0.0);
    }

    #[test]
    fn near_full_promotions_gain_the_tier_multiplier_and_one_payout() {
        let context = RouteContext {
            unending_loot_search: 100.0,
            ..RouteContext::default()
        };
        let rows = near_full_pile(&context);
        assert_eq!(rows.len(), 4);
        for row in &rows {
            assert_eq!(row.next_tier, row.tier + 1);
            assert!(row.value_ratio > row.next_loot_multiplier / row.loot_multiplier);
            assert_eq!(row.artifact_multiplier, 1.4);
        }
        assert_eq!(rows[0].chance, 0.25);
        assert_eq!(rows[3].chance, 0.0003);
        assert_eq!(rows[3].chance, overflow_upgrade_chance(3, 100.0));
    }

    #[test]
    fn verisimilitude_raises_the_artifact_cap() {
        let fleet = fleet();
        let locked = ArtifactUnlocks::default();
        assert_eq!(max_artifact_tier(locked), 6);
        assert_eq!(
            incomplete_artifacts(&fleet, 0, locked),
            island_artifact_count(0)
        );
        let unlocked = ArtifactUnlocks {
            verisimilitude: true,
            ..ArtifactUnlocks::default()
        };
        assert_eq!(max_artifact_tier(unlocked), 7);

        let mut fleet = fleet;
        for tier in &mut fleet.artifact_tiers {
            *tier = 7;
        }
        assert_eq!(incomplete_artifacts(&fleet, 0, unlocked), 0);
        assert_eq!(
            incomplete_artifacts(&fleet, 0, locked),
            0,
            "a maxed artifact stays maxed below the raised cap"
        );
        fleet.artifact_tiers[0] = 6;
        assert_eq!(incomplete_artifacts(&fleet, 0, unlocked), 1);
        assert_eq!(incomplete_artifacts(&fleet, 0, locked), 0);
    }
}
