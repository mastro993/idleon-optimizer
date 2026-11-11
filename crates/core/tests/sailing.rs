//! Sailing fleet assignment: save decoding, route formulas and the optimizer.
//!
//! The expected route numbers were produced by the upstream
//! `sailing-chests.js` against the same purpose-built save, so this test pins
//! the port to the source rather than to the fixture account.

use idleon_core::sailing::{
    AssignmentMode, Fleet, Goal, RouteContext, chest_tier_distribution, compare,
    island_cloud_required, island_count, optimize, route_metrics, saved,
};
use idleon_core::save::import;
use serde_json::json;

/// A small save with three boats, three captains and no account bonuses.
fn save() -> String {
    let mut research = vec![json!([]); 48];
    research[0] = json!(vec![0.0; 240]);
    let mut lv0 = vec![0.0; 21];
    lv0[0] = 100.0;
    lv0[13] = 10.0;
    let sailing = json!([vec![-1.0; 20], vec![0.0; 31], [2.0, 2.0], vec![0.0; 41],]);
    json!({
        "Research": json!(research).to_string(),
        "Lv0_0": json!(lv0).to_string(),
        "OptLacc": json!(vec![0.0; 128]).to_string(),
        // Boat 0 and 1 have each other's best captain, so the saved fleet is
        // not the optimum.
        "Boats": json!([
            [1, 0, 0, 100, 0, 100, 0],
            [0, 1, 0, 80, 0, 80, 0],
            [2, 0, 0, 60, 0, 60, 0],
        ])
        .to_string(),
        "Captains": json!([
            [3, 0, 1, 10, 0, 20, 10],
            [6, 3, 4, 20, 0, 30, 15],
            [2, 1, 0, 5, 0, 10, 10],
        ])
        .to_string(),
        "Sailing": sailing.to_string(),
    })
    .to_string()
}

fn imported() -> (Fleet, RouteContext) {
    let imported = import(&save()).expect("the purpose-built save imports");
    (imported.fleet, imported.sailing)
}

fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn the_save_decodes_the_active_fleet_and_captains() {
    let (fleet, context) = imported();
    assert_eq!(fleet.boats.len(), 3);
    assert_eq!(fleet.captains.len(), 3);
    assert_eq!(fleet.sailing_level, 10);
    assert_eq!(fleet.artifact_tiers.len(), 41);
    assert_eq!(fleet.boats[0].captain, Some(1));
    assert_eq!(fleet.boats[1].captain, Some(0));
    assert_eq!(fleet.boats[0].loot_level, 100);
    assert_eq!(fleet.captains[1].bonus_types, [3, 4]);
    assert_eq!(context.minimum_tier, 0);
    assert_eq!(context.travel.minutes(), 120.0);
}

#[test]
fn the_route_matches_upstream() {
    let (fleet, context) = imported();
    let first = route_metrics(&fleet, 0, 0, Some(0), &context);
    close(first.timing.trip_seconds, 7200.0);
    close(first.timing.trips_per_hour, 0.5);
    close(first.loot.base, 14605.0);
    close(first.loot.value, 36512.5);
    close(first.rarity.rarity_scale, 1.0);
    close(first.artifact_base.value, 1.25);
    close(first.treasure_per_hour, 34699.651664169374);
    close(first.artifact_finds_per_hour, 0.20828596550617087);

    let second = route_metrics(&fleet, 1, 1, Some(1), &context);
    close(second.loot.base, 8165.0);
    close(second.loot.value, 10206.25);
    close(second.rarity.rarity_scale, 2.5);
    close(second.artifact_base.value, 7.25);
    close(second.treasure_per_hour, 16594.08482437033);
    close(second.artifact_finds_per_hour, 0.40144152717013887);
}

#[test]
fn chest_tier_chances_sum_to_one() {
    let (fleet, _) = imported();
    for captain in [None, Some(0), Some(1), Some(2)] {
        let distribution = chest_tier_distribution(&fleet, captain);
        let sum: f64 = distribution.tiers.iter().map(|tier| tier.probability).sum();
        close(sum, 1.0);
        assert!(
            distribution
                .tiers
                .iter()
                .all(|tier| tier.probability >= 0.0)
        );
    }
}

#[test]
fn the_optimizer_beats_the_saved_fleet_and_respects_one_captain_per_boat() {
    let (fleet, context) = imported();
    let comparison = compare(&fleet, 0, Goal::Treasure, &context);
    assert!(comparison.optimized_wins());
    assert!(comparison.gain_pct() > 0.0);
    close(comparison.saved.score(), 54542.937169287536);
    close(comparison.optimized.score(), 56755.278718119254);

    let optimized = optimize(&fleet, 0, Goal::Treasure, &context);
    let mut captains: Vec<usize> = optimized
        .routes
        .iter()
        .filter_map(|route| route.captain)
        .collect();
    captains.sort_unstable();
    captains.dedup();
    assert_eq!(captains.len(), optimized.routes.len());
    assert_eq!(optimized.mode, AssignmentMode::Optimized);
    assert_eq!(optimized.routes[0].captain, Some(0));
    assert_eq!(optimized.routes[1].captain, Some(1));
}

#[test]
fn every_goal_prefers_the_optimized_plan() {
    let (fleet, context) = imported();
    for goal in Goal::ALL {
        let comparison = compare(&fleet, 0, goal, &context);
        assert!(
            comparison.optimized.score() + 1e-9 >= comparison.saved.score(),
            "{} loses to the save",
            goal.id()
        );
    }
}

#[test]
fn more_boats_than_captains_still_assigns_each_captain_once() {
    let (mut fleet, context) = imported();
    fleet.captains.truncate(1);
    let plan = optimize(&fleet, 0, Goal::Treasure, &context);
    assert_eq!(plan.routes.len(), fleet.boats.len());
    let captains: Vec<Option<usize>> = plan.routes.iter().map(|route| route.captain).collect();
    assert_eq!(
        captains.iter().filter(|captain| captain.is_some()).count(),
        1
    );
    assert_eq!(
        captains.iter().filter(|captain| captain.is_none()).count(),
        2
    );
}

#[test]
fn locked_islands_stay_out_of_the_selector() {
    let (mut fleet, _) = imported();
    assert_eq!(fleet.available_islands(), island_count());
    fleet.island_progress = vec![0.0; island_count()];
    assert_eq!(fleet.available_islands(), 1);
    fleet.island_progress[1] = -1.0;
    assert_eq!(fleet.available_islands(), 2);
    fleet.island_progress[1] = island_cloud_required(1);
    assert_eq!(fleet.available_islands(), 2);
}

#[test]
fn the_saved_plan_keeps_the_save_pairings() {
    let (fleet, context) = imported();
    let plan = saved(&fleet, 0, Goal::Treasure, &context);
    assert_eq!(plan.mode, AssignmentMode::Saved);
    assert_eq!(plan.routes[0].captain, Some(1));
    assert_eq!(plan.routes[1].captain, Some(0));
    assert_eq!(plan.routes[2].captain, Some(2));
}

#[test]
fn route_switching_changes_the_route() {
    let (fleet, context) = imported();
    assert!(island_count() > 1);
    let first = route_metrics(&fleet, 0, 0, Some(0), &context);
    let far = route_metrics(&fleet, 0, 4, Some(0), &context);
    assert!(far.timing.distance > first.timing.distance);
    assert!(far.artifact_finds_per_hour < first.artifact_finds_per_hour);
}

#[test]
fn a_save_without_sailing_fields_imports_an_empty_fleet_silently() {
    let save = json!({
        "gridLevels": vec![0.0; 240],
        "Lv0_0": json!(vec![0.0; 21]).to_string(),
    })
    .to_string();
    let imported = import(&save).expect("the research-only save imports");
    assert!(imported.fleet.boats.is_empty());
    assert!(imported.fleet.captains.is_empty());
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("Boats") || warning.contains("Captains"))
    );
}
