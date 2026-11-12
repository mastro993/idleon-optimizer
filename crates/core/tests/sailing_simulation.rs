//! The fleet pile simulation, pinned to the upstream simulator's own output.
//!
//! `script/sailing-simulation-oracle.mjs` runs `sailing-sim.js` over a
//! purpose-built save with a seeded `Math.random`, writes the configuration and
//! the result of two cases, and this test compares the Rust port with it field
//! for field. The seeded draw is the only difference from a browser run, and it
//! makes "the same inputs reproduce the same result" testable.

use idleon_core::model::Control;
use idleon_core::sailing::{
    Fleet, Goal, RouteContext, SimConfig, SimOptions, SimResult, optimize, saved, simulate,
};
use idleon_core::save::import;
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/sailing-simulation.json")).expect("oracle JSON")
}

fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number in the oracle, got {}", value[key]))
}

fn index(value: &Value, key: &str) -> usize {
    value[key]
        .as_u64()
        .unwrap_or_else(|| panic!("{key} is an index in the oracle")) as usize
}

fn close(actual: f64, expected: f64, field: &str) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{field}: expected {expected}, got {actual}"
    );
}

/// The imported account with the case's talent value.
fn imported(oracle: &Value, case: &Value) -> (Fleet, RouteContext) {
    let save = serde_json::to_string(&oracle["save"]).expect("the oracle save serialises");
    let imported = import(&save).expect("the oracle save imports");
    let mut context = imported.sailing;
    let config = &case["config"];
    context.unending_loot_search = number(config, "unendingLootSearch");
    context.minimum_tier = index(config, "minimumTier") as u32;
    (imported.fleet, context)
}

fn plan(fleet: &Fleet, context: &RouteContext, case: &Value) -> idleon_core::sailing::FleetPlan {
    let island = index(case, "island");
    let goal = Goal::from_id(case["goal"].as_str().expect("goal")).expect("a known goal");
    match case["mode"].as_str().expect("mode") {
        "saved" => saved(fleet, island, goal, context),
        "optimized" => optimize(fleet, island, goal, context),
        mode => panic!("unknown assignment mode {mode}"),
    }
}

fn config(case: &Value) -> SimConfig {
    let (fleet, context) = imported(&oracle(), case);
    let plan = plan(&fleet, &context, case);
    SimConfig::from_plan(
        &plan,
        &context,
        SimOptions {
            hours: number(case, "hours"),
            trials: index(case, "trials"),
            seed: index(case, "seed") as u64,
        },
    )
}

fn run(case: &Value) -> SimResult {
    simulate(&config(case), &mut |_| Control::Continue).expect("the simulation completes")
}

fn summary(expected: &Value, actual: (f64, f64, f64, f64), field: &str) {
    close(actual.0, number(expected, "mean"), &format!("{field}.mean"));
    close(actual.1, number(expected, "p10"), &format!("{field}.p10"));
    close(
        actual.2,
        number(expected, "median"),
        &format!("{field}.median"),
    );
    close(actual.3, number(expected, "p90"), &format!("{field}.p90"));
}

fn numbers(expected: &Value, actual: &[f64], field: &str) {
    let expected = expected.as_array().expect("an array in the oracle");
    assert_eq!(actual.len(), expected.len(), "{field}: length");
    for (index, value) in expected.iter().enumerate() {
        close(
            actual[index],
            value.as_f64().expect("a number"),
            &format!("{field}[{index}]"),
        );
    }
}

#[test]
fn the_simulation_config_matches_the_upstream_calculator() {
    let oracle = oracle();
    for case in oracle["cases"].as_array().expect("cases") {
        let expected = &case["config"];
        let config = config(case);
        let label = case["name"].as_str().expect("case name");
        assert_eq!(
            config.capacity,
            index(expected, "capacity"),
            "{label}: capacity"
        );
        assert_eq!(
            config.minimum_tier,
            index(expected, "minimumTier") as u32,
            "{label}: minimum tier"
        );
        close(
            config.unending_loot_search,
            number(expected, "unendingLootSearch"),
            &format!("{label}: unending loot search"),
        );
        close(
            config.hours,
            number(expected, "hours"),
            &format!("{label}: hours"),
        );
        assert_eq!(config.trials, index(expected, "trials"), "{label}: trials");
        assert_eq!(config.seed, index(expected, "seed") as u64, "{label}: seed");
        assert_eq!(
            config.target_count(),
            index(&case["result"]["config"], "targetCount"),
            "{label}: target count"
        );
        assert_eq!(config.island, index(case, "island"), "{label}: island");
        assert_eq!(
            config.goal,
            Goal::from_id(case["goal"].as_str().expect("goal")).expect("a known goal"),
            "{label}: goal"
        );
        assert_eq!(
            config.mode,
            match case["mode"].as_str().expect("mode") {
                "saved" => idleon_core::sailing::AssignmentMode::Saved,
                _ => idleon_core::sailing::AssignmentMode::Optimized,
            },
            "{label}: mode"
        );
        numbers(
            &expected["lootMultipliers"],
            &config.loot_multipliers,
            &format!("{label}: loot multipliers"),
        );
        numbers(
            &expected["artifactMultipliers"],
            &config.artifact_multipliers,
            &format!("{label}: artifact multipliers"),
        );
        numbers(
            &expected["upgradeChances"],
            &config.upgrade_chances,
            &format!("{label}: upgrade chances"),
        );
        let payouts = expected["payoutsByTier"].as_array().expect("payouts");
        for (tier, value) in payouts.iter().enumerate() {
            assert_eq!(
                config.payouts_by_tier[tier],
                value.as_u64().expect("a payout count") as u32,
                "{label}: payouts tier {tier}"
            );
        }
        let shares = &expected["currencyShares"];
        close(
            config.currency_shares.gold,
            number(shares, "gold"),
            &format!("{label}: gold share"),
        );
        close(
            config.currency_shares.primary,
            number(shares, "primary"),
            &format!("{label}: primary share"),
        );
        close(
            config.currency_shares.rare,
            number(shares, "rare"),
            &format!("{label}: rare share"),
        );

        let routes = expected["routes"].as_array().expect("routes");
        assert_eq!(config.routes.len(), routes.len(), "{label}: route count");
        for (route, expected) in config.routes.iter().zip(routes) {
            let field = format!("{label}: boat {}", route.boat);
            assert_eq!(route.boat, index(expected, "boatIdx"), "{field}: boat");
            let captain = expected["captainIdx"].as_i64().expect("captain");
            assert_eq!(
                route.captain.map(|captain| captain as i64),
                (captain >= 0).then_some(captain),
                "{field}: captain"
            );
            assert_eq!(
                route.island,
                index(expected, "islandIdx"),
                "{field}: island"
            );
            close(
                route.trips_per_hour,
                number(expected, "tripsPerHour"),
                &format!("{field}: trips per hour"),
            );
            close(
                route.loot_value,
                number(expected, "lootValue"),
                &format!("{field}: loot value"),
            );
            close(
                route.artifact_multiplier,
                number(expected, "artifactMultiplier"),
                &format!("{field}: artifact multiplier"),
            );
            numbers(
                &expected["tierProbabilities"],
                &route.tier_probabilities,
                &format!("{field}: tier chances"),
            );
            let sum: f64 = route.tier_probabilities.iter().sum();
            assert!(
                (sum - 1.0).abs() < 1e-12,
                "{field}: the tier chances sum to {sum}"
            );
        }
    }
}

#[test]
fn the_simulation_result_matches_the_upstream_calculator() {
    let oracle = oracle();
    for case in oracle["cases"].as_array().expect("cases") {
        let label = case["name"].as_str().expect("case name");
        let expected = &case["result"];
        let result = run(case);

        let setup = &expected["config"];
        assert_eq!(
            result.setup.island,
            index(case, "island"),
            "{label}: setup island"
        );
        assert_eq!(
            result.setup.goal,
            Goal::from_id(case["goal"].as_str().expect("goal")).expect("a known goal"),
            "{label}: setup goal"
        );
        assert_eq!(
            result.setup.mode,
            match case["mode"].as_str().expect("mode") {
                "saved" => idleon_core::sailing::AssignmentMode::Saved,
                _ => idleon_core::sailing::AssignmentMode::Optimized,
            },
            "{label}: setup mode"
        );
        close(
            result.setup.hours,
            number(setup, "hours"),
            &format!("{label}: setup hours"),
        );
        assert_eq!(
            result.setup.trials,
            index(setup, "trials"),
            "{label}: setup trials"
        );
        assert_eq!(
            result.setup.capacity,
            index(setup, "capacity"),
            "{label}: setup capacity"
        );
        assert_eq!(
            result.setup.target_count,
            index(setup, "targetCount"),
            "{label}: setup target"
        );
        assert_eq!(
            result.setup.minimum_tier,
            index(setup, "minimumTier") as u32,
            "{label}: setup minimum tier"
        );
        assert_eq!(
            result.setup.seed,
            index(case, "seed") as u64,
            "{label}: setup seed"
        );

        let events = &expected["events"];
        let actual = result.events;
        close(
            actual.generated,
            number(events, "generated"),
            &format!("{label}: generated"),
        );
        close(
            actual.accepted,
            number(events, "accepted"),
            &format!("{label}: accepted"),
        );
        close(
            actual.upgrades,
            number(events, "upgrades"),
            &format!("{label}: upgrades"),
        );
        close(
            actual.replacements,
            number(events, "replacements"),
            &format!("{label}: replacements"),
        );
        close(
            actual.filter_discards,
            number(events, "filterDiscards"),
            &format!("{label}: filter discards"),
        );
        close(
            actual.overflow_discards,
            number(events, "overflowDiscards"),
            &format!("{label}: overflow discards"),
        );
        close(
            actual.blocked,
            number(events, "blocked"),
            &format!("{label}: blocked"),
        );

        let pile = &expected["pile"];
        let count = result.pile.count;
        summary(
            &pile["count"],
            (count.mean, count.p10, count.median, count.p90),
            &format!("{label}: pile count"),
        );
        let value = result.pile.value;
        summary(
            &pile["value"],
            (value.mean, value.p10, value.median, value.p90),
            &format!("{label}: pile value"),
        );
        close(
            result.pile.average_value_per_chest,
            number(pile, "averageValuePerChest"),
            &format!("{label}: average value per chest"),
        );
        let active = result.active_collection_value;
        summary(
            &expected["activeCollectionValue"],
            (active.mean, active.p10, active.median, active.p90),
            &format!("{label}: active collection"),
        );
        close(
            result.retained_fraction,
            number(expected, "retainedFraction"),
            &format!("{label}: retained fraction"),
        );

        let slots = expected["slots"].as_array().expect("slots");
        assert_eq!(result.slots.len(), slots.len(), "{label}: slot count");
        for (actual, expected) in result.slots.iter().zip(slots) {
            let field = format!("{label}: slot {}", actual.index);
            assert_eq!(actual.index, index(expected, "index"), "{field}: index");
            close(
                actual.occupancy,
                number(expected, "occupancy"),
                &format!("{field}: occupancy"),
            );
            close(
                actual.average_tier,
                number(expected, "averageTier"),
                &format!("{field}: average tier"),
            );
            let likely = expected["mostLikelyTier"]
                .as_i64()
                .expect("most likely tier");
            assert_eq!(
                actual.most_likely_tier.map(|tier| tier as i64),
                (likely >= 0).then_some(likely),
                "{field}: most likely tier"
            );
            numbers(
                &expected["tierDistribution"],
                &actual.tier_distribution,
                &format!("{field}: tier distribution"),
            );
            close(
                actual.average_value_per_payout,
                number(expected, "averageValuePerPayout"),
                &format!("{field}: value per payout"),
            );
            close(
                actual.average_artifact_multiplier,
                number(expected, "averageArtifactMultiplier"),
                &format!("{field}: artifact multiplier"),
            );
            close(
                actual.average_payouts,
                number(expected, "averagePayouts"),
                &format!("{field}: payouts"),
            );
            close(
                actual.average_opened_value,
                number(expected, "averageOpenedValue"),
                &format!("{field}: opened value"),
            );
            numbers(
                &Value::Array(vec![
                    expected["averageCurrencyValue"]["gold"].clone(),
                    expected["averageCurrencyValue"]["primary"].clone(),
                    expected["averageCurrencyValue"]["rare"].clone(),
                ]),
                &actual.average_currency_value,
                &format!("{field}: currency values"),
            );
        }

        let tiers = expected["tiers"].as_array().expect("tiers");
        assert_eq!(result.tiers.len(), tiers.len(), "{label}: tier count");
        for (actual, expected) in result.tiers.iter().zip(tiers) {
            let field = format!("{label}: tier {}", actual.tier);
            assert_eq!(actual.tier, index(expected, "tier"), "{field}: tier");
            close(
                actual.average_count,
                number(expected, "averageCount"),
                &format!("{field}: average count"),
            );
            close(
                actual.average_value_per_payout,
                number(expected, "averageValuePerPayout"),
                &format!("{field}: value per payout"),
            );
            close(
                actual.average_opened_value,
                number(expected, "averageOpenedValue"),
                &format!("{field}: opened value"),
            );
            close(
                actual.opened_value_contribution,
                number(expected, "openedValueContribution"),
                &format!("{field}: contribution"),
            );
        }

        for (key, actual) in [
            ("gold", result.currencies.gold),
            ("primary", result.currencies.primary),
            ("rare", result.currencies.rare),
        ] {
            let expected = &expected["currencies"][key];
            let field = format!("{label}: {key}");
            summary(
                &expected["value"],
                (
                    actual.value.mean,
                    actual.value.p10,
                    actual.value.median,
                    actual.value.p90,
                ),
                &format!("{field} value"),
            );
            summary(
                &expected["payouts"],
                (
                    actual.payouts.mean,
                    actual.payouts.p10,
                    actual.payouts.median,
                    actual.payouts.p90,
                ),
                &format!("{field} payouts"),
            );
        }
    }
}

#[test]
fn the_same_inputs_reproduce_the_result() {
    let oracle = oracle();
    let first = run(&oracle["cases"][0]);
    let second = run(&oracle["cases"][0]);
    assert_eq!(first, second);

    let mut config = config(&oracle["cases"][0]);
    config.seed += 1;
    let other = simulate(&config, &mut |_| Control::Continue).expect("the simulation completes");
    assert_ne!(first, other, "a new seed moves the samples");
}

#[test]
fn a_cancelled_simulation_stores_nothing() {
    let oracle = oracle();
    let mut reports = 0;
    let result = simulate(&config(&oracle["cases"][0]), &mut |_| {
        reports += 1;
        Control::Stop
    });
    assert!(result.is_none());
    assert_eq!(reports, 1);
}

#[test]
fn the_saved_and_optimized_plans_differ_on_the_oracle_fleet() {
    let oracle = oracle();
    let case = &oracle["cases"][0];
    let (fleet, context) = imported(&oracle, case);
    let island = index(case, "island");
    let goal = Goal::from_id(case["goal"].as_str().expect("goal")).expect("a known goal");
    let optimized = optimize(&fleet, island, goal, &context);
    let current = saved(&fleet, island, goal, &context);
    assert_eq!(
        optimized.mode,
        idleon_core::sailing::AssignmentMode::Optimized
    );
    assert_eq!(current.mode, idleon_core::sailing::AssignmentMode::Saved);
    let optimized: Vec<Option<usize>> =
        optimized.routes.iter().map(|route| route.captain).collect();
    let current: Vec<Option<usize>> = current.routes.iter().map(|route| route.captain).collect();
    assert_ne!(
        optimized, current,
        "the fixture's boats hold each other's best captain"
    );
}
