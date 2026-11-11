//! Chest outcomes, payout breakdowns, near-full transitions and artifact odds,
//! pinned to the upstream calculator's own output.
//!
//! `script/sailing-oracle.mjs` runs `sailing-calc.html`'s fleet aggregation and
//! `sailing-chests.js` against the save the fixture carries and writes every
//! expected number, so this test compares the port with the source rather than
//! with hand-picked values.

use idleon_core::sailing::{
    Fleet, Goal, RouteContext, aggregate_fleet_tiers, fleet_chest_payout_breakdown,
    incomplete_artifacts, max_artifact_tier, near_full_pile, optimize, route_metrics,
};
use idleon_core::save::import;
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/sailing-outcomes.json")).expect("oracle JSON")
}

fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number in the oracle"))
}

/// The imported account, with the oracle's talent value and no other bonuses.
fn imported(oracle: &Value) -> (Fleet, RouteContext) {
    let save = serde_json::to_string(&oracle["save"]).expect("the oracle save serialises");
    let imported = import(&save).expect("the oracle save imports");
    let mut context = imported.sailing;
    context.unending_loot_search = number(oracle, "unendingLootSearch");
    (imported.fleet, context)
}

fn close(actual: f64, expected: f64, field: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{field}: expected {expected}, got {actual}"
    );
}

#[test]
fn the_route_outcomes_match_the_upstream_calculator() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let island = oracle["island"].as_u64().expect("island") as usize;
    let plan = optimize(&fleet, island, Goal::Treasure, &context);
    let routes = oracle["routes"].as_array().expect("route rows");
    assert_eq!(routes.len(), plan.routes.len());

    for expected in routes {
        let boat = expected["boat"].as_u64().expect("boat") as usize;
        let route = plan
            .routes
            .iter()
            .find(|route| route.boat == boat)
            .expect("the plan has this boat");
        let field = format!("boat {boat}");
        assert_eq!(
            route.captain,
            Some(expected["captain"].as_u64().expect("captain") as usize)
        );
        assert_eq!(route.island, island);
        close(
            route.generated_chests_per_hour,
            number(expected, "generatedChestsPerHour"),
            &field,
        );
        close(
            route.kept_chests_per_hour,
            number(expected, "keptChestsPerHour"),
            &field,
        );
        close(route.loot.value, number(expected, "lootValue"), &field);
        close(
            route.treasure_per_hour,
            number(expected, "treasurePerHour"),
            &field,
        );
        close(
            route.artifact_finds_per_hour,
            number(expected, "artifactFindsPerHour"),
            &field,
        );
        close(
            route.artifact_chance_per_trip,
            number(expected, "artifactChancePerTrip"),
            &field,
        );
        close(
            route.artifact_base.value,
            number(expected, "artifactBase"),
            &field,
        );
        close(
            route.rarity.rarity_scale,
            number(expected, "rarityScale"),
            &field,
        );
        let shares = expected["treasureShares"].as_object().expect("shares");
        close(
            route.treasure_shares.gold,
            shares["gold"].as_f64().expect("gold"),
            &field,
        );
        close(
            route.treasure_shares.primary,
            shares["primary"].as_f64().expect("primary"),
            &field,
        );
        close(
            route.treasure_shares.rare,
            shares["rare"].as_f64().expect("rare"),
            &field,
        );
    }
}

#[test]
fn the_fleet_tiers_match_upstream_and_sum_to_one() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let island = oracle["island"].as_u64().expect("island") as usize;
    let plan = optimize(&fleet, island, Goal::Treasure, &context);
    let outcomes = aggregate_fleet_tiers(&plan.routes);
    let expected = &oracle["fleet"];
    close(
        outcomes.generated_per_hour,
        number(expected, "generatedPerHour"),
        "generated",
    );

    let rows = expected["tiers"].as_array().expect("tier rows");
    assert_eq!(rows.len(), outcomes.tiers.len());
    for (row, expected) in outcomes.tiers.iter().zip(rows) {
        let field = format!("tier {}", row.tier);
        close(row.probability, number(expected, "probability"), &field);
        close(row.cumulative, number(expected, "cumulative"), &field);
        close(
            row.expected_per_hour,
            number(expected, "expectedPerHour"),
            &field,
        );
    }

    let sum: f64 = outcomes.tiers.iter().map(|tier| tier.probability).sum();
    close(sum, 1.0, "chance sum");
    let cumulative: f64 = outcomes
        .tiers
        .iter()
        .map(|tier| tier.expected_per_hour)
        .sum();
    close(cumulative, outcomes.generated_per_hour, "arrival sum");
}

#[test]
fn the_payout_breakdown_matches_upstream() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let island = oracle["island"].as_u64().expect("island") as usize;
    let plan = optimize(&fleet, island, Goal::Treasure, &context);

    for expected in oracle["payouts"].as_array().expect("payout rows") {
        let tier = expected["tier"].as_u64().expect("tier") as usize;
        let breakdown = fleet_chest_payout_breakdown(&plan.routes, tier, &context);
        let field = format!("tier {tier}");
        assert_eq!(breakdown.tier, tier);
        close(breakdown.multiplier, number(expected, "multiplier"), &field);
        assert_eq!(
            breakdown.payouts as f64,
            number(expected, "payouts"),
            "{field} payouts"
        );
        close(breakdown.chest_rate, number(expected, "chestRate"), &field);
        close(
            breakdown.average_stored_value,
            number(expected, "averageStoredValue"),
            &field,
        );
        close(
            breakdown.total_value_per_chest,
            number(expected, "totalValuePerChest"),
            &field,
        );

        let currencies = expected["currencies"].as_object().expect("currencies");
        for (key, actual) in [
            ("gold", breakdown.currencies.gold),
            ("primary", breakdown.currencies.primary),
            ("rare", breakdown.currencies.rare),
        ] {
            let expected = &currencies[key];
            let field = format!("tier {tier} {key}");
            close(
                actual.chance_per_payout,
                number(expected, "chancePerPayout"),
                &field,
            );
            close(
                actual.expected_payouts_per_chest,
                number(expected, "expectedPayoutsPerChest"),
                &field,
            );
            close(
                actual.expected_value_per_chest,
                number(expected, "expectedValuePerChest"),
                &field,
            );
            close(
                actual.expected_value_per_hour,
                number(expected, "expectedValuePerHour"),
                &field,
            );
        }
    }
}

#[test]
fn the_near_full_transitions_match_upstream() {
    let oracle = oracle();
    let (_, context) = imported(&oracle);
    let rows = near_full_pile(&context);
    let expected = oracle["nearFull"].as_array().expect("near-full rows");
    assert_eq!(rows.len(), expected.len());
    for (row, expected) in rows.iter().zip(expected) {
        let field = format!("tier {}", row.tier);
        assert_eq!(row.tier as f64, number(expected, "tier"));
        close(row.chance, number(expected, "chance"), &field);
        close(
            row.loot_multiplier,
            number(expected, "lootMultiplier"),
            &field,
        );
        close(
            row.next_loot_multiplier,
            number(expected, "nextLootMultiplier"),
            &field,
        );
        close(row.value_ratio, number(expected, "valueRatio"), &field);
        close(
            row.artifact_multiplier,
            number(expected, "artifactMultiplier"),
            &field,
        );
    }
}

#[test]
fn the_artifact_odds_match_upstream() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let island = oracle["island"].as_u64().expect("island") as usize;
    let plan = optimize(&fleet, island, Goal::Treasure, &context);
    let route = plan
        .routes
        .iter()
        .find(|route| route.boat as f64 == number(&oracle, "boat"))
        .expect("the plan has the selected boat");

    assert_eq!(
        f64::from(max_artifact_tier(context.unlocks)),
        number(&oracle, "maxArtifactTier")
    );

    // The island ranking: every unlocked island at the selected boat's captain,
    // best find rate first.
    let mut ranked: Vec<(usize, usize, f64, f64, f64)> = (0..fleet.available_islands())
        .map(|island| {
            let route = route_metrics(&fleet, route.boat, island, route.captain, &context);
            (
                island,
                incomplete_artifacts(&fleet, island, context.unlocks),
                route.timing.trip_seconds,
                route.artifact_chance_per_trip,
                route.artifact_finds_per_hour,
            )
        })
        .collect();
    ranked.sort_by(|left, right| right.4.total_cmp(&left.4));
    let expected = oracle["islands"].as_array().expect("island rows");
    assert_eq!(ranked.len(), expected.len());
    for (actual, expected) in ranked.iter().zip(expected) {
        let island = expected["island"].as_u64().expect("island") as usize;
        assert_eq!(actual.0, island);
        assert_eq!(
            actual.1 as f64,
            number(expected, "incomplete"),
            "island {island} incomplete"
        );
        close(actual.2, number(expected, "tripSeconds"), "trip seconds");
        close(
            actual.3,
            number(expected, "artifactChancePerTrip"),
            "chance per chest",
        );
        close(
            actual.4,
            number(expected, "artifactFindsPerHour"),
            "finds per hour",
        );
    }

    // The selected island's artifacts, weighted over the kept tiers.
    let expected = oracle["artifacts"].as_array().expect("artifact rows");
    assert_eq!(route.artifact_outcomes.len(), expected.len());
    for outcome in &route.artifact_outcomes {
        let expected = expected
            .iter()
            .find(|row| row["artifact"].as_u64() == Some(outcome.artifact as u64))
            .unwrap_or_else(|| panic!("artifact {} is in the oracle", outcome.artifact));
        let field = format!("artifact {}", outcome.artifact);
        assert_eq!(
            outcome.name,
            expected["name"].as_str().expect("name"),
            "{field} name"
        );
        assert_eq!(
            outcome.current_tier as f64,
            number(expected, "currentTier"),
            "{field} tier"
        );
        assert_eq!(
            outcome.unlocked,
            expected["unlocked"].as_bool().expect("unlocked"),
            "{field} unlocked"
        );
        match expected["denominator"].as_f64() {
            Some(denominator) => close(outcome.denominator, denominator, &field),
            None => assert!(
                !outcome.denominator.is_finite(),
                "{field}: the artifact never drops"
            ),
        }
        close(outcome.chance_per_trip, number(expected, "chance"), &field);
    }
}
