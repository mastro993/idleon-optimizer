//! The Damage Breakdown summary against the committed synthetic account and
//! the upstream reference beside it.
//!
//! `script/damage-oracle.mjs` runs `js/stats/defs/damage.js` over
//! `fixtures/data.json` through `createStatContext` and writes
//! `fixtures/damage.json`: one summary per character, and the descriptor's own
//! dungeon reason. Every expected number here is upstream's output.
//!
//! The three layers still need the shared stat pipeline that SA-02 names as a
//! gap, so the port reports the missing-pipeline reason instead of a number.
//! [`the_summary_reports_the_missing_engine_until_the_provider_lands`] compares
//! against upstream as soon as that changes, so the provider cannot land
//! without matching this fixture.

use idleon_damage::{self as damage, DUNGEON, Layers, Metadata, STATS_ENGINE, Summary};
use idleon_import::{Imported, import};
use idleon_save::Save;
use idleon_stats::{self as stats, Snapshot};
use serde_json::Value;

fn imported() -> Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports")
}

fn fixture() -> Save {
    imported().save
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/damage.json");
    let text = std::fs::read_to_string(path).expect("fixtures/damage.json is committed");
    serde_json::from_str(&text).expect("the oracle fixture is JSON")
}

fn cases(oracle: &Value) -> &Vec<Value> {
    oracle["cases"]
        .as_array()
        .expect("the oracle carries one case per character")
}

/// The compare the oracle's own output needs: relative to the larger value.
fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// Upstream's descriptor says "damage", the page says "DMG"; the rest has to
/// agree word for word.
fn normalized(reason: &str) -> String {
    reason
        .to_ascii_lowercase()
        .replace("unavailable:", "")
        .replace("dmg", "damage")
        .trim()
        .trim_end_matches('.')
        .to_string()
}

fn evaluate(save: &Save, character: usize, map: usize) -> Summary {
    damage::evaluate(save, damage::Context { character, map })
}

#[test]
fn every_character_context_matches_the_upstream_cases() {
    let save = fixture();
    let snapshot = Snapshot::from_save(&save);
    let oracle = oracle();
    let cases = cases(&oracle);
    assert_eq!(
        snapshot.characters(),
        cases.len(),
        "the oracle resolves one case per character"
    );
    for expected in cases {
        let character = expected["character"].as_u64().expect("an index") as usize;
        assert_eq!(
            snapshot.current_map(character),
            Some(expected["map"].as_u64().expect("a map") as usize)
        );
        assert_eq!(
            snapshot.class_id(character),
            expected["class"].as_u64().expect("a class") as usize
        );
    }
}

#[test]
fn the_layer_combine_matches_every_upstream_case() {
    let oracle = oracle();
    for expected in cases(&oracle) {
        let layers = Layers {
            base: expected["layers"]["base"].as_f64().expect("a base"),
            percent: expected["layers"]["percent"].as_f64().expect("a percent"),
            multiplier: expected["layers"]["multiplier"]
                .as_f64()
                .expect("a multiplier"),
            mastery: expected["mastery"].as_f64().expect("a mastery"),
        };
        let summary = Summary::from_layers(layers);
        let expected_max = expected["max"].as_f64().expect("a max");
        let expected_min = expected["min"].as_f64().expect("a min");
        assert!(
            close(summary.max, expected_max),
            "character {}: {} != {expected_max}",
            expected["character"],
            summary.max
        );
        assert!(
            close(summary.min, expected_min),
            "character {}: {} != {expected_min}",
            expected["character"],
            summary.min
        );
        assert!(close(summary.mastery, layers.mastery));
    }
}

#[test]
fn the_summary_reports_the_missing_engine_until_the_provider_lands() {
    let save = fixture();
    let oracle = oracle();
    for expected in cases(&oracle) {
        let character = expected["character"].as_u64().expect("an index") as usize;
        let map = expected["map"].as_u64().expect("a map") as usize;
        let summary = evaluate(&save, character, map);
        if summary.available {
            // The provider landed: it has to match upstream, case by case.
            assert!(
                close(summary.max, expected["max"].as_f64().expect("a max")),
                "character {character}: max {} != {}",
                summary.max,
                expected["max"]
            );
            assert!(
                close(summary.min, expected["min"].as_f64().expect("a min")),
                "character {character}: min {} != {}",
                summary.min,
                expected["min"]
            );
            assert!(close(
                summary.mastery,
                expected["mastery"].as_f64().expect("a mastery")
            ));
            continue;
        }
        assert_eq!(summary.reason, STATS_ENGINE, "character {character}");
        assert_eq!(summary.game_range(), None);
        assert_eq!(
            summary.partial(),
            expected["partial"]
                .as_bool()
                .expect("the descriptor's partial"),
            "character {character}"
        );
        assert_eq!(
            summary.partial_reason(),
            expected["reason"]
                .as_str()
                .expect("the descriptor's reason"),
            "character {character}"
        );
    }
}

#[test]
fn the_metadata_flags_match_the_descriptor() {
    let imported = imported();
    let oracle = oracle();
    let summary = evaluate(&imported.save, 0, 200);
    assert_eq!(
        summary.partial_reason(),
        oracle["metadata"]["missing"]
            .as_str()
            .expect("the fixture carries the missing-metadata reason")
    );
    // The descriptor clears both flags once the wrapper carries the roster and
    // the vote row; the port reads the same two fields.
    assert!(!summary.metadata.complete());
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed");
    let flat: Value = serde_json::from_str(&text).expect("the fixture is JSON");
    let wrapped = Save::new(serde_json::json!({
        "data": flat,
        "companion": { "l": [] },
        "serverVars": { "voteCategories": [0] },
    }));
    let metadata = Metadata::read(&wrapped);
    assert_eq!(
        metadata.complete(),
        !oracle["metadata"]["complete"]["partial"]
            .as_bool()
            .expect("the fixture carries the complete case")
    );
    assert_eq!(metadata.reason(), "");
}

#[test]
fn the_map_selector_lists_the_upstream_maps() {
    let save = fixture();
    let snapshot = Snapshot::from_save(&save);
    let oracle = oracle();
    let maps = oracle["maps"]
        .as_array()
        .expect("the fixture lists the maps");
    let table: Vec<(usize, String)> = maps
        .iter()
        .map(|row| {
            (
                row["map"].as_u64().expect("a map") as usize,
                row["name"].as_str().expect("a name").to_string(),
            )
        })
        .collect();
    let dungeons: Vec<usize> = oracle["dungeons"]
        .as_array()
        .expect("the dungeon maps")
        .iter()
        .map(|row| row["map"].as_u64().expect("a map") as usize)
        .collect();
    for expected in cases(&oracle) {
        let character = expected["character"].as_u64().expect("an index") as usize;
        let map = expected["map"].as_u64().expect("a map") as usize;
        let rows = damage::Context { character, map }.map_rows(&snapshot);
        assert_eq!(rows.len(), table.len(), "character {character}");
        for (row, (index, name)) in rows.iter().zip(&table) {
            assert_eq!(row.map, *index);
            let mut label = name.clone();
            if dungeons.contains(index) {
                label.push_str(" (dungeon)");
            }
            if *index == map {
                label.push_str(" (current)");
            }
            assert_eq!(row.label, label, "map {index}");
            assert_eq!(row.dungeon, dungeons.contains(index), "map {index}");
            assert_eq!(row.current, *index == map, "map {index}");
        }
    }
}

#[test]
fn every_upstream_dungeon_map_reports_the_descriptor_reason() {
    let save = fixture();
    let oracle = oracle();
    let dungeons = oracle["dungeons"]
        .as_array()
        .expect("the oracle carries the dungeon maps");
    assert_eq!(dungeons.len(), 6, "the upstream dungeon set");
    for expected in dungeons {
        let map = expected["map"].as_u64().expect("a map") as usize;
        assert!(damage::is_dungeon(map), "map {map}");
        let summary = evaluate(&save, 0, map);
        assert!(!summary.available, "map {map}");
        assert_eq!(summary.reason, DUNGEON);
        assert_eq!(
            normalized(DUNGEON),
            normalized(expected["reason"].as_str().expect("a reason")),
            "map {map}"
        );
    }
}

#[test]
fn the_stats_registry_still_marks_damage_unported() {
    let imported = imported();
    let inputs = stats::BreakdownInputs {
        save: &imported.save,
        simulation: &imported.simulation,
        sim_context: &imported.context,
        character: 0,
        map: 200,
    };
    let sheet = stats::Sheet::build(&inputs);
    let row = sheet
        .get("damage")
        .expect("the stats registry lists damage");
    match &row.resolution {
        stats::Resolution::Unavailable { reason } => {
            assert!(reason.contains("js/stats/defs/damage.js"), "{reason}");
            assert!(reason.contains("shared stat pipeline"), "{reason}");
        }
        stats::Resolution::Value { .. } => {
            panic!("damage resolved in the stats registry; port its provider into this module")
        }
    }
}
