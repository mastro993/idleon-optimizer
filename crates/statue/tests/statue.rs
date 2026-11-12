//! Statue requirements: the upstream table, the fixture parity and save decoding.

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

/// The requirement table the upstream JS produced for the fixture's first character.
fn upstream_fixture() -> Value {
    serde_json::from_str(include_str!("statue-requirements.json"))
        .expect("the extracted upstream table is valid JSON")
}

#[test]
fn the_fixture_matches_the_upstream_requirement_table() {
    let imported = fixture();
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

/// The target-cost table the upstream JS produced for the fixture.
fn upstream_costs() -> Value {
    serde_json::from_str(include_str!("statue-costs.json"))
        .expect("the extracted target-cost table is valid JSON")
}

fn assert_close(actual: f64, expected: &Value, what: &str) {
    let expected = expected
        .as_f64()
        .unwrap_or_else(|| panic!("{what}: the upstream table has a number"));
    let tolerance = (expected.abs() * 1e-12).max(1e-9);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{what}: got {actual}, upstream {expected}"
    );
}

#[test]
fn the_fixture_matches_the_upstream_target_cost_table() {
    let imported = fixture();
    let saved = &imported.statues;
    let leveling = saved.leveling();
    let expected = upstream_costs();
    let characters = expected["characters"]
        .as_array()
        .expect("the table has characters");
    assert_eq!(characters.len(), 11);

    for character in characters {
        let ci = character["character"].as_u64().expect("a character index") as usize;
        let inputs = &character["inputs"];
        assert_close(leveling.bubble_pct, &inputs["bubblePct"], "bubblePct");
        assert_close(leveling.cost_factor, &inputs["costFactor"], "costFactor");
        assert_close(
            leveling.retention_pct,
            &inputs["retentionPct"],
            "retentionPct",
        );
        assert_close(
            leveling.double_chance,
            &inputs["doubleChance"],
            "doubleChance",
        );
        assert_eq!(
            u64::from(leveling.onyx),
            inputs["onyxCount"].as_u64().expect("an onyx count")
        );
        assert_eq!(
            u64::from(leveling.zenith),
            inputs["zenithCount"].as_u64().expect("a zenith count")
        );
        assert_close(
            leveling.tier_multiplier,
            &inputs["tierMultiplier"],
            "tierMultiplier",
        );
        assert_eq!(
            leveling.flair_available,
            inputs["flairAvailable"].as_bool().expect("a flag")
        );

        for row in character["rows"].as_array().expect("rows") {
            let index = row["index"].as_u64().expect("an index") as usize;
            let flair = row["flairLevel"].as_u64().expect("a flair level") as u32;
            assert_eq!(leveling.flair_level(index), flair, "statue {index} flair");
            assert_close(
                leveling.flair_multiplier(index),
                &row["flairMultiplier"],
                &format!("statue {index} flair multiplier"),
            );
            for cost in row["costs"].as_array().expect("costs") {
                let target = cost["target"].as_u64().expect("a target") as u32;
                let actual = statue::target_cost(saved, leveling, ci, index, target)
                    .unwrap_or_else(|error| {
                        panic!("character {ci} statue {index} target {target}: {error:?}")
                    });
                assert_close(
                    actual.guaranteed_exp,
                    &row["guaranteedExp"],
                    &format!("statue {index} guaranteed EXP"),
                );
                assert_close(
                    actual.average_exp,
                    &row["averageExp"],
                    &format!("statue {index} average EXP"),
                );
                assert_close(
                    actual.exp_needed,
                    &cost["expNeeded"],
                    &format!("character {ci} statue {index} target {target} EXP"),
                );
                assert_eq!(
                    Some(actual.statues_guaranteed),
                    cost["statuesGuaranteed"].as_u64(),
                    "character {ci} statue {index} target {target} guaranteed statues"
                );
                assert_eq!(
                    Some(actual.statues_average),
                    cost["statuesAverage"].as_u64(),
                    "character {ci} statue {index} target {target} average statues"
                );
            }
        }
    }
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

/// The set-all entries the upstream page's handler accepted and rejected.
fn upstream_batch() -> Value {
    serde_json::from_str(include_str!("statue-batch.json"))
        .expect("the extracted set-all table is valid JSON")
}

#[test]
fn a_batch_target_matches_the_upstream_set_all_handler() {
    let imported = fixture();
    let saved = &imported.statues;
    let table = upstream_batch();
    let ci = table["character"].as_u64().expect("a character") as usize;
    assert_eq!(saved.first_character(), Some(ci));

    for entry in table["entries"].as_array().expect("entries") {
        let text = entry["entry"].as_str().expect("an entry");
        let mut plan = statue::Plan::new();
        plan.set_target(0, 3);
        let applied = plan.set_all(saved, ci, text);
        let Some(level) = entry.get("level") else {
            let error = applied.expect_err("the upstream handler rejects this entry");
            assert_eq!(
                error.message(),
                entry["error"].as_str().expect("an error"),
                "the rejection for {text:?}"
            );
            assert_eq!(plan.target(0), Some(3), "{text:?} changes nothing");
            continue;
        };
        let level = level.as_u64().expect("a level") as u32;
        applied.unwrap_or_else(|error| panic!("{text:?} applies: {}", error.message()));
        assert!(
            plan.targets().iter().all(|target| *target == Some(level)),
            "{text:?} sets every target to {level}"
        );
        for row in entry["rows"].as_array().expect("rows") {
            let statue = row["index"].as_u64().expect("an index") as usize;
            let cost = statue::target_cost(saved, saved.leveling(), ci, statue, level)
                .unwrap_or_else(|error| panic!("statue {statue} at {level}: {error:?}"));
            assert_close(
                cost.exp_needed,
                &row["expNeeded"],
                &format!("statue {statue} at level {level} EXP"),
            );
            assert_close(
                cost.guaranteed_exp,
                &row["guaranteedExp"],
                &format!("statue {statue} at level {level} guaranteed EXP"),
            );
            assert_close(
                cost.average_exp,
                &row["averageExp"],
                &format!("statue {statue} at level {level} average EXP"),
            );
            // Past 2^53 the upstream double no longer counts single statues;
            // the port keeps the exact count, so compare at its precision.
            assert_close(
                cost.statues_guaranteed as f64,
                &row["statuesGuaranteed"],
                &format!("statue {statue} at level {level} guaranteed statues"),
            );
            assert_close(
                cost.statues_average as f64,
                &row["statuesAverage"],
                &format!("statue {statue} at level {level} average statues"),
            );
        }
    }
}

#[test]
fn the_farming_breakdown_matches_the_upstream_terms() {
    let imported = fixture();
    let saved = &imported.statues;
    let leveling = saved.leveling();
    let expected = upstream_costs();
    let rate = 1_000_000.0;

    for character in expected["characters"].as_array().expect("characters") {
        let ci = character["character"].as_u64().expect("a character index") as usize;
        let inputs = &character["inputs"];
        for row in character["rows"].as_array().expect("rows") {
            let index = row["index"].as_u64().expect("an index") as usize;
            let target = row["level"].as_u64().expect("a level") as u32 + 1;
            let cost = statue::target_cost(saved, leveling, ci, index, target)
                .unwrap_or_else(|error| panic!("character {ci} statue {index}: {error:?}"));
            for estimate in [statue::Estimate::Guaranteed, statue::Estimate::Average] {
                let tree = statue::breakdown_tree(saved, ci, index, Some(target), estimate, rate);
                assert_eq!(tree.children.len(), 4);
                assert_eq!(
                    tree.value,
                    estimate.statues(&cost) as f64,
                    "character {ci} statue {index} needs the row's statues"
                );
                let needed = &tree.children[0];
                assert_close(
                    needed.value,
                    &row["costs"][0]["expNeeded"],
                    &format!("character {ci} statue {index} EXP needed"),
                );
                assert_close(
                    needed.children[0].value,
                    &inputs["bubblePct"],
                    "the STARTUE bubble",
                );
                assert_close(
                    needed.children[1].value,
                    &inputs["retentionPct"],
                    "the EXP retention",
                );
                let deposit = &tree.children[1];
                assert_close(
                    deposit.value,
                    if matches!(estimate, statue::Estimate::Guaranteed) {
                        &row["guaranteedExp"]
                    } else {
                        &row["averageExp"]
                    },
                    &format!("character {ci} statue {index} deposit"),
                );
                assert_close(
                    deposit.children[0].value,
                    &inputs["tierMultiplier"],
                    "the Onyx/Zenith tier",
                );
                assert_close(
                    deposit.children[1].value,
                    &row["flairMultiplier"],
                    &format!("statue {index} Flair"),
                );
                assert_close(
                    deposit.children[2].value,
                    &json!(estimate.vial(leveling.double_chance)),
                    "the vial under the selected estimate",
                );
                let time = &tree.children[3];
                assert_close(
                    time.value,
                    &json!(estimate.statues(&cost) as f64 / rate),
                    "the time the rate covers",
                );
            }
        }
    }
}
