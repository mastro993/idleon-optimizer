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

use idleon_import::import;
use idleon_royal_guardian::{ClearState, RoyalGuardian};
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
        royal.xtra_clear_level(),
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

// ----- RG-02: the collection rate, the map, the nodes and the currencies -----

use idleon_royal_guardian::{CollectionInputs, Endpoint, ResourceStream};

/// The collection inputs as upstream reads them, so a rate can be compared once
/// the port's two unread sources are handed in.
fn filled(royal: &RoyalGuardian) -> CollectionInputs {
    let oracle = oracle();
    let inputs = &oracle["collection"]["inputs"];
    let default = royal.collection_inputs();
    CollectionInputs {
        char_level: number(inputs, "charLevel"),
        talent_225: number(inputs, "talent225"),
        talent_226: number(inputs, "talent226"),
        talent_230: number(inputs, "talent230Multi"),
        sushi_60: number(inputs, "sushi60"),
        companion_141: number(inputs, "companion141"),
        arcade_70: number(inputs, "arcade70"),
        alchemy_w14: number(inputs, "alchemyW14"),
        zenith_10: number(inputs, "zenith10"),
        missing: default.missing,
    }
}

#[test]
fn the_collection_inputs_the_port_reads_match_the_save() {
    let royal = royal();
    let oracle = oracle();
    let inputs = &oracle["collection"]["inputs"];
    let read = royal.collection_inputs();
    near(
        "character level",
        read.char_level,
        number(inputs, "charLevel"),
    );
    near("talent 225", read.talent_225, number(inputs, "talent225"));
    near("talent 226", read.talent_226, number(inputs, "talent226"));
    near("sushi 60", read.sushi_60, number(inputs, "sushi60"));
    near("arcade 70", read.arcade_70, number(inputs, "arcade70"));
    near("zenith 10", read.zenith_10, number(inputs, "zenith10"));
    // The three collection talents, at the purchased level the port reads. The
    // all-talent bonus levels upstream adds on top are a known gap, so the
    // oracle prints the purchased-only figures beside the effective ones.
    let talents = &oracle["collection"]["talents"];
    near("talent 225", read.talent_225, number(talents, "225"));
    near("talent 226", read.talent_226, number(talents, "226"));
    near("talent 230", read.talent_230, number(talents, "230"));
    // The two sources this port does not read, named rather than guessed.
    assert_eq!(read.alchemy_w14, 0.0);
    assert_eq!(read.companion_141, 0.0);
    let missing = &read.missing;
    assert!(
        missing.iter().any(|source| source.contains("W14")),
        "{missing:?}"
    );
    assert!(
        missing.iter().any(|source| source.contains("Companion")),
        "{missing:?}"
    );
}

#[test]
fn the_global_collection_rate_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let collection = &oracle["collection"];
    let filled = royal.global_collection_rate_with(&filled(&royal));
    near("global rate", filled.value, number(collection, "global"));
    let factors = &collection["factors"];
    for (name, expected) in factors.as_object().expect("the factor map") {
        let found = filled
            .factors
            .iter()
            .find(|factor| factor.name == name)
            .map(|factor| factor.value)
            .unwrap_or(f64::NAN);
        near(
            &format!("factor {name}"),
            found,
            expected.as_f64().expect("a number"),
        );
    }
    // What the section shows: the purchased-only talents with the alchemy bubble
    // and the companion roster at one. The oracle folds the same three gaps into
    // its figure, so a drift in any of them shows up here.
    near(
        "global rate with the port's gaps",
        royal.global_collection_rate().value,
        number(collection, "globalReadOnly"),
    );
    near(
        "support collection",
        royal.support_collection(),
        number(collection, "supportCollection"),
    );
    near(
        "savage collection",
        royal.savage_collection(),
        number(collection, "savageCollection"),
    );
    let stats = royal.total_stats();
    let expected = collection["totalStats"].as_array().expect("five figures");
    near(
        "total grades",
        stats.grades,
        expected[0].as_f64().expect("a number"),
    );
    near(
        "total levels",
        stats.levels,
        expected[1].as_f64().expect("a number"),
    );
    near(
        "purified outposts",
        stats.purified,
        expected[2].as_f64().expect("a number"),
    );
    near(
        "currency logs",
        stats.log_currencies,
        expected[3].as_f64().expect("a number"),
    );
    near(
        "built outposts",
        stats.outposts,
        expected[4].as_f64().expect("a number"),
    );
}

#[test]
fn every_outpost_on_the_map_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let filled = filled(&royal);
    let rows = oracle["outposts"].as_array().expect("the outpost list");
    assert!(!rows.is_empty());
    let mut built = 0;
    for row in rows {
        let map = number(row, "map") as usize;
        let outpost = royal.outpost(map);
        assert!(outpost.eligible, "map {map} carries an outpost");
        assert_eq!(
            outpost.built,
            row["built"].as_bool().expect("a built flag"),
            "map {map} built"
        );
        assert_eq!(
            outpost.world,
            number(row, "world") as usize,
            "map {map} world"
        );
        let (x, y) = RoyalGuardian::outpost_position(map).expect("a map anchor");
        near(&format!("map {map} x"), x, number(row, "x"));
        near(&format!("map {map} y"), y, number(row, "y"));
        assert_eq!(
            royal.outpost_type(map),
            number(row, "type"),
            "map {map} type"
        );
        assert_eq!(
            royal.outpost_supports(map),
            row["supports"].as_bool().expect("a supports flag"),
            "map {map} supports"
        );
        let connections = row["connections"].as_array().expect("two connections");
        assert_eq!(connections.len(), 2);
        let saved = royal.outpost_connections(map);
        for (connection, expected) in saved.iter().zip(connections) {
            assert_eq!(
                connection.endpoint.kind(),
                expected["kind"].as_str().expect("a kind"),
                "map {map} slot {} endpoint kind",
                connection.slot
            );
            match connection.endpoint {
                Endpoint::Empty => assert!(expected["endpoint"].is_null()),
                Endpoint::Resource(index) | Endpoint::Map(index) => near(
                    &format!("map {map} slot {} endpoint", connection.slot),
                    index as f64,
                    number(expected, "endpoint"),
                ),
            }
            near(
                &format!("map {map} slot {} raw", connection.slot),
                connection.raw.unwrap_or(-1.0),
                expected["raw"].as_f64().unwrap_or(-1.0),
            );
        }
        if !outpost.built {
            assert!(row["range"].is_null(), "map {map} reports no range");
            continue;
        }
        built += 1;
        near(
            &format!("map {map} range"),
            royal.range(map),
            number(row, "range"),
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
        assert_eq!(
            royal.support_count(map),
            number(row, "supportCount") as usize,
            "map {map} support links"
        );
        let links = row["supportLinks"].as_array().expect("the support links");
        let saved: Vec<(usize, usize)> = royal
            .support_links(map)
            .into_iter()
            .map(|link| (link.map, link.slot))
            .collect();
        let expected: Vec<(usize, usize)> = links
            .iter()
            .map(|link| (number(link, "map") as usize, number(link, "slot") as usize))
            .collect();
        assert_eq!(saved, expected, "map {map} support link sources");
        let units = row["units"].as_array().expect("four totals");
        for (index, total) in royal.units_by_type(map).into_iter().enumerate() {
            near(
                &format!("map {map} unit type {index}"),
                total,
                units[index].as_f64().expect("a number"),
            );
        }
        near(
            &format!("map {map} collection rate"),
            royal.outpost_collection_rate_with(map, &filled).value,
            number(row, "collectionRate"),
        );
        let resources = row["resourceRate"]
            .as_array()
            .expect("the reachable resources");
        let saved = royal.reachable_resources(map);
        assert_eq!(saved.len(), resources.len(), "map {map} world nodes");
        for (node, expected) in saved.iter().zip(resources) {
            assert_eq!(node.resource, number(expected, "resource") as usize);
            near(
                &format!("map {map} resource {} distance", node.resource),
                node.distance,
                number(expected, "distance"),
            );
            assert_eq!(
                node.reachable,
                expected["reachable"].as_bool().expect("a reachable flag"),
                "map {map} resource {} reachable",
                node.resource
            );
        }
        let targets = row["supportTargets"]
            .as_array()
            .expect("the reachable support targets");
        let saved: Vec<usize> = royal
            .support_targets(map)
            .into_iter()
            .filter(|target| target.reachable)
            .map(|target| target.map)
            .collect();
        let expected: Vec<usize> = targets
            .iter()
            .map(|target| target.as_f64().expect("a map index") as usize)
            .collect();
        assert_eq!(saved, expected, "map {map} support targets");
    }
    assert!(built > 0, "the fixture carries built outposts");
}

#[test]
fn the_resource_nodes_and_their_streams_match_upstream() {
    let royal = royal();
    let oracle = oracle();
    let filled = filled(&royal);
    let rows = oracle["resources"].as_array().expect("the node list");
    assert_eq!(rows.len(), 80, "every resource row");
    let mut streams_seen = 0;
    for row in rows {
        let resource = number(row, "resource") as usize;
        let node = royal.resource_node(resource);
        near(
            &format!("node {resource} grade"),
            node.grade,
            number(row, "grade"),
        );
        near(
            &format!("node {resource} base capacity"),
            node.base_capacity,
            number(row, "baseCapacity"),
        );
        near(
            &format!("node {resource} capacity"),
            node.capacity,
            number(row, "capacity"),
        );
        near(
            &format!("node {resource} saved progress"),
            node.raw_progress,
            number(row, "rawProgress"),
        );
        near(
            &format!("node {resource} collected"),
            node.collected,
            number(row, "collected"),
        );
        near(
            &format!("node {resource} remaining"),
            node.remaining,
            number(row, "remaining"),
        );
        assert_eq!(
            node.drained,
            row["drained"].as_bool().expect("a drained flag"),
            "node {resource} drained"
        );
        assert_eq!(
            node.currency_slot,
            number(row, "currencySlot") as i64,
            "node {resource} currency slot"
        );
        let detail = royal
            .resource_allocation_with(&filled, 24.0)
            .details
            .into_iter()
            .find(|detail| detail.resource == resource)
            .expect("every node reports");
        near(
            &format!("node {resource} drain rate"),
            detail.drain_rate,
            number(row, "drainRate"),
        );
        near(
            &format!("node {resource} currency rate"),
            detail.currency_rate,
            number(row, "currencyRate"),
        );
        let streams = row["streams"].as_array().expect("the stream list");
        assert_eq!(
            detail.streams.len(),
            streams.len(),
            "node {resource} stream count"
        );
        for (stream, expected) in detail.streams.iter().zip(streams) {
            assert_stream(resource, stream, expected);
        }
        streams_seen += streams.len();
    }
    assert!(streams_seen > 0, "the fixture drains at least one node");
}

#[track_caller]
fn assert_stream(resource: usize, stream: &ResourceStream, expected: &Value) {
    assert_eq!(
        stream.map,
        number(expected, "map") as usize,
        "node {resource} stream map"
    );
    assert_eq!(
        stream.slot,
        number(expected, "slot") as usize,
        "node {resource} stream slot"
    );
    near(
        &format!("node {resource} map {} stream rate", stream.map),
        stream.rate,
        number(expected, "rate"),
    );
    near(
        &format!("node {resource} map {} stream drain", stream.map),
        stream.drain_rate,
        number(expected, "drainRate"),
    );
    near(
        &format!("node {resource} map {} stream currency", stream.map),
        stream.currency_rate,
        number(expected, "currencyRate"),
    );
    assert_eq!(
        stream.savage,
        expected["savage"].as_bool().expect("a savage flag"),
        "node {resource} stream savage"
    );
}

#[test]
fn the_drain_projection_and_its_reset_maths_match_upstream() {
    let royal = royal();
    let oracle = oracle();
    let filled = filled(&royal);
    let allocation = royal.allocation_to_reset_with(&filled);
    let reset = &oracle["reset"];
    assert_eq!(
        allocation.reset.available,
        reset["available"].as_bool().expect("a reset flag"),
        "the save carries the reset countdown"
    );
    near(
        "hours to reset",
        allocation.reset.hours_remaining.unwrap_or(f64::NAN),
        number(reset, "hoursRemaining"),
    );
    assert_eq!(
        allocation.reset_projection_available,
        reset["projectionAvailable"]
            .as_bool()
            .expect("a projection flag")
    );
    near(
        "collectable before reset",
        allocation.collectable_before_reset.unwrap_or(f64::NAN),
        number(reset, "collectableBeforeReset"),
    );
    near(
        "projected remaining at reset",
        allocation.projected_remaining_at_reset.unwrap_or(f64::NAN),
        number(reset, "projectedRemainingAtReset"),
    );
    assert_eq!(
        allocation.nodes_emptying_by_reset,
        Some(number(reset, "nodesEmptyingByReset") as usize),
        "nodes emptying by reset"
    );
    assert_eq!(
        allocation.projected_refills_at_reset,
        Some(number(reset, "projectedRefillsAtReset") as usize),
        "projected refills"
    );
    near(
        "projected grade gains",
        allocation
            .projected_grade_gains_at_reset
            .unwrap_or(f64::NAN),
        number(reset, "projectedGradeGainsAtReset"),
    );
    let rows = oracle["resources"].as_array().expect("the node list");
    for row in rows {
        let resource = number(row, "resource") as usize;
        let detail = allocation
            .details
            .iter()
            .find(|detail| detail.resource == resource)
            .expect("every node reports");
        let drain_hours = row["drainHours"].as_f64();
        match (
            detail.drain_hours.is_finite().then_some(detail.drain_hours),
            drain_hours,
        ) {
            (Some(got), Some(expected)) => {
                near(&format!("node {resource} drain hours"), got, expected)
            }
            (None, None) => {}
            (got, expected) => panic!(
                "node {resource} drain hours do not travel: got {got:?}, upstream {expected:?}"
            ),
        }
        assert_eq!(
            detail.refill_boundary,
            row["refillBoundary"].as_str().expect("a boundary"),
            "node {resource} refill boundary"
        );
        assert_eq!(
            detail.refills_at_reset,
            row["refillsAtReset"].as_bool().expect("a refill flag"),
            "node {resource} refills"
        );
        assert_eq!(
            detail.refill_requires_collection,
            row["refillRequiresCollection"]
                .as_bool()
                .expect("a collection flag"),
            "node {resource} refill collection"
        );
        let expected_grade = row["projectedGradeAtReset"].as_f64();
        assert_eq!(
            detail.projected_grade_at_reset, expected_grade,
            "node {resource} projected grade"
        );
        let expected_remaining = row["projectedRemainingAtReset"].as_f64();
        match (detail.projected_remaining_at_reset, expected_remaining) {
            (Some(got), Some(expected)) => near(
                &format!("node {resource} projected remaining"),
                got,
                expected,
            ),
            (None, None) => {}
            (got, expected) => panic!(
                "node {resource} projected remaining does not travel: got {got:?}, upstream {expected:?}"
            ),
        }
    }
}

#[test]
fn the_currency_balances_and_rates_match_upstream() {
    let royal = royal();
    let oracle = oracle();
    let filled = filled(&royal);
    let rows = oracle["currencies"].as_array().expect("the currency list");
    assert!(!rows.is_empty());
    for row in rows {
        let slot = number(row, "slot") as i64;
        let income = royal.currency_income_with(slot, &filled);
        near(
            &format!("currency {slot} balance"),
            income.balance,
            number(row, "balance"),
        );
        near(
            &format!("currency {slot} nominal rate"),
            income.nominal_per_hour,
            number(row, "nominalPerHour"),
        );
        near(
            &format!("currency {slot} collectable"),
            income.collectable_within_window,
            number(row, "collectableWithinWindow"),
        );
        assert_eq!(
            income.active_streams,
            number(row, "activeStreams") as usize,
            "currency {slot} active streams"
        );
        assert_eq!(
            income.active_nodes,
            number(row, "activeNodes") as usize,
            "currency {slot} active nodes"
        );
        near(
            &format!("currency {slot} remaining"),
            income.remaining_now.unwrap_or(f64::NAN),
            number(row, "remainingNow"),
        );
        near(
            &format!("currency {slot} projected at reset"),
            income.projected_available_at_reset.unwrap_or(f64::NAN),
            number(row, "projectedAvailableAtReset"),
        );
        near(
            &format!("currency {slot} collectable before reset"),
            income.collectable_before_reset.unwrap_or(f64::NAN),
            number(row, "collectableBeforeReset"),
        );
        let nodes = row["nodes"].as_array().expect("the node list");
        assert_eq!(income.nodes.len(), nodes.len(), "currency {slot} nodes");
        for (node, expected) in income.nodes.iter().zip(nodes) {
            assert_eq!(node.resource, number(expected, "resource") as usize);
            near(
                &format!("currency {slot} node {} rate", node.resource),
                node.currency_rate,
                number(expected, "currencyRate"),
            );
            near(
                &format!("currency {slot} node {} collectable", node.resource),
                node.collectable_within_window,
                number(expected, "collectableWithinWindow"),
            );
        }
    }
}

#[test]
fn the_banked_hours_match_upstream() {
    let royal = royal();
    let oracle = oracle();
    let filled = filled(&royal);
    let expected = &oracle["banked"];
    let banked = royal.banked_with(&filled);
    assert_eq!(
        banked.banks_time,
        expected["banksTime"].as_bool().expect("a banking flag"),
        "Armory 30 makes the account bank time"
    );
    near("banked hours", banked.hours, number(expected, "hours"));
    near("banked gain", banked.gain, number(expected, "bankedGain"));
    near(
        "banked currency",
        banked.currency_gain,
        number(expected, "currencyGain"),
    );
}
