//! The Royal Guardian overview against the upstream JS.
//!
//! Every figure in `fixtures/royal-guardian.json` was produced by
//! `script/royal-oracle.mjs` running the upstream
//! `js/stats/systems/w7/royal-guardian.js` on `fixtures/data.json`, so a drift
//! in either the port or the fixture fails here.
//!
//! The clearing rate multiplies talent 231's purchased level by the Orblet,
//! Spelunk and Jelly factors. Upstream raises that level with the all-talent
//! bonus levels, which this port does not decode yet: the assertion below names
//! the gap, and the fixture save carries none of them, so the two agree here.

use idleon_core::royal_guardian::{ClearState, RoyalGuardian};
use idleon_core::save::import;
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/royal-guardian.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/royal-guardian.json"))
        .expect("the oracle is JSON")
}

fn royal() -> RoyalGuardian {
    import(&fixture())
        .expect("the fixture imports")
        .royal
        .expect("the fixture carries the Royal Guardian rows")
}

/// Compare a figure the oracle printed, naming it when it drifts.
#[track_caller]
fn near(label: &str, got: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (got - expected).abs() <= tolerance,
        "{label}: got {got}, upstream {expected}"
    );
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

#[test]
fn the_counters_match_the_save() {
    let royal = royal();
    let oracle = oracle();
    let expected = &oracle["counters"];
    let counters = royal.counters();
    assert_eq!(
        counters.built_outposts,
        number(expected, "builtOutposts") as usize,
        "built outposts"
    );
    assert_eq!(
        counters.outposts,
        number(expected, "outposts") as usize,
        "outposts on the map"
    );
    assert_eq!(
        counters.worlds_unlocked,
        number(expected, "worldsUnlocked") as usize,
        "worlds unlocked"
    );
    near(
        "armory levels",
        counters.armory_levels,
        number(expected, "armoryLevels"),
    );
    near("units", counters.units, number(expected, "units"));
    assert_eq!(counters.purified, number(expected, "purified") as usize);
    assert_eq!(counters.support, number(expected, "support") as usize);
    assert_eq!(counters.savage, number(expected, "savage") as usize);
    assert_eq!(
        counters.militia_clearing,
        number(expected, "militiaClearing") as usize,
        "outposts militia is clearing"
    );
    near(
        "militia clears per hour",
        counters.militia_clears_per_hour,
        number(expected, "militiaClearsPerHour"),
    );
}

#[test]
fn the_clearing_rate_multiplies_the_talent_and_the_cross_system_factors() {
    let royal = royal();
    let oracle = oracle();
    let talent = &oracle["talent"];
    near(
        "talent level",
        royal.talent_level(),
        number(talent, "purchasedLevel"),
    );
    near(
        "talent multi",
        royal.talent_multi(),
        number(talent, "purchasedMulti"),
    );
    assert_eq!(
        number(talent, "allTalentLevels"),
        0.0,
        "this save's Royal Guardian carries no all-talent bonus levels, so the purchased level is \
         the whole talent; a save that carries them is a known gap (see the module docs)"
    );
    near(
        "xtra clear killz",
        royal.xtra_clear_killz(),
        number(talent, "xtraClearKillz"),
    );
    near(
        "xtra clear killz (purchased only)",
        royal.xtra_clear_killz(),
        number(talent, "xtraClearKillzPurchasedOnly"),
    );
    near(
        "big fish 6",
        royal.big_fish_bonus(6),
        number(&oracle["crossSystem"], "bigFish6"),
    );
    near(
        "jelly obstruction 2",
        royal.jelly_bonus(2),
        number(&oracle["crossSystem"], "jelly2"),
    );
}

#[test]
fn the_world_progress_matches_the_save() {
    let royal = royal();
    let oracle = oracle();
    let worlds = oracle["worlds"].as_array().expect("the worlds list");
    assert_eq!(worlds.len(), 7);
    for world in worlds {
        let index = number(world, "world") as usize;
        assert_eq!(
            royal.world_unlocked(index),
            world["unlocked"].as_bool().expect("an unlock flag"),
            "world {index} unlock"
        );
        let outposts = royal.world_outposts(index);
        assert_eq!(
            outposts.len(),
            number(world, "outposts") as usize,
            "world {index} outposts"
        );
        assert_eq!(
            outposts.iter().filter(|outpost| outpost.built).count(),
            number(world, "built") as usize,
            "world {index} built outposts"
        );
        assert_eq!(
            outposts
                .iter()
                .filter(|outpost| !outpost.built
                    && royal.clear(outpost.map).state == ClearState::Clearing)
                .count(),
            number(world, "clearing") as usize,
            "world {index} militia clearing"
        );
    }
}

#[test]
fn the_raw_clearing_values_match_the_save() {
    let royal = royal();
    let oracle = oracle();
    let rows = oracle["clearing"].as_array().expect("the clearing list");
    assert!(!rows.is_empty());
    for row in rows {
        let map = number(row, "map") as usize;
        let clear = royal.clear(map);
        assert_eq!(
            clear.state.id(),
            row["state"].as_str().expect("a state"),
            "map {map} state"
        );
        assert_eq!(
            clear.active,
            row["active"].as_bool().expect("an active flag")
        );
        near(
            &format!("map {map} progress"),
            clear.progress,
            number(row, "progress"),
        );
        near(
            &format!("map {map} requirement"),
            clear.requirement,
            number(row, "requirement"),
        );
        near(
            &format!("map {map} remaining"),
            clear.remaining,
            number(row, "remaining"),
        );
        near(
            &format!("map {map} rate"),
            clear.rate_per_hour,
            number(row, "ratePerHour"),
        );
        near(
            &format!("map {map} militia"),
            clear.assignments,
            number(row, "assignments"),
        );
        match (clear.eta_hours, row["etaHours"].as_f64()) {
            (Some(got), Some(expected)) => near(&format!("map {map} ETA"), got, expected),
            (None, None) => {}
            (got, expected) => {
                panic!("map {map} ETA does not travel: got {got:?}, upstream {expected:?}")
            }
        }
        let name = oracle["names"]
            .get(map.to_string())
            .and_then(Value::as_str)
            .expect("a name for every clearing map");
        assert_eq!(royal.outpost(map).name, name, "map {map} name");
    }
}

#[test]
fn the_built_outposts_report_their_range_and_units() {
    let royal = royal();
    let oracle = oracle();
    let rows = oracle["built"].as_array().expect("the built list");
    assert_eq!(rows.len(), 54);
    for row in rows {
        let map = number(row, "map") as usize;
        let outpost = royal.outpost(map);
        assert!(outpost.built, "map {map} is built");
        assert_eq!(
            royal.outpost_type(map),
            number(row, "type"),
            "map {map} type"
        );
        near(
            &format!("map {map} range"),
            royal.range(map),
            number(row, "range"),
        );
        near(
            &format!("map {map} units"),
            royal.total_units(map),
            number(row, "units"),
        );
        near(
            &format!("map {map} guards"),
            royal.units_by_type(map)[2],
            number(row, "guards"),
        );
        assert_eq!(
            royal.purified(map),
            row["purified"].as_bool().expect("a purified flag"),
            "map {map} purified"
        );
        assert_eq!(
            royal.glorified(map),
            row["glorified"].as_bool().expect("a glorified flag"),
            "map {map} glorified"
        );
    }
}

#[test]
fn the_range_targets_match_the_save() {
    let royal = royal();
    let oracle = oracle();
    let rows = oracle["rangeTargets"].as_array().expect("the range list");
    let mut planned = 0;
    for row in rows {
        let map = number(row, "map") as usize;
        near(
            &format!("map {map} range"),
            royal.range(map),
            number(row, "range"),
        );
        let expected = &row["target"];
        let target = royal.next_range_target(map);
        if expected.is_null() {
            assert!(
                target.is_none(),
                "map {map} reaches every node upstream reaches"
            );
            continue;
        }
        let target = target.unwrap_or_else(|| panic!("map {map} has the node upstream names"));
        near(
            &format!("map {map} target resource"),
            target.resource as f64,
            number(expected, "resource"),
        );
        near(
            &format!("map {map} target distance"),
            target.distance,
            number(expected, "distance"),
        );
        near(
            &format!("map {map} reach"),
            target.reach,
            number(expected, "reach"),
        );
        assert_eq!(
            target.plan.possible,
            expected["possible"].as_bool().expect("a plan flag"),
            "map {map} plan"
        );
        near(
            &format!("map {map} logistics level"),
            target.plan.current_level,
            number(expected, "currentLevel"),
        );
        near(
            &format!("map {map} required range"),
            target.plan.required_range,
            number(expected, "requiredRange"),
        );
        let level = expected["targetLevel"].as_f64();
        let levels = expected["levelsNeeded"].as_f64();
        assert_eq!(
            target.plan.target_level, level,
            "map {map} Advanced Logistics target level"
        );
        assert_eq!(
            target.plan.levels_needed, levels,
            "map {map} Advanced Logistics levels needed"
        );
        assert_eq!(
            target.plan.reason,
            expected["reason"].as_str(),
            "map {map} plan reason"
        );
        if target.plan.possible {
            planned += 1;
        }
    }
    assert!(planned > 0, "the fixture plans at least one range target");
}
