//! The Damage Breakdown summary against the committed synthetic account.
//!
//! The three layers still need the upstream shared stat pipeline that SA-02
//! names as a gap, so the fixture pins the current, honest state: dungeons
//! report the upstream reason and every other map reports the missing pipeline
//! rather than a number. The layer arithmetic itself is pinned by the unit
//! tests in `src/lib.rs`.

use idleon_damage::{self as damage, Context, DUNGEON, STATS_ENGINE, Summary};
use idleon_import::{Imported, import};
use idleon_save::Save;
use idleon_stats as stats;

fn imported() -> Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports")
}

fn fixture() -> Save {
    imported().save
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
