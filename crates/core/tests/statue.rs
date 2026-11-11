//! Statue requirements: the upstream table, the fixture parity and save decoding.

use idleon_core::save::import;
use idleon_core::statue;
use serde_json::{Value, json};

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

/// The requirement table the upstream JS produced for the fixture's first character.
fn upstream_fixture() -> Value {
    serde_json::from_str(include_str!("statue-requirements.json"))
        .expect("the extracted upstream table is valid JSON")
}

#[test]
fn the_fixture_matches_the_upstream_requirement_table() {
    let imported = import(&fixture()).expect("the fixture imports");
    let saved = &imported.statues;
    let expected = upstream_fixture();
    assert_eq!(expected["characters"].as_u64(), Some(11));
    let rows = expected["firstCharacter"]
        .as_array()
        .expect("the table has rows");
    assert_eq!(rows.len(), statue::count());
    assert_eq!(saved.first_character(), Some(0));
    assert_eq!(saved.missing_rows(0), 0);

    for character in saved.character_indices() {
        for row in rows {
            let index = row["index"].as_u64().expect("an index") as usize;
            let level = row["level"].as_u64().expect("a level") as u32;
            let exp = row["exp"].as_f64().expect("an exp");
            let required = row["required"].as_f64().expect("a required bar");
            let next_required = row["nextRequired"].as_f64().expect("a next bar");

            let progress = saved
                .progress(character, index)
                .unwrap_or_else(|| panic!("statue {index} has a saved row"));
            assert_eq!(
                progress.level, level,
                "character {character} statue {index} level"
            );
            assert!(
                (progress.exp - exp).abs() <= exp.abs() * 1e-12,
                "character {character} statue {index} exp {} != {exp}",
                progress.exp
            );

            let actual = statue::exp_requirement(saved, character, index, level)
                .unwrap_or_else(|error| panic!("statue {index} requirement: {error:?}"));
            assert!(
                (actual - required).abs() <= required.abs() * 1e-12,
                "character {character} statue {index} requirement {actual} != {required}"
            );
            let next = statue::exp_requirement(saved, character, index, level + 1)
                .unwrap_or_else(|error| panic!("statue {index} next requirement: {error:?}"));
            assert!(
                (next - next_required).abs() <= next_required.abs() * 1e-12,
                "character {character} statue {index} next requirement {next} != {next_required}"
            );

            let summary = statue::next_level(saved, character, index)
                .unwrap_or_else(|error| panic!("statue {index} next level: {error:?}"));
            assert_eq!(summary.level, level);
            assert!((summary.required - required).abs() <= required.abs() * 1e-12);
            assert!(
                (summary.remaining - (required - exp).max(0.0)).abs() <= required.abs() * 1e-12
            );
        }
    }
}

#[test]
fn the_fixture_carries_every_character_and_statue_row() {
    let imported = import(&fixture()).expect("the fixture imports");
    assert_eq!(imported.statues.character_indices().count(), 11);
    for character in imported.statues.character_indices() {
        assert_eq!(imported.statues.missing_rows(character), 0);
    }
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("statues")),
        "the fixture carries complete statue rows: {:?}",
        imported.warnings
    );
}

fn save_with(statues: Value, names: Option<Value>) -> String {
    let mut data = json!({
        "Research": json!([[ ]]).to_string(),
        "Lv0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).to_string(),
        "StatueLevels_0": statues.to_string(),
    });
    if let Some(names) = names {
        data["charNames"] = names;
    }
    json!({ "data": data }).to_string()
}

#[test]
fn a_raw_statue_row_survives_the_import() {
    let imported = import(&save_with(
        json!([[5, 12.5], [6, 0.0], [7, 1.0]]),
        Some(json!(["main"])),
    ))
    .expect("the save imports");

    assert_eq!(imported.statues.name(0), "main");
    assert_eq!(
        imported.statues.progress(0, 0),
        Some(statue::Progress {
            level: 5,
            exp: 12.5
        })
    );
    assert_eq!(imported.statues.progress(0, 3), None);
    assert_eq!(imported.statues.missing_rows(0), statue::count() - 3);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("statues")),
        "missing rows are reported: {:?}",
        imported.warnings
    );
}

#[test]
fn a_decoded_state_export_reads_statue_lv_all_data() {
    let text = json!({
        "researchLevel": 7,
        "statueLvAllData": [[[9, 3.5]]],
        "charNames": ["alt"],
    })
    .to_string();
    let imported = import(&text).expect("the decoded state imports");

    assert_eq!(imported.statues.name(0), "alt");
    assert_eq!(
        imported.statues.progress(0, 0),
        Some(statue::Progress { level: 9, exp: 3.5 })
    );
}

#[test]
fn an_unreadable_row_is_unavailable_instead_of_level_zero() {
    let imported = import(&save_with(
        json!([["not a level", 1.0], [4, -2.0], [3, 1.0]]),
        None,
    ))
    .expect("the save imports");

    assert_eq!(imported.statues.progress(0, 0), None);
    assert_eq!(imported.statues.progress(0, 1), None);
    assert_eq!(
        imported.statues.progress(0, 2),
        Some(statue::Progress { level: 3, exp: 1.0 })
    );
}

#[test]
fn a_save_without_statue_rows_warns_and_offers_no_characters() {
    let text = json!({
        "data": {
            "Research": json!([[]]).to_string(),
            "Lv0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).to_string(),
        },
    })
    .to_string();
    let imported = import(&text).expect("the save imports");

    assert!(imported.statues.is_empty());
    assert_eq!(imported.statues.first_character(), None);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("StatueLevels")),
        "the warning names the missing fields: {:?}",
        imported.warnings
    );
}
