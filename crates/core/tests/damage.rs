//! The Damage Breakdown summary against the committed synthetic account.
//!
//! The three layers still need the upstream shared stat pipeline that SA-02
//! names as a gap, so the fixture pins the current, honest state: dungeons
//! report the upstream reason and every other map reports the missing pipeline
//! rather than a number.
//! The layer arithmetic itself is pinned by the unit tests in `src/damage.rs`.

use idleon_core::damage::{self, Context, DUNGEON, STATS_ENGINE, Summary};
use idleon_core::save::{Save, import};
use idleon_core::stats;

fn fixture() -> Save {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports").save
}

fn evaluate(save: &Save, character: usize, map: usize) -> Summary {
    damage::evaluate(save, Context { character, map })
}

#[test]
fn the_fixture_reports_the_missing_shared_stat_pipeline() {
    let save = fixture();
    let summary = evaluate(&save, 0, 200);
    assert!(!summary.available);
    assert!(!summary.partial);
    assert_eq!(summary.reason, STATS_ENGINE);
    assert_eq!(summary.min, 0.0);
    assert_eq!(summary.max, 0.0);
    assert_eq!(summary.mastery, 0.0);
    assert_eq!(summary.game_range(), None);
}

#[test]
fn every_upstream_dungeon_map_reports_the_upstream_reason() {
    let save = fixture();
    for map in [39, 40, 70, 71, 118, 119] {
        let summary = evaluate(&save, 0, map);
        assert!(!summary.available, "map {map}");
        assert_eq!(summary.reason, DUNGEON, "map {map}");
    }
}

#[test]
fn the_stats_registry_still_marks_damage_unported() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    let imported = import(&text).expect("the fixture imports");
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

#[test]
fn a_context_on_another_character_keeps_its_map() {
    let save = fixture();
    let summary = evaluate(&save, 3, 120);
    assert_eq!(summary.reason, STATS_ENGINE);
    assert!(damage::is_dungeon(39));
    assert!(!damage::is_dungeon(120));
}
