//! Ship breakpoints, captain quality and the replacement matrix, pinned to the
//! upstream calculator's own output.
//!
//! `script/sailing-oracle.mjs` runs `sailing-chests.js` against the save the
//! fixture carries and writes every expected number, so this test compares the
//! port with the source rather than with hand-picked values.

mod common;

use common::{close, imported, number};
use idleon_sailing::{
    AssignmentMode, Currency, Goal, UpgradeStat, currency, level_time, optimize, ranked,
    replacement_outcome, roll_quality, saved, strongest, upgrade_breakpoint,
};
use serde_json::Value;

fn oracle() -> Value {
    common::oracle("fixtures/sailing-upgrades.json")
}

fn stat(value: &Value) -> UpgradeStat {
    match value["stat"].as_str() {
        Some("loot") => UpgradeStat::Loot,
        Some("speed") => UpgradeStat::Speed,
        other => panic!("unknown stat {other:?}"),
    }
}

#[test]
fn the_breakpoints_match_the_upstream_calculator() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let breakpoints = oracle["breakpoints"].as_array().expect("breakpoint rows");
    assert_eq!(
        breakpoints.len(),
        fleet.boats.len() * UpgradeStat::ALL.len()
    );

    for expected in breakpoints {
        let boat = expected["boat"].as_u64().expect("boat") as usize;
        let stat = stat(expected);
        let kind = expected["type"].as_u64().expect("currency type") as usize;
        let island = currency(kind).island.unwrap_or(0);
        let plan = optimize(&fleet, island, Goal::Treasure, &context);
        let breakpoint = upgrade_breakpoint(&fleet, boat, stat, &plan.routes, &context);
        let field = format!("boat {boat} {}", stat.label());

        assert_eq!(
            breakpoint.level as f64,
            number(expected, "level"),
            "{field}"
        );
        assert_eq!(
            breakpoint.interval as f64,
            number(expected, "interval"),
            "{field}"
        );
        assert_eq!(
            breakpoint.target_level as f64,
            number(expected, "targetLevel"),
            "{field}"
        );
        assert_eq!(
            breakpoint.levels as f64,
            number(expected, "levels"),
            "{field}"
        );
        assert_eq!(
            breakpoint.currency.kind as f64,
            number(expected, "type"),
            "{field}"
        );
        close(
            breakpoint.current_base,
            number(expected, "currentBase"),
            &field,
        );
        close(breakpoint.next_base, number(expected, "nextBase"), &field);
        close(
            breakpoint.target_base,
            number(expected, "targetBase"),
            &field,
        );
        close(
            breakpoint.next_base_gain_pct,
            number(expected, "nextBaseGainPct"),
            &field,
        );
        close(
            breakpoint.base_gain_pct,
            number(expected, "baseGainPct"),
            &field,
        );
        close(
            breakpoint.threshold_marginal_pct,
            number(expected, "thresholdMarginalPct"),
            &field,
        );
        close(breakpoint.total_cost, number(expected, "totalCost"), &field);
        close(breakpoint.balance, number(expected, "balance"), &field);
        close(breakpoint.remaining, number(expected, "remaining"), &field);
        close(
            breakpoint.initial_rate,
            number(expected, "initialRate"),
            &field,
        );
        close(breakpoint.final_rate, number(expected, "finalRate"), &field);
        close(breakpoint.hours, number(expected, "hours"), &field);
        close(
            breakpoint.treasure_gain_pct,
            number(expected, "treasureGainPct"),
            &field,
        );
        close(
            breakpoint.chest_gain_pct,
            number(expected, "chestGainPct"),
            &field,
        );
        close(
            breakpoint.artifact_gain_pct,
            number(expected, "artifactGainPct"),
            &field,
        );
        assert_eq!(
            breakpoint.speed_capped,
            expected["speedCapped"].as_bool().expect("speedCapped"),
            "{field}"
        );
        assert_eq!(
            breakpoint.davey_unlocked,
            expected["daveyUnlocked"].as_bool().expect("daveyUnlocked"),
            "{field}"
        );
        assert_eq!(
            breakpoint.davey_active,
            expected["daveyActive"].as_bool().expect("daveyActive"),
            "{field}"
        );
        assert_eq!(
            breakpoint.davey_level.map(f64::from),
            expected["daveyLevel"].as_f64(),
            "{field}"
        );
    }
}

#[test]
fn captain_quality_and_leveling_match_the_upstream_calculator() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let captains = oracle["captains"].as_array().expect("captain rows");
    assert_eq!(captains.len(), fleet.captains.len());

    for expected in captains {
        let index = expected["captain"].as_u64().expect("captain") as usize;
        let entry = &fleet.captains[index];
        assert_eq!(entry.level as f64, number(expected, "level"));
        for (slot, roll) in expected["slots"]
            .as_array()
            .expect("two slots")
            .iter()
            .enumerate()
        {
            let quality = roll_quality(entry, slot);
            assert_eq!(
                f64::from(quality.bonus_type),
                number(roll, "bonusType"),
                "captain {index} slot {slot}"
            );
            close(quality.roll, number(roll, "roll"), "roll");
            close(quality.min, number(roll, "min"), "min");
            close(quality.max, number(roll, "max"), "max");
            close(quality.quality, number(roll, "quality"), "quality");
        }

        // A captain with no boat has no level time, which the section flags.
        let Some(boat) = expected["boat"].as_u64().map(|boat| boat as usize) else {
            continue;
        };
        let time = level_time(&fleet, index, boat, 0, &context);
        close(time.progress, number(expected, "progress"), "progress");
        close(
            time.requirement,
            number(expected, "requirement"),
            "requirement",
        );
        close(time.remaining, number(expected, "remaining"), "remaining");
        close(
            time.exp_per_trip,
            number(expected, "expPerTrip"),
            "expPerTrip",
        );
        close(
            time.exp_per_hour,
            number(expected, "expPerHour"),
            "expPerHour",
        );
        close(time.hours, number(expected, "hours"), "hours");
    }
}

#[test]
fn the_replacement_matrix_matches_the_upstream_calculator() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let plan = optimize(&fleet, 0, Goal::Treasure, &context);
    let reference = strongest(&plan.routes).expect("a strongest source");
    let cells = oracle["replacement"].as_array().expect("replacement rows");
    assert_eq!(cells.len(), plan.routes.len() * 5);

    for expected in cells {
        let boat = expected["boat"].as_u64().expect("boat") as usize;
        let tier = expected["tier"].as_u64().expect("tier") as u32;
        assert_eq!(
            reference.boat as f64,
            number(expected, "strongestBoat"),
            "the matrix reference is the strongest boat"
        );
        let route = plan
            .routes
            .iter()
            .find(|route| route.boat == boat)
            .expect("the plan sails every boat");
        close(route.loot.value, number(expected, "baseValue"), "baseValue");
        let outcome = replacement_outcome(route, reference, tier, &context);
        let field = format!("boat {boat} tier {tier}");

        assert_eq!(
            outcome.threshold.same_island,
            expected["sameIsland"].as_bool().expect("sameIsland"),
            "{field}"
        );
        assert_eq!(
            outcome.threshold.same_tier_eligible,
            expected["sameTierEligible"]
                .as_bool()
                .expect("sameTierEligible"),
            "{field}"
        );
        close(
            outcome.threshold.reference_value,
            number(expected, "referenceValue"),
            &field,
        );
        assert_eq!(
            outcome.threshold.required_tier.map(f64::from),
            expected["requiredTier"].as_f64(),
            "{field}"
        );
        close(
            outcome.threshold.replacement_value,
            number(expected, "replacementValue"),
            &field,
        );
        close(outcome.threshold.margin, number(expected, "margin"), &field);
        close(
            outcome.threshold.max_value,
            number(expected, "maxValue"),
            &field,
        );
        close(
            outcome.threshold.shortfall,
            number(expected, "shortfall"),
            &field,
        );
        close(
            outcome.exact_same_tier_chance,
            number(expected, "exactSameTierChance"),
            &field,
        );
        close(outcome.uls_chance, number(expected, "ulsChance"), &field);
        close(
            outcome.same_tier_upgrade_chance,
            number(expected, "sameTierUpgradeChance"),
            &field,
        );
        close(
            outcome.natural_replacement_chance,
            number(expected, "naturalReplacementChance"),
            &field,
        );
        close(
            outcome.miracle_generation_chance,
            number(expected, "miracleGenerationChance"),
            &field,
        );
        close(
            outcome.miracle_generation_rate_per_hour,
            number(expected, "miracleGenerationRatePerHour"),
            &field,
        );
        close(
            outcome.total_chance,
            number(expected, "totalChance"),
            &field,
        );
        close(
            outcome.total_rate_per_hour,
            number(expected, "totalRatePerHour"),
            &field,
        );
        assert!(outcome.active, "{field}: Unending Loot Search is active");
    }
}

#[test]
fn the_replacement_ranking_does_not_depend_on_the_input_order() {
    let oracle = oracle();
    let (fleet, context) = imported(&oracle);
    let plan = optimize(&fleet, 0, Goal::Treasure, &context);
    let order: Vec<usize> = ranked(&plan.routes)
        .iter()
        .map(|route| route.boat)
        .collect();
    let mut reversed = plan.routes.clone();
    reversed.reverse();
    let again: Vec<usize> = ranked(&reversed).iter().map(|route| route.boat).collect();
    assert_eq!(order, again);
    assert_eq!(
        order,
        (0..plan.routes.len()).collect::<Vec<usize>>(),
        "boat 0 holds the strongest source on the oracle account"
    );
}

#[test]
fn a_missing_captain_leaves_the_boat_uncrewed() {
    let oracle = oracle();
    let (mut fleet, context) = imported(&oracle);
    // The save's second boat points at a captain row that is not in the save.
    fleet.boats[1].captain = Some(fleet.captains.len() + 4);
    let saved_plan = saved(&fleet, 0, Goal::Treasure, &context);
    assert_eq!(saved_plan.mode, AssignmentMode::Saved);
    assert!(
        saved_plan.routes.iter().all(|route| route.boat != 1),
        "the save's pairings skip a boat without a captain"
    );
    let optimized = optimize(&fleet, 0, Goal::Treasure, &context);
    assert!(
        optimized.routes.iter().any(|route| route.boat == 1),
        "the optimizer still sails the boat"
    );
}

#[test]
fn the_currency_names_cover_gold_and_island_shares() {
    let gold: Currency = currency(0);
    assert_eq!(gold.name(), "Gold Bars");
    assert_eq!(gold.island, None);
    assert_eq!(currency(1).name(), "Island 1 Primary");
    assert_eq!(currency(2).name(), "Island 1 Rare");
    assert_eq!(currency(3).name(), "Island 2 Primary");
    assert_eq!(currency(4).name(), "Island 2 Rare");
}
