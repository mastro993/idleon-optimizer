//! The strongest-source replacement matrix, from upstream `sailing-chests.js`.
//!
//! A banked chest stores no source boat or captain, so upstream answers a
//! hypothetical: if the near-full scan reached a chest with this boat's value,
//! what would replace it? Two ordinary paths exist — the same tier promoted by
//! Unending Loot Search, and a naturally higher tier on the same island.

use std::cmp::Ordering;

use super::{RouteContext, RouteMetrics, chest_loot_multiplier};

/// What one incoming source needs to replace a banked chest of one tier.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplacementThreshold {
    /// Whether the incoming route sails the same island as the reference.
    pub same_island: bool,
    /// Whether the same tier at the incoming value already meets the reference.
    pub same_tier_eligible: bool,
    pub reference_tier: u32,
    /// The banked chest's stored value.
    pub reference_value: f64,
    /// The lowest ordinary tier whose value beats the reference, if any.
    pub required_tier: Option<u32>,
    /// The incoming value at `required_tier`.
    pub replacement_value: f64,
    /// `replacement_value` over `reference_value`, minus one.
    pub margin: f64,
    /// The best an ordinary incoming chest can store, at tier four.
    pub max_value: f64,
    /// How far tier four falls short when nothing qualifies.
    pub shortfall: f64,
}

/// The chance one incoming source replaces a banked chest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ReplacementOutcome {
    /// Whether Unending Loot Search is active, which near-full replacement needs.
    pub active: bool,
    /// The talent's percentage.
    pub talent: f64,
    pub threshold: ReplacementThreshold,
    /// The incoming chest's exact chance of the reference tier.
    pub exact_same_tier_chance: f64,
    /// The conditional Unending Loot Search promotion chance.
    pub uls_chance: f64,
    /// The same-tier path: `exact_same_tier_chance * uls_chance`.
    pub same_tier_upgrade_chance: f64,
    /// The chance of a naturally higher tier at or above `required_tier`.
    pub natural_replacement_chance: f64,
    /// The chance the incoming chest is a Miracle, tier five.
    pub miracle_generation_chance: f64,
    /// Miracle finds per hour at the incoming route's chest rate.
    pub miracle_generation_rate_per_hour: f64,
    /// Either ordinary path, capped at certainty.
    pub total_chance: f64,
    /// Ordinary replacements per hour.
    pub total_rate_per_hour: f64,
}

/// Strongest source first: descending loot value, then a stable boat order.
pub fn compare_strength(left: &RouteMetrics, right: &RouteMetrics) -> Ordering {
    right
        .loot
        .value
        .total_cmp(&left.loot.value)
        .then_with(|| left.boat.cmp(&right.boat))
}

/// The fleet's routes with the strongest source first.
pub fn ranked(routes: &[RouteMetrics]) -> Vec<&RouteMetrics> {
    let mut ordered: Vec<&RouteMetrics> = routes.iter().collect();
    ordered.sort_by(|left, right| compare_strength(left, right));
    ordered
}

/// The strongest route, the matrix's reference source.
pub fn strongest(routes: &[RouteMetrics]) -> Option<&RouteMetrics> {
    ranked(routes).into_iter().next()
}

/// The conditional Unending Loot Search promotion at `tier`, upstream
/// `overflowUpgradeChance`.
pub fn overflow_upgrade_chance(tier: u32, unending_loot_search: f64) -> f64 {
    const BASE: [f64; 5] = [0.25, 0.05, 0.007, 0.0003, 0.0];
    let base = BASE.get(tier as usize).copied().unwrap_or(0.0);
    base * (0.6 + unending_loot_search / 100.0).min(1.0)
}

/// What `incoming` needs to replace a banked chest of `reference_tier`,
/// upstream `chestReplacementThreshold`.
pub fn replacement_threshold(
    incoming: &RouteMetrics,
    reference: &RouteMetrics,
    reference_tier: u32,
) -> ReplacementThreshold {
    let tier = reference_tier.min(5) as usize;
    let same_island = incoming.island == reference.island;
    let incoming_base = incoming.loot.value;
    let reference_base = reference.loot.value;
    let reference_value = reference_base * chest_loot_multiplier(tier);
    let same_tier_value = incoming_base * chest_loot_multiplier(tier);

    // Only the same island can pay a higher ordinary tier: another island's
    // chest never enters this pile.
    let required = same_island
        .then(|| {
            (tier + 1..5).find(|candidate| {
                incoming_base * chest_loot_multiplier(*candidate) > reference_value
            })
        })
        .flatten();
    let replacement_value = required.map_or(0.0, |candidate| {
        incoming_base * chest_loot_multiplier(candidate)
    });
    let max_value = incoming_base * chest_loot_multiplier(4);

    ReplacementThreshold {
        same_island,
        same_tier_eligible: tier < 5 && same_island && same_tier_value >= reference_value,
        reference_tier: tier as u32,
        reference_value,
        required_tier: required.map(|candidate| candidate as u32),
        replacement_value,
        margin: if required.is_some() && reference_value > 0.0 {
            replacement_value / reference_value - 1.0
        } else {
            0.0
        },
        max_value,
        shortfall: if required.is_none() && reference_value > 0.0 {
            (1.0 - max_value / reference_value).max(0.0)
        } else {
            0.0
        },
    }
}

/// How likely `incoming` is to replace a banked chest of `reference_tier`,
/// upstream `chestReplacementOutcome`.
pub fn replacement_outcome(
    incoming: &RouteMetrics,
    reference: &RouteMetrics,
    reference_tier: u32,
    context: &RouteContext,
) -> ReplacementOutcome {
    let threshold = replacement_threshold(incoming, reference, reference_tier);
    let tier = threshold.reference_tier as usize;
    let probability = |index: usize| {
        incoming
            .rarity
            .tiers
            .get(index)
            .map_or(0.0, |row| row.probability)
    };
    let talent = context.unending_loot_search;
    let active = talent > 0.0;
    let exact_same_tier_chance = probability(tier);
    let uls_chance = if active && threshold.same_tier_eligible {
        overflow_upgrade_chance(threshold.reference_tier, talent)
    } else {
        0.0
    };
    let natural_replacement_chance = match (active, threshold.required_tier) {
        (true, Some(required)) => (required as usize..5).map(probability).sum(),
        _ => 0.0,
    };
    let miracle_generation_chance = probability(5);
    let total_chance = (exact_same_tier_chance * uls_chance + natural_replacement_chance).min(1.0);
    let generated = incoming.generated_chests_per_hour;
    ReplacementOutcome {
        active,
        talent,
        threshold,
        exact_same_tier_chance,
        uls_chance,
        same_tier_upgrade_chance: exact_same_tier_chance * uls_chance,
        natural_replacement_chance,
        miracle_generation_chance,
        miracle_generation_rate_per_hour: generated * miracle_generation_chance,
        total_chance,
        total_rate_per_hour: generated * total_chance,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sailing::{Boat, Captain, Fleet, chest_tier_distribution, route_metrics};

    fn fleet() -> Fleet {
        Fleet {
            boats: vec![
                Boat {
                    captain: Some(0),
                    loot_level: 200,
                    speed_level: 100,
                    ..Boat::default()
                },
                Boat {
                    captain: Some(1),
                    loot_level: 100,
                    speed_level: 100,
                    ..Boat::default()
                },
            ],
            captains: vec![
                Captain {
                    rarity: 6,
                    bonus_types: [1, 4],
                    level: 20,
                    exp: 0.0,
                    rolls: [15.0, 29.0],
                },
                Captain {
                    rarity: 3,
                    bonus_types: [1, 2],
                    level: 10,
                    exp: 0.0,
                    rolls: [20.0, 10.0],
                },
            ],
            sailing_level: 10,
            artifact_tiers: vec![0; crate::sailing::artifact_count()],
            ..Fleet::default()
        }
    }

    fn routes(fleet: &Fleet, context: &RouteContext) -> Vec<RouteMetrics> {
        (0..fleet.boats.len())
            .map(|boat| route_metrics(fleet, boat, 0, fleet.boats[boat].captain, context))
            .collect()
    }

    #[test]
    fn the_ranking_is_strongest_first_and_stable_on_ties() {
        let fleet = fleet();
        let routes = routes(&fleet, &RouteContext::default());
        assert!(routes[0].loot.value > routes[1].loot.value);
        let order: Vec<usize> = ranked(&routes).iter().map(|route| route.boat).collect();
        assert_eq!(order, [0, 1]);
        assert_eq!(strongest(&routes).map(|route| route.boat), Some(0));

        // Equal sources keep the boat order whatever the input order is.
        let mut tied = routes.clone();
        tied[1].loot.value = tied[0].loot.value;
        for shuffled in [[1, 0], [0, 1]] {
            let pair = vec![tied[shuffled[0]].clone(), tied[shuffled[1]].clone()];
            let order: Vec<usize> = ranked(&pair).iter().map(|route| route.boat).collect();
            assert_eq!(order, [0, 1]);
        }
    }

    #[test]
    fn a_lower_tier_needs_uls_and_a_higher_tier_is_natural() {
        let fleet = fleet();
        let mut context = RouteContext::default();
        let routes = routes(&fleet, &context);
        let (strong, weak) = (&routes[0], &routes[1]);

        // Without the talent nothing is replaced.
        let idle = replacement_outcome(weak, strong, 0, &context);
        assert!(!idle.active);
        assert_eq!(idle.total_chance, 0.0);

        context.unending_loot_search = 100.0;
        // Only a source whose same-tier value already meets the reference can
        // use the promotion path, so the strongest boat compares to itself.
        let equal = route_metrics(&fleet, 0, 0, Some(0), &context);
        let same_tier = replacement_outcome(&equal, strong, 0, &context);
        assert!(same_tier.threshold.same_tier_eligible);
        assert!(same_tier.uls_chance > 0.0);
        let expected = chest_tier_distribution(&fleet, Some(0)).tiers[0].probability;
        close(same_tier.exact_same_tier_chance, expected);
        close(
            same_tier.same_tier_upgrade_chance,
            expected * same_tier.uls_chance,
        );

        // A weaker source still replaces through a naturally higher tier.
        let natural = replacement_outcome(weak, strong, 0, &context);
        assert!(!natural.threshold.same_tier_eligible);
        assert_eq!(natural.uls_chance, 0.0);
        assert!(natural.threshold.required_tier.is_some());
        assert!(natural.natural_replacement_chance > 0.0);
        assert!(natural.total_chance > 0.0);
    }

    #[test]
    fn the_upgrade_chance_reads_the_talent_and_the_tier() {
        close(overflow_upgrade_chance(0, 0.0), 0.15);
        close(overflow_upgrade_chance(0, 100.0), 0.25);
        close(overflow_upgrade_chance(1, 0.0), 0.03);
        assert_eq!(overflow_upgrade_chance(4, 100.0), 0.0);
        assert_eq!(overflow_upgrade_chance(5, 100.0), 0.0);
    }

    #[test]
    fn another_island_can_never_replace_an_ordinary_chest() {
        let fleet = fleet();
        let context = RouteContext {
            unending_loot_search: 100.0,
            ..RouteContext::default()
        };
        let strong = route_metrics(&fleet, 0, 0, Some(0), &context);
        let elsewhere = route_metrics(&fleet, 1, 3, Some(1), &context);
        let threshold = replacement_threshold(&elsewhere, &strong, 0);
        assert!(!threshold.same_island);
        assert_eq!(threshold.required_tier, None);
        assert!(!threshold.same_tier_eligible);
        let outcome = replacement_outcome(&elsewhere, &strong, 0, &context);
        assert_eq!(outcome.total_chance, 0.0);
        assert!(outcome.miracle_generation_chance > 0.0);
    }

    #[test]
    fn a_hopeless_source_reports_its_shortfall() {
        let fleet = fleet();
        let context = RouteContext::default();
        let strong = route_metrics(&fleet, 0, 0, Some(0), &context);
        let weak = route_metrics(&fleet, 1, 0, Some(1), &context);
        let threshold = replacement_threshold(&weak, &strong, 4);
        assert_eq!(threshold.required_tier, None);
        assert!(threshold.shortfall > 0.0);
        assert_eq!(threshold.margin, 0.0);
    }

    fn close(actual: f64, expected: f64) {
        assert!(
            (actual - expected).abs() <= expected.abs().max(1.0) * 1e-12,
            "expected {expected}, got {actual}"
        );
    }
}
