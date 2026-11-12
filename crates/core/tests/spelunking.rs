//! Spelunking parity against the upstream calculator's own output.
//!
//! `fixtures/spelunking.json` is written by `script/spelunking-oracle.mjs` from
//! `js/stats/systems/w7/spelunk-inputs.js`, `spelunk-expedition.js` and
//! `spelunk-depth.js`, so the port is checked against the source of truth rather
//! than hand-copied numbers.
//!
//! The four account multipliers that upstream derives from its whole stat
//! context are explicit here, exactly as the app's account-input panel supplies
//! them; the save-derived half is the upstream `buildSpelunkInputs` output.

use idleon_core::save::import;
use idleon_core::spelunking::{
    Account, ConfigError, DepthRange, ReferenceTool, Requested, Save, normalize, profile,
};
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/spelunking.json")).expect("oracle JSON")
}

/// The port's inputs for the fixture, with the manual account values the oracle
/// passes to the same formulas.
fn inputs(oracle: &Value) -> idleon_core::spelunking::Inputs {
    let imported = import(&read("fixtures/data.json")).expect("the fixture imports");
    let save = Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows");
    let account = &oracle["account"];
    let max_stamina: Vec<f64> = oracle["characters"]
        .as_array()
        .expect("characters")
        .iter()
        .map(|character| number(&character["max"]))
        .collect();
    save.inputs(&Account {
        power: number(&account["power"]),
        holes_bonus: number(&account["holesBonus"]),
        max_stamina,
    })
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("a number")
}

fn close(actual: f64, expected: f64, path: &str) {
    let tolerance = 1e-9 * expected.abs().max(1.0);
    assert!(
        (actual - expected).abs() <= tolerance,
        "{path}: {actual} != {expected}"
    );
}

fn strings(value: &Value) -> Vec<usize> {
    value
        .as_array()
        .expect("an array")
        .iter()
        .map(|item| item.as_u64().expect("an index") as usize)
        .collect()
}

#[test]
fn the_save_derives_the_upstream_inputs() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    assert_eq!(
        inputs.characters.len(),
        oracle["characters"].as_array().unwrap().len()
    );
    for (index, expected) in oracle["characters"].as_array().unwrap().iter().enumerate() {
        let character = &inputs.characters[index];
        assert_eq!(character.id, index);
        assert_eq!(character.name, expected["name"].as_str().unwrap());
        assert_eq!(
            character.class_id as u64,
            expected["classId"].as_u64().unwrap()
        );
        close(
            character.level,
            number(&expected["level"]),
            "character.level",
        );
        close(
            character.stamina,
            number(&expected["stamina"]),
            "character.stamina",
        );
        close(
            character.max_stamina,
            number(&expected["max"]),
            "character.max",
        );
    }
    assert_eq!(
        inputs.caves.len(),
        oracle["caves"].as_array().unwrap().len()
    );
    for (cave, expected) in inputs.caves.iter().zip(oracle["caves"].as_array().unwrap()) {
        assert_eq!(cave.id, expected["id"].as_u64().unwrap() as usize);
        assert_eq!(cave.name, expected["name"].as_str().unwrap());
        assert_eq!(
            cave.capacity,
            expected["capacity"].as_u64().unwrap() as usize
        );
        close(
            cave.boss_depth,
            number(&expected["bossDepth"]),
            "cave.bossDepth",
        );
    }
    assert_eq!(
        inputs.tools.len(),
        oracle["tools"].as_array().unwrap().len()
    );
    for (tool, expected) in inputs.tools.iter().zip(oracle["tools"].as_array().unwrap()) {
        assert_eq!(tool.id, expected["id"].as_u64().unwrap() as usize);
        assert_eq!(tool.name, expected["name"].as_str().unwrap());
        close(tool.radius, number(&expected["radius"]), "tool.radius");
        close(tool.damage, number(&expected["damage"]), "tool.damage");
        close(tool.cost, number(&expected["cost"]), "tool.cost");
    }
    for (index, expected) in oracle["shop"].as_array().unwrap().iter().enumerate() {
        close(
            inputs.shop[index],
            number(expected),
            &format!("shop[{index}]"),
        );
    }
    assert_eq!(
        inputs.elixir_owned,
        oracle["elixirOwned"].as_u64().unwrap() as usize
    );
    assert_eq!(inputs.slots, oracle["slots"].as_u64().unwrap() as usize);
    assert_eq!(inputs.dupes, oracle["dupes"].as_u64().unwrap() as usize);
    assert_eq!(
        inputs.memory_slots,
        oracle["memorySlots"].as_u64().unwrap() as usize
    );
    assert_eq!(inputs.memorized, strings(&oracle["memorized"]));
    let saved: Vec<i32> = oracle["savedElixirs"]
        .as_array()
        .unwrap()
        .iter()
        .map(|id| id.as_i64().unwrap() as i32)
        .collect();
    assert_eq!(inputs.saved_elixirs, saved);
    assert_eq!(inputs.saved_manic, oracle["savedManic"].as_bool().unwrap());
    assert_eq!(inputs.full_clear, oracle["fullClear"].as_bool().unwrap());
    assert_eq!(inputs.auto, oracle["auto"].as_bool().unwrap());
    assert_eq!(
        inputs.speed_max,
        oracle["speedMax"].as_u64().unwrap() as u32
    );
    assert_eq!(inputs.manic, oracle["manic"].as_bool().unwrap());
    assert_eq!(inputs.partial, oracle["partial"].as_bool().unwrap());
    for (index, expected) in oracle["depths"].as_array().unwrap().iter().enumerate() {
        close(inputs.depths[index], number(expected), "depths");
    }
    for (index, expected) in oracle["manicDepths"].as_array().unwrap().iter().enumerate() {
        close(inputs.manic_depths[index], number(expected), "manicDepths");
    }
}

#[test]
fn every_profile_matches_the_upstream_calculator() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    for config in oracle["configs"].as_array().unwrap() {
        let label = config["label"].as_str().unwrap();
        let requested: Requested =
            serde_json::from_value(config["requested"].clone()).expect("the request decodes");
        let range = &config["range"];
        let range = DepthRange {
            from: range["from"].as_u64().unwrap() as usize,
            to: range["to"].as_u64().unwrap() as usize,
            tool: match &range["tool"] {
                Value::String(_) => ReferenceTool::Best,
                Value::Number(id) => ReferenceTool::Tool(id.as_u64().unwrap() as usize),
                other => panic!("{label}: unexpected tool {other:?}"),
            },
        };
        let profile =
            profile(&inputs, &requested, &range).unwrap_or_else(|error| panic!("{label}: {error}"));
        let expected_config = &config["config"];
        assert_eq!(
            profile.config.cave,
            expected_config["cave"].as_u64().unwrap() as usize
        );
        assert_eq!(profile.config.party, strings(&expected_config["party"]));
        assert_eq!(
            profile.config.manic,
            expected_config["manic"].as_bool().unwrap()
        );
        assert_eq!(
            profile.config.policy.id(),
            expected_config["policy"].as_str().unwrap()
        );
        assert_eq!(
            profile.config.speed,
            expected_config["speed"].as_u64().unwrap() as u32
        );
        let elixirs: Vec<i32> = expected_config["elixirs"]
            .as_array()
            .unwrap()
            .iter()
            .map(|id| id.as_i64().unwrap() as i32)
            .collect();
        assert_eq!(profile.config.elixirs, elixirs);
        assert_eq!(
            profile.config.memorized,
            strings(&expected_config["memorized"])
        );
        assert_eq!(
            profile.config.tool,
            match &expected_config["tool"] {
                Value::String(_) => idleon_core::spelunking::ToolChoice::Adaptive,
                Value::Number(id) => {
                    idleon_core::spelunking::ToolChoice::Tool(id.as_u64().unwrap() as usize)
                }
                other => panic!("{label}: unexpected tool {other:?}"),
            }
        );
        assert_eq!(
            profile.config.entry.id(),
            expected_config["entry"].as_str().unwrap()
        );
        assert_eq!(
            profile.config.full_clear,
            expected_config["fullClear"].as_bool().unwrap()
        );
        close(
            profile.config.reaction_ms,
            number(&expected_config["reactionMs"]),
            "reactionMs",
        );
        assert_eq!(
            profile.config.stamina.id(),
            expected_config["stamina"].as_str().unwrap()
        );
        close(profile.budget, number(&config["budget"]), "budget");
        close(profile.power, number(&config["power"]), "power");
        close(profile.refund, number(&config["refund"]), "refund");
        let holes = &config["holes"];
        for index in 0..3 {
            close(profile.holes[index], number(&holes[index]), "holes");
        }
        let rows = config["rows"].as_array().unwrap();
        assert_eq!(profile.rows.len(), rows.len(), "{label}: row count");
        for (row, expected) in profile.rows.iter().zip(rows) {
            let path = format!("{label} D{}", row.depth);
            assert_eq!(row.depth, expected["depth"].as_u64().unwrap() as usize);
            close(row.hard, number(&expected["hard"]), &format!("{path}.hard"));
            close(
                row.blocked_exits,
                number(&expected["blockedExits"]),
                &format!("{path}.blockedExits"),
            );
            let objects = expected["objects"].as_array().unwrap();
            assert_eq!(row.objects.len(), objects.len(), "{path}: object count");
            for (object, expected) in row.objects.iter().zip(objects) {
                assert_eq!(object.kind, expected["type"].as_u64().unwrap() as usize);
                assert_eq!(object.name, expected["name"].as_str().unwrap());
                assert_eq!(object.boss, expected["boss"].as_bool().unwrap());
                close(object.probability, number(&expected["probability"]), &path);
                close(object.hp, number(&expected["hp"]), &path);
                close(object.requirement, number(&expected["requirement"]), &path);
                close(object.hit_chance, number(&expected["hitChance"]), &path);
                assert_eq!(object.tool, expected["tool"].as_u64().unwrap() as usize);
                close(object.hits, number(&expected["hits"]), &path);
                close(object.cost, number(&expected["cost"]), &path);
                close(object.per_attempt, number(&expected["perAttempt"]), &path);
                close(object.expected, number(&expected["expected"]), &path);
                assert_eq!(
                    object.over_budget,
                    expected["overBudget"].as_bool().unwrap()
                );
            }
        }
    }
}

#[test]
fn every_rejection_reports_the_upstream_message() {
    let oracle = oracle();
    for entry in oracle["rejections"].as_array().unwrap() {
        let label = entry["label"].as_str().unwrap();
        let mut inputs = inputs(&oracle);
        match &entry["input"] {
            Value::String(kind) if kind == "controlled" => {}
            Value::String(_) => match label {
                "tired member" | "tired member with full stamina is allowed" => {
                    let max = inputs.characters[0].max_stamina;
                    inputs.characters[0].stamina = (max / 4.0).floor();
                }
                "locked manic" => inputs.manic = false,
                "duplicate limit" => inputs.dupes = 1,
                "locked policy" => inputs.auto = false,
                "locked tool" => inputs.tools.truncate(1),
                other => panic!("unhandled input variant {other}"),
            },
            other => panic!("{label}: unexpected input {other:?}"),
        }
        let requested: Requested =
            serde_json::from_value(entry["requested"].clone()).expect("the request decodes");
        let outcome: Result<_, ConfigError> = normalize(&inputs, &requested);
        match &entry["error"] {
            Value::Null => assert!(outcome.is_ok(), "{label}: expected acceptance"),
            Value::String(message) => {
                let error = outcome.expect_err("expected rejection");
                assert_eq!(&error.to_string(), message, "{label}");
            }
            other => panic!("{label}: unexpected error {other:?}"),
        }
    }
}

#[test]
fn the_depth_range_is_bounded_like_upstream() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    for (from, to) in [(0, 10), (10, 5), (1, 10_001), (1, 1_002)] {
        let error = profile(
            &inputs,
            &Requested::default(),
            &DepthRange {
                from,
                to,
                tool: ReferenceTool::Best,
            },
        )
        .expect_err("the range is out of bounds");
        assert_eq!(
            error.to_string(),
            "Choose depths 1-10,000 spanning at most 1,001 floors."
        );
    }
}
