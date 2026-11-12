//! The calculation context: the selector lists and the save defaults behind
//! them, against upstream `js/stats/systems/common/map-access.js`.

use idleon_import::{Imported, import};
use idleon_research::sim;
use idleon_save::Save;
use idleon_stats::{
    ConstructionMode, Context, MapVisibility, Skill, Snapshot, build_options, classify,
};
use serde_json::json;

fn fixture() -> Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports")
}

fn snapshot(save: serde_json::Value) -> Snapshot {
    Snapshot::from_save(&Save::new(save))
}

fn map_ids(snapshot: &Snapshot, character: usize, visibility: MapVisibility) -> Vec<usize> {
    build_options(character, snapshot, visibility)
        .into_iter()
        .map(|option| option.class.map)
        .collect()
}

#[test]
fn a_save_opens_on_character_one_and_its_current_map() {
    let snapshot = snapshot(json!({
        "charNames": ["Ana", "Bo"],
        "Lv0_0": "[1]",
        "Lv0_1": "[1]",
        "CurrentMap_0": 0,
        "CurrentMap_1": 120,
    }));
    let context = Context::from_snapshot(&snapshot);
    assert_eq!(context.character, 0);
    assert_eq!(context.map, 0);
    assert_eq!(context.skill, Skill::Mining);
    assert_eq!(context.map_visibility, MapVisibility::Accessible);
    assert_eq!(context.construction, ConstructionMode::Active);

    // Upstream `switchChar` follows the new character's current map.
    let mut context = context;
    context.select_character(1, &snapshot);
    assert_eq!(context.character, 1);
    assert_eq!(context.map, 120);
}

#[test]
fn the_current_map_leads_the_list_and_hidden_maps_never_do() {
    let snapshot = snapshot(json!({
        "Lv0_0": "[1]",
        "CurrentMap_0": 200,
    }));
    let options = build_options(0, &snapshot, MapVisibility::Accessible);
    let first = options.first().expect("a current map is always listed");
    assert_eq!(first.class.map, 200);
    assert!(first.current);
    // Upstream `_mapLabel` shows the target name with its underscores turned
    // into spaces; this map's target is the `_` placeholder, so it reads as one.
    assert_eq!(
        first.label(snapshot.map_bonus(200)),
        "Magma Rivertown -   (current)"
    );
    assert!(options.iter().all(|option| option.class.map != 3));

    // The hidden scenes stay out of every list level, including "All".
    let all = map_ids(&snapshot, 0, MapVisibility::All);
    assert!(!all.contains(&3), "{all:?}");
    assert!(!all.contains(&299), "{all:?}");
}

#[test]
fn an_advanced_map_label_names_its_mob_bonus_and_category() {
    let snapshot = snapshot(json!({
        "KLA_0": "[[0]]",
        "CurrentMap_0": 50,
        "MapBon": "[[0], [0], [0]]",
    }));
    let boss = build_options(0, &snapshot, MapVisibility::All)
        .into_iter()
        .find(|option| option.class.map == 29)
        .expect("the boss map is in the full list");
    assert!(boss.locked, "the boss map is listed but not proven");
    assert_eq!(
        boss.label(Some(&[1_000_000.0])),
        "End Of The Road - Wode Board (1.38x) (not proven accessible) [boss]"
    );
    assert_eq!(
        boss.label(None),
        "End Of The Road - Wode Board (not proven accessible) [boss]"
    );
}

#[test]
fn a_hidden_current_map_is_still_listed() {
    // Upstream includes the current map before it filters by visibility.
    let snapshot = snapshot(json!({
        "Lv0_0": "[1]",
        "CurrentMap_0": 3,
    }));
    let options = build_options(0, &snapshot, MapVisibility::Accessible);
    assert_eq!(
        options.first().map(|option| option.class.map),
        Some(3),
        "a current internal scene still names the map the context is on"
    );
}

#[test]
fn the_list_levels_match_the_upstream_rules() {
    let snapshot = snapshot(json!({
        "Lv0_0": "[1]",
        "CurrentMap_0": 0,
    }));
    let accessible = map_ids(&snapshot, 0, MapVisibility::Accessible);
    let advanced = map_ids(&snapshot, 0, MapVisibility::Advanced);
    let all = map_ids(&snapshot, 0, MapVisibility::All);
    // A boss map is a special scene: never in the plain list, listed from
    // "Accessible + Special" because its world's town is unlocked.
    assert!(!accessible.contains(&29), "{accessible:?}");
    assert!(advanced.contains(&29), "{advanced:?}");
    assert!(all.len() >= advanced.len());
    // A feature map in an unlocked world follows the same rule.
    assert!(!accessible.contains(&41), "{accessible:?}");
    assert!(advanced.contains(&41), "{advanced:?}");
    // A map in a world whose gate is not cleared is only in the full list.
    assert!(!advanced.contains(&100), "{advanced:?}");
    assert!(all.contains(&100), "{all:?}");
}

#[test]
fn cleared_portals_unlock_their_destination_with_the_upstream_reason() {
    // The map's first counter cleared: upstream reads `transitions[0 + 1]`.
    let snapshot = snapshot(json!({
        "KLA_0": "[[0]]",
    }));
    let options = build_options(0, &snapshot, MapVisibility::Accessible);
    let destiny = options
        .iter()
        .find(|option| option.class.map == 50)
        .expect("Blunder Hills' first portal leads to the desert town");
    assert!(destiny.accessible);
    // The world starter claims the map first, exactly like upstream `add()`.
    assert_eq!(destiny.evidence, "World 2 unlocked");

    // A boss scene is never reachable through the plain access walk, so the
    // portal from the town into the grass does not appear without a clear.
    let locked = build_options(0, &snapshot, MapVisibility::All)
        .into_iter()
        .find(|option| option.class.map == 15)
        .expect("the sewers are in the full list");
    assert!(!locked.accessible);
    assert!(locked.locked);
    assert!(
        locked
            .label(snapshot.map_bonus(15))
            .contains("(not proven accessible)")
    );
}

#[test]
fn a_map_with_a_multi_character_requirement_needs_that_many_clears() {
    // A KLA row carries one counter row per map; `Seaweed Street` (303) needs
    // five characters with its first counter cleared before its portal opens.
    let kla = |cleared: bool| {
        let mut rows = vec![Vec::<u32>::new(); 304];
        if cleared {
            rows[303] = vec![0, 0];
        }
        json!(rows)
    };
    let mut save = json!({
        "KLA_0": kla(true),
        "CurrentMap_0": 303,
    });
    // Four characters are not enough for a five-clear map.
    for character in 1..4 {
        save[format!("KLA_{character}")] = kla(true);
    }
    let four = snapshot(save.clone());
    let options = build_options(0, &four, MapVisibility::Accessible);
    let source = options
        .iter()
        .find(|option| option.class.map == 303)
        .expect("the current map is listed");
    assert!(source.current);
    assert!(!options.iter().any(|option| option.class.map == 307));

    // The fifth clear opens the destination below.
    save["KLA_4"] = kla(true);
    let five = snapshot(save);
    let options = build_options(0, &five, MapVisibility::Accessible);
    let destination = options
        .iter()
        .find(|option| option.class.map == 307)
        .expect("five clears open Seaweed Street's portal");
    assert!(destination.accessible);
    assert_eq!(destination.evidence, "Cleared portal from Seaweed Street");
}

#[test]
fn selecting_a_map_outside_the_list_falls_back_to_the_current_one() {
    let snapshot = snapshot(json!({
        "Lv0_0": "[1]",
        "CurrentMap_0": 50,
    }));
    let mut context = Context {
        map: 29,
        ..Context::default()
    };
    assert_eq!(context.clamp_map(&snapshot), Some(29));
    assert_eq!(context.map, 50);
    // A map already in the list is left alone.
    let mut context = Context {
        map: 50,
        ..Context::default()
    };
    assert_eq!(context.clamp_map(&snapshot), None);
    assert_eq!(context.map, 50);
}

#[test]
fn the_fixture_opens_on_its_saved_current_map() {
    let snapshot = Snapshot::from_save(&fixture().save);
    assert!(snapshot.characters() >= 10);
    assert_eq!(snapshot.current_map(0), Some(200));
    assert_eq!(snapshot.name(0), "Character 1");

    let context = Context::from_snapshot(&snapshot);
    assert_eq!(context.map, 200);
    let options = build_options(0, &snapshot, MapVisibility::Accessible);
    assert_eq!(
        options.first().map(|option| option.class.map),
        Some(200),
        "the saved current map leads the selector"
    );
    assert_eq!(classify(200).category.id(), "town");
    let mut seen = std::collections::HashSet::new();
    for option in &options {
        assert!(
            seen.insert(option.class.map),
            "map {} appears twice",
            option.class.map
        );
        assert!(!option.class.name.is_empty());
    }
}

/// The upstream stats page's own output for the committed fixture, read by
/// running `computeMaxCapacity` from
/// `js/stats/systems/common/carry-capacity.js` over the same save.
const FIXTURE_CAPACITY: [(&str, f64); 8] = [
    ("carry-capacity-mining", 113_019_116.0),
    ("carry-capacity-chopping", 123_532_522.0),
    ("carry-capacity-fishing", 90_415_293.0),
    ("carry-capacity-catching", 77_799_205.0),
    ("carry-capacity-food", 10_513_406.0),
    ("carry-capacity-critters", 10_513_406.0),
    ("carry-capacity-souls", 10_513_406.0),
    ("carry-capacity-materials", 542_813_368.0),
];

fn fixture_sheet(imported: &Imported) -> idleon_stats::Sheet {
    let inputs = idleon_stats::BreakdownInputs {
        save: &imported.save,
        simulation: &imported.simulation,
        sim_context: &imported.context,
        character: 0,
        map: 0,
    };
    idleon_stats::Sheet::build(&inputs)
}

#[test]
fn the_fixture_capacity_matches_the_upstream_stats_page() {
    let sheet = fixture_sheet(&fixture());
    for (id, expected) in FIXTURE_CAPACITY {
        let row = sheet.get(id).unwrap_or_else(|| panic!("{id} is listed"));
        assert_eq!(
            row.resolution.value(),
            Some(expected),
            "{id} disagrees with the upstream page"
        );
    }
    // The roster sits beside the save in an it.json and this fixture is flat,
    // exactly like upstream's own partial reason for the same save.
    let mining = sheet.get("carry-capacity-mining").expect("the row");
    let warning = mining.warning().expect("the missing roster is named");
    assert!(warning.contains("companion roster"), "{warning}");
}

#[test]
fn the_fixture_research_rows_agree_with_the_simulator() {
    let imported = fixture();
    let sheet = fixture_sheet(&imported);
    let exp = sheet.get("research-exp").expect("research-exp is listed");
    assert_eq!(
        exp.resolution.value(),
        Some(sim::exp_rate(&imported.simulation, &imported.context).multi)
    );
    let afk = sheet
        .get("research-afk-gains")
        .expect("research-afk-gains is listed");
    assert_eq!(
        afk.resolution.value(),
        Some(sim::afk_rate(&imported.simulation, &imported.context).rate * 100.0)
    );
}

#[test]
fn every_category_renders_and_unported_rows_name_their_source() {
    let sheet = fixture_sheet(&fixture());
    for category in idleon_stats::BreakdownCategory::ALL {
        let rows: Vec<&idleon_stats::Breakdown> = sheet.in_category(category).collect();
        assert!(!rows.is_empty(), "{} is empty", category.label());
        for row in rows {
            assert_eq!(row.category, category);
            match &row.resolution {
                idleon_stats::Resolution::Value { tree, .. } => {
                    assert_eq!(Some(tree.value), row.resolution.value());
                }
                idleon_stats::Resolution::Unavailable { reason } => {
                    assert!(reason.contains(row.source), "{}: {reason}", row.id);
                }
            }
        }
    }
}
