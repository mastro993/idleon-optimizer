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

use idleon_import::import;
use idleon_spelunking::{
    Account, ConfigError, DepthRange, ReferenceTool, Requested, RunOptions, RunResult, Save,
    Status, normalize, profile, sim::Run,
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
fn inputs(oracle: &Value) -> idleon_spelunking::Inputs {
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
        amber: number(&account["amber"]),
        regen: number(&account["regen"]),
        grand: number(&account["grand"]),
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

fn indices(value: &Value) -> Vec<usize> {
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
    assert_eq!(inputs.memorized, indices(&oracle["memorized"]));
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
    close(
        inputs.nonconsume,
        number(&oracle["nonconsume"]),
        "nonconsume",
    );
    assert_eq!(inputs.overuse, oracle["overuse"].as_bool().unwrap());
    assert_eq!(inputs.switch_nova, oracle["switchNova"].as_bool().unwrap());
    close(inputs.bags, number(&oracle["bags"]), "bags");
    close(inputs.prisma, number(&oracle["prisma"]), "prisma");
    close(inputs.exalt, number(&oracle["exalt"]), "exalt");
    assert_eq!(inputs.shortcut, oracle["shortcut"].as_bool().unwrap());
    for (index, expected) in oracle["bosses"].as_array().unwrap().iter().enumerate() {
        close(inputs.bosses[index], number(expected), "bosses");
    }
    for (index, expected) in oracle["grands"].as_array().unwrap().iter().enumerate() {
        close(inputs.grands[index], number(expected), "grands");
    }
    let discoveries: Vec<String> = oracle["discoveries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap().to_string())
        .collect();
    assert_eq!(inputs.discoveries, discoveries);
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
        assert_eq!(profile.config.party, indices(&expected_config["party"]));
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
            indices(&expected_config["memorized"])
        );
        assert_eq!(
            profile.config.tool,
            match &expected_config["tool"] {
                Value::String(_) => idleon_spelunking::ToolChoice::Adaptive,
                Value::Number(id) => {
                    idleon_spelunking::ToolChoice::Tool(id.as_u64().unwrap() as usize)
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
        match (&profile.config.amber, &expected_config["amber"]) {
            (None, Value::Null) => {}
            (Some(amber), Value::Object(_)) => {
                compare_amber_policy(amber, &expected_config["amber"], label)
            }
            other => panic!("{label}: unexpected amber policy {other:?}"),
        }
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

#[test]
fn every_simulated_run_matches_the_upstream_calculator() {
    let oracle = oracle();
    for entry in oracle["runs"].as_array().unwrap() {
        let label = entry["label"].as_str().unwrap();
        let mut inputs = inputs(&oracle);
        // The unbeaten-boss run clears the saved boss, Grand and discovery
        // state and buys the nova, potion and charge upgrades, so the run
        // exercises the branches the fixture save has already finished.
        if entry["input"].as_str() == Some("unbeaten") {
            inputs.bosses.iter_mut().for_each(|boss| *boss = 0.0);
            inputs.grands.iter_mut().for_each(|grand| *grand = 0.0);
            inputs.discoveries.clear();
            for index in [36, 39, 40] {
                inputs.shop[index] = 1.0;
            }
        }
        // The weak run lowers the account Power so the Amber burn finds rocks
        // this character cannot break.
        if entry["input"].as_str() == Some("weak") {
            inputs.power = 25.0;
        }
        let requested: Requested =
            serde_json::from_value(entry["requested"].clone()).expect("the request decodes");
        let config =
            normalize(&inputs, &requested).unwrap_or_else(|error| panic!("{label}: {error}"));
        let expected_config = &entry["config"];
        assert_eq!(
            config.cave,
            expected_config["cave"].as_u64().unwrap() as usize
        );
        assert_eq!(config.party, indices(&expected_config["party"]));
        assert_eq!(
            config.policy.id(),
            expected_config["policy"].as_str().unwrap()
        );
        assert_eq!(
            config.entry.id(),
            expected_config["entry"].as_str().unwrap()
        );
        assert_eq!(
            config.full_clear,
            expected_config["fullClear"].as_bool().unwrap()
        );
        match (&config.amber, &expected_config["amber"]) {
            (None, Value::Null) => {}
            (Some(amber), Value::Object(_)) => {
                compare_amber_policy(amber, &expected_config["amber"], label)
            }
            other => panic!("{label}: unexpected amber policy {other:?}"),
        }
        let seed = entry["seed"].as_u64().unwrap() as u32;
        let mut run = Run::new(&inputs, config, RunOptions { seed })
            .unwrap_or_else(|error| panic!("{label}: {error}"));
        while !run.done() {
            run.step(5_000);
        }
        let actual = run.result();
        compare_run(&actual, entry, label);
    }
}

/// Compare the Amber policy a configuration carries against the upstream object.
fn compare_amber_policy(actual: &idleon_spelunking::AmberPolicy, expected: &Value, label: &str) {
    assert_eq!(
        actual.dump_depth as u64,
        expected["dumpDepth"].as_u64().unwrap(),
        "{label}: amber dumpDepth"
    );
    assert_eq!(
        actual.finishers as u64,
        expected["finishers"].as_u64().unwrap(),
        "{label}: amber finishers"
    );
    assert_eq!(
        actual.reserve_pct as u64,
        expected["reservePct"].as_u64().unwrap(),
        "{label}: amber reservePct"
    );
    assert_eq!(
        actual.harvest_depths as u64,
        expected["harvestDepths"].as_u64().unwrap(),
        "{label}: amber harvestDepths"
    );
    assert_eq!(
        actual.rope_reserve_pct as u64,
        expected["ropeReservePct"].as_u64().unwrap(),
        "{label}: amber ropeReservePct"
    );
}

/// Compare the Amber policy's run record against the upstream object.
fn compare_amber_result(actual: &idleon_spelunking::AmberResult, expected: &Value, label: &str) {
    assert_eq!(
        actual.phase.id(),
        expected["phase"].as_str().unwrap(),
        "{label}: amber phase"
    );
    close(
        actual.dump_spent,
        number(&expected["dumpSpent"]),
        &format!("{label} dumpSpent"),
    );
    assert_eq!(
        actual.dump_digs,
        expected["dumpDigs"].as_u64().unwrap(),
        "{label}: dumpDigs"
    );
    let transitions = expected["transitions"].as_array().unwrap();
    assert_eq!(
        actual.transitions.len(),
        transitions.len(),
        "{label}: amber transitions"
    );
    for (transition, upstream) in actual.transitions.iter().zip(transitions) {
        let path = format!("{label} amber transition");
        assert_eq!(
            transition.phase.id(),
            upstream["phase"].as_str().unwrap(),
            "{path}: phase"
        );
        assert_eq!(
            transition.reason,
            upstream["reason"].as_str().unwrap(),
            "{path}: reason"
        );
        assert_eq!(
            transition.depth as u64,
            upstream["depth"].as_u64().unwrap(),
            "{path}: depth"
        );
        close(transition.spent, number(&upstream["spent"]), &path);
        close(transition.time, number(&upstream["time"]), &path);
    }
    let activations = expected["activations"].as_array().unwrap();
    assert_eq!(
        actual.activations.len(),
        activations.len(),
        "{label}: amber activations"
    );
    for (activation, upstream) in actual.activations.iter().zip(activations) {
        let path = format!("{label} amber activation");
        assert_eq!(
            activation.depth as u64,
            upstream["depth"].as_u64().unwrap(),
            "{path}: depth"
        );
        close(activation.spent, number(&upstream["spent"]), &path);
        close(activation.effect, number(&upstream["effect"]), &path);
        close(activation.time, number(&upstream["time"]), &path);
        assert_eq!(
            activation.slot as u64,
            upstream["slot"].as_u64().unwrap(),
            "{path}: slot"
        );
    }
}

/// Compare one ported run against the upstream result, field for field.
fn compare_run(actual: &RunResult, expected: &Value, label: &str) {
    assert_eq!(actual.seed, expected["seed"].as_u64().unwrap() as u32);
    assert_eq!(
        actual.status.id(),
        expected["status"].as_str().unwrap(),
        "{label}: status"
    );
    assert!(actual.status != Status::Running, "{label}: still running");
    assert_eq!(
        actual.settled,
        expected["settled"].as_bool().unwrap(),
        "{label}: settled"
    );
    close(
        actual.seconds,
        number(&expected["seconds"]),
        &format!("{label} seconds"),
    );
    assert_eq!(
        actual.start_depth as u64,
        expected["startDepth"].as_u64().unwrap(),
        "{label}: startDepth"
    );
    assert_eq!(
        actual.depth as u64,
        expected["depth"].as_u64().unwrap(),
        "{label}: depth"
    );
    assert_eq!(
        actual.actions,
        expected["actions"].as_u64().unwrap(),
        "{label}: actions"
    );
    close(
        actual.spent,
        number(&expected["spent"]),
        &format!("{label} spent"),
    );
    assert_eq!(
        actual.hits,
        expected["hits"].as_u64().unwrap(),
        "{label}: hits"
    );
    assert_eq!(
        actual.misses,
        expected["misses"].as_u64().unwrap(),
        "{label}: misses"
    );
    assert_eq!(
        actual.broken,
        expected["broken"].as_u64().unwrap(),
        "{label}: broken"
    );
    for index in 0..12 {
        close(
            actual.rewards[index],
            number(&expected["rewards"][index]),
            &format!("{label} rewards[{index}]"),
        );
        close(
            actual.gross_rewards[index],
            number(&expected["grossRewards"][index]),
            &format!("{label} grossRewards[{index}]"),
        );
    }
    match (&actual.exit, &expected["exit"]) {
        (None, Value::Null) => {}
        (Some(exit), Value::Object(_)) => {
            let upstream = &expected["exit"];
            assert_eq!(
                exit.kind.id(),
                upstream["type"].as_str().unwrap(),
                "{label}: exit kind"
            );
            close(
                exit.retained,
                number(&upstream["retained"]),
                &format!("{label} retained"),
            );
            close(
                exit.amber_multiplier,
                upstream["amberMultiplier"].as_f64().unwrap_or(1.0),
                &format!("{label} amberMultiplier"),
            );
            let items = upstream["items"].as_array().unwrap();
            assert_eq!(exit.items.len(), items.len(), "{label}: exit items");
            for (item, upstream) in exit.items.iter().zip(items) {
                assert_eq!(item.reward, upstream["reward"].as_u64().unwrap() as usize);
                close(
                    item.gross,
                    number(&upstream["gross"]),
                    &format!("{label} item gross"),
                );
                close(
                    item.expected,
                    number(&upstream["expected"]),
                    &format!("{label} item expected"),
                );
                close(
                    item.collected,
                    number(&upstream["collected"]),
                    &format!("{label} item collected"),
                );
                assert_eq!(
                    item.rounded_up,
                    upstream["roundedUp"].as_bool(),
                    "{label}: roundedUp"
                );
                match (&item.chance, &upstream["chance"]) {
                    (None, Value::Null) => {}
                    (Some(chance), Value::Number(_)) => close(
                        *chance,
                        number(&upstream["chance"]),
                        &format!("{label} chance"),
                    ),
                    other => panic!("{label}: unexpected chance {other:?}"),
                }
            }
        }
        other => panic!("{label}: unexpected exit {other:?}"),
    }
    let stamina = expected["stamina"].as_array().unwrap();
    assert_eq!(actual.stamina.len(), stamina.len(), "{label}: stamina rows");
    for (member, upstream) in actual.stamina.iter().zip(stamina) {
        assert_eq!(member.id, upstream["id"].as_u64().unwrap() as usize);
        close(
            member.start,
            number(&upstream["start"]),
            &format!("{label} stamina start"),
        );
        close(
            member.end,
            number(&upstream["end"]),
            &format!("{label} stamina end"),
        );
        close(
            member.max,
            number(&upstream["max"]),
            &format!("{label} stamina max"),
        );
    }
    let diagnostics = &expected["diagnostics"];
    assert_eq!(
        actual.diagnostics.resist_stall,
        diagnostics["resistStall"].as_bool().unwrap(),
        "{label}: resistStall"
    );
    assert_eq!(
        actual.diagnostics.unused,
        diagnostics["unused"].as_u64().unwrap() as usize,
        "{label}: unused"
    );
    assert_eq!(
        actual.diagnostics.no_advance,
        diagnostics["noAdvance"].as_u64().unwrap() as usize,
        "{label}: noAdvance"
    );
    close(
        actual.diagnostics.resisted_stamina,
        number(&diagnostics["resistedStamina"]),
        &format!("{label} resistedStamina"),
    );
    assert_eq!(
        actual.diagnostics.blocked,
        diagnostics["blocked"].as_bool().unwrap(),
        "{label}: blocked"
    );
    let members = diagnostics["members"].as_array().unwrap();
    assert_eq!(
        actual.diagnostics.members.len(),
        members.len(),
        "{label}: member rows"
    );
    for (member, upstream) in actual.diagnostics.members.iter().zip(members) {
        let path = format!("{label} member {}", member.id);
        assert_eq!(member.id, upstream["id"].as_u64().unwrap() as usize);
        assert_eq!(
            member.start_depth,
            upstream["startDepth"].as_u64().map(|depth| depth as usize),
            "{path}: startDepth"
        );
        for (name, value, upstream) in [
            ("digs", member.digs, &upstream["digs"]),
            ("spent", member.spent, &upstream["spent"]),
            (
                "resistedStamina",
                member.resisted_stamina,
                &upstream["resistedStamina"],
            ),
            ("hits", member.hits, &upstream["hits"]),
            ("misses", member.misses, &upstream["misses"]),
            ("damage", member.damage, &upstream["damage"]),
            ("broken", member.broken, &upstream["broken"]),
            ("descents", member.descents, &upstream["descents"]),
            ("grands", member.grands, &upstream["grands"]),
        ] {
            close(value, number(upstream), &format!("{path} {name}"));
        }
        for index in 0..12 {
            close(
                member.rewards[index],
                number(&upstream["rewards"][index]),
                &format!("{path} rewards[{index}]"),
            );
        }
        for (name, value, upstream) in [
            ("unused", member.unused, &upstream["unused"]),
            ("exhausted", member.exhausted, &upstream["exhausted"]),
            ("noAdvance", member.no_advance, &upstream["noAdvance"]),
            (
                "noFurtherFloor",
                member.no_further_floor,
                &upstream["noFurtherFloor"],
            ),
            ("resistStall", member.resist_stall, &upstream["resistStall"]),
        ] {
            assert_eq!(value, upstream.as_bool().unwrap(), "{path}: {name}");
        }
    }
    close(
        actual.grands,
        number(&expected["grands"]),
        &format!("{label} grands"),
    );
    match (&actual.amber, &expected["amber"]) {
        (None, Value::Null) => {}
        (Some(amber), Value::Object(_)) => compare_amber_result(amber, &expected["amber"], label),
        other => panic!("{label}: unexpected amber record {other:?}"),
    }
    let discoveries: Vec<&str> = expected["discoveries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|name| name.as_str().unwrap())
        .collect();
    assert_eq!(
        actual
            .discoveries
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        discoveries,
        "{label}: discoveries"
    );
    assert_eq!(
        actual.rng_calls,
        expected["rngCalls"].as_u64().unwrap(),
        "{label}: rngCalls"
    );
    assert_eq!(
        actual.events_dropped,
        expected["eventsDropped"].as_u64().unwrap() as usize,
        "{label}: eventsDropped"
    );
    let events: Vec<(&str, usize)> = expected["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|event| {
            (
                event[0].as_str().unwrap(),
                event[1].as_u64().unwrap() as usize,
            )
        })
        .collect();
    let actual_events: Vec<(&str, usize)> = actual
        .events
        .iter()
        .map(|event| (event.kind.id(), event.depth))
        .collect();
    assert_eq!(actual_events, events, "{label}: event trace");
    let notes: Vec<&str> = expected["notes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|note| note.as_str().unwrap())
        .collect();
    assert_eq!(
        actual.notes.iter().map(String::as_str).collect::<Vec<_>>(),
        notes,
        "{label}: notes"
    );
    assert_eq!(actual.error, None, "{label}: error");
}
