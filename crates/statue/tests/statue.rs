//! Statue requirements and save decoding.

use idleon_import::{Imported, import};
use idleon_statue as statue;
use serde_json::{Value, json};

fn fixture() -> Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports")
}

/// A one-character raw save carrying `fields` on top of the minimum the
/// importer needs.
fn raw_save(fields: Value) -> String {
    let mut data = json!({
        "Research": json!([[]]).to_string(),
        "Lv0_0": json!([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1]).to_string(),
    });
    let Value::Object(fields) = fields else {
        panic!("save fields are an object");
    };
    for (key, value) in fields {
        data[key] = value;
    }
    json!({ "data": data }).to_string()
}

#[test]
fn the_fixture_carries_every_character_and_statue_row() {
    let imported = fixture();
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
    let mut fields = json!({ "StatueLevels_0": statues.to_string() });
    if let Some(names) = names {
        fields["charNames"] = names;
    }
    raw_save(fields)
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
    let imported = import(&raw_save(json!({}))).expect("the save imports");

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

#[test]
fn a_save_without_royal_guardian_data_keeps_flair_neutral() {
    let imported = import(&save_with(json!([[5, 0.0]]), None)).expect("the save imports");
    let leveling = imported.statues.leveling();
    assert!(!leveling.flair_available);
    assert_eq!(leveling.flair_level(0), 0);
    assert_eq!(leveling.flair_multiplier(0), 1.0);
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("RoyalG")),
        "the missing Flair row is named: {:?}",
        imported.warnings
    );
}

/// A statue save with a STARTUE bubble level at `level` and nothing else.
fn save_with_bubble(level: f64) -> String {
    let mut yellow = vec![0.0; 46];
    yellow[2] = level;
    let mut cauldron = vec![vec![]; 5];
    cauldron[3] = yellow;
    raw_save(json!({
        "StatueLevels_0": json!(vec![[10.0, 0.0]; statue::count()]).to_string(),
        "CauldronInfo": json!(cauldron).to_string(),
    }))
}

#[test]
fn the_startue_bubble_from_the_save_changes_the_estimate() {
    let plain = import(&save_with_bubble(0.0)).expect("the save imports");
    let bubbled = import(&save_with_bubble(400.0)).expect("the save imports");
    let plain_leveling = plain.statues.leveling();
    let bubbled_leveling = bubbled.statues.leveling();
    assert_eq!(plain_leveling.retention_pct, 0.0);
    assert!(bubbled_leveling.retention_pct > 0.0);
    assert!(bubbled_leveling.retention_pct <= 50.0);

    // One level's bar is always full, so the bubble shows on a multi-level target.
    let plain_cost =
        statue::target_cost(&plain.statues, plain_leveling, 0, 0, 20).expect("a target");
    let bubbled_cost =
        statue::target_cost(&bubbled.statues, bubbled_leveling, 0, 0, 20).expect("a target");
    assert!(
        bubbled_cost.exp_needed < plain_cost.exp_needed,
        "the bubble lowers the EXP a target needs: {} vs {}",
        bubbled_cost.exp_needed,
        plain_cost.exp_needed
    );
    assert!(bubbled_cost.statues_guaranteed < plain_cost.statues_guaranteed);
}
