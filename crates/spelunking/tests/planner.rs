//! Optimizer parity against the upstream planner and the search invariants.
//!
//! `fixtures/spelunking.json` is written by `script/spelunking-oracle.mjs` from
//! `spelunk-planner.js`, so the exhaustive configuration search, the party
//! prefixes and the bounded Amber phase search are compared field for field
//! with the upstream generator's own output.

use idleon_common::Control;
use idleon_import::import;
use idleon_spelunking::{
    Account, AmberSearchOptions, Clear, Enabled, Entry, Inputs, Mode, Objective, Policy, Requested,
    Save, ScopeRequest, SearchOptions, SearchResult, ToolChoice, amber_search, normalize,
    plan_text, result_text, search, search_space,
};
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/spelunking.json")).expect("oracle JSON")
}

fn inputs(oracle: &Value) -> Inputs {
    let imported = import(&read("fixtures/data.json")).expect("the fixture imports");
    let save = Save::from_parts(&imported.save).expect("the fixture carries Spelunk rows");
    let max_stamina: Vec<f64> = oracle["characters"]
        .as_array()
        .expect("characters")
        .iter()
        .map(|character| number(&character["max"]))
        .collect();
    let account = &oracle["account"];
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

fn baseline(value: &Value) -> Requested {
    let value = &value["optimizerSearchSpace"]["baseline"];
    Requested {
        cave: Some(value["cave"].as_u64().expect("cave") as usize),
        party: Some(indices(&value["party"])),
        manic: value["manic"].as_bool(),
        policy: value["policy"].as_str().and_then(Policy::from_id),
        speed: Some(value["speed"].as_u64().expect("speed") as u32),
        elixirs: Some(
            value["elixirs"]
                .as_array()
                .expect("elixirs")
                .iter()
                .map(|id| id.as_i64().expect("an elixir id") as i32)
                .collect(),
        ),
        memorized: Some(indices(&value["memorized"])),
        tool: value["tool"]
            .as_str()
            .and_then(ToolChoice::from_id)
            .or_else(|| {
                value["tool"]
                    .as_u64()
                    .map(|id| ToolChoice::Tool(id as usize))
            }),
        entry: value["entry"].as_str().and_then(Entry::from_id),
        full_clear: value["fullClear"].as_bool(),
        amber: value["amber"]
            .is_object()
            .then(|| serde_json::from_value(value["amber"].clone()).expect("an amber policy")),
        reaction_ms: value["reactionMs"].as_f64(),
        stamina: value["stamina"]
            .as_str()
            .and_then(idleon_spelunking::StaminaMode::from_id),
    }
}

fn scope(value: &Value) -> ScopeRequest {
    let enabled = &value["enabled"];
    ScopeRequest {
        enabled: Some(Enabled {
            party: enabled["party"].as_bool().unwrap_or(true),
            modes: enabled["modes"].as_bool().unwrap_or(true),
            clears: enabled["clears"].as_bool().unwrap_or(true),
            speeds: enabled["speeds"].as_bool().unwrap_or(true),
            elixirs: enabled["elixirs"].as_bool().unwrap_or(true),
            memorized: enabled["memorized"].as_bool().unwrap_or(true),
            tools: enabled["tools"].as_bool().unwrap_or(true),
            entry: enabled["entry"].as_bool().unwrap_or(true),
        }),
        modes: value["modes"].as_array().map(|modes| {
            modes
                .iter()
                .map(|mode| match mode.as_str().expect("a mode") {
                    "auto" => Mode::Auto,
                    _ => Mode::Manual,
                })
                .collect()
        }),
        clears: value["clears"].as_array().map(|clears| {
            clears
                .iter()
                .map(|clear| match clear {
                    Value::Bool(false) => Clear::First,
                    Value::Bool(true) => Clear::Full,
                    _ => Clear::Fifth,
                })
                .collect()
        }),
        speeds: value["speeds"].as_array().map(|speeds| {
            speeds
                .iter()
                .map(|speed| speed.as_u64().expect("a speed") as u32)
                .collect()
        }),
        ..ScopeRequest::default()
    }
}

fn options(value: &Value) -> SearchOptions {
    let value = &value["optimizerSearchSpace"]["options"];
    SearchOptions {
        objective: value["objective"]
            .as_str()
            .and_then(Objective::from_id)
            .expect("an objective"),
        screening: value["screening"].as_u64().expect("screening") as usize,
        trials: value["trials"].as_u64().expect("trials") as usize,
        seed: value["seed"].as_u64().expect("seed") as u32,
        scope: scope(&value["scope"]),
    }
}

fn assert_search(input: &Inputs, result: &SearchResult, expected: &Value) {
    assert_eq!(
        result.kind,
        expected["kind"].as_str().expect("kind"),
        "kind"
    );
    assert_eq!(
        result.objective.id(),
        expected["objective"].as_str().expect("objective")
    );
    assert_eq!(
        result.screened,
        expected["screened"]
            .as_str()
            .expect("screened")
            .parse()
            .expect("a count")
    );
    assert_eq!(result.exhaustive, expected["exhaustive"].as_bool().unwrap());
    close(
        result.delta.expect("a delta"),
        number(&expected["delta"]),
        "delta",
    );
    let interval = result.interval.expect("an interval");
    close(
        interval[0],
        number(&expected["interval"][0]),
        "interval.low",
    );
    close(
        interval[1],
        number(&expected["interval"][1]),
        "interval.high",
    );
    assert_row(input, &result.baseline, &expected["baseline"], "baseline");
    assert_row(
        input,
        result.winner.as_ref().expect("a winner"),
        &expected["winner"],
        "winner",
    );
    assert_eq!(
        result.rows.len(),
        expected["rows"].as_array().unwrap().len()
    );
    for (index, (row, expected)) in result
        .rows
        .iter()
        .zip(expected["rows"].as_array().unwrap())
        .enumerate()
    {
        assert_row(input, row, expected, &format!("rows[{index}]"));
    }
}

fn assert_row(input: &Inputs, row: &idleon_spelunking::OptimizerRow, expected: &Value, path: &str) {
    assert_eq!(
        row.label,
        expected["label"].as_str().expect("label"),
        "{path}.label"
    );
    assert_eq!(
        row.reference_only,
        expected["referenceOnly"].as_bool().unwrap_or(false),
        "{path}.referenceOnly"
    );
    close(
        row.summary.score,
        number(&expected["score"]),
        &format!("{path}.score"),
    );
    close(
        row.summary.se,
        number(&expected["se"]),
        &format!("{path}.se"),
    );
    close(
        row.summary.depth,
        number(&expected["depth"]),
        &format!("{path}.depth"),
    );
    close(
        row.summary.start_depth,
        number(&expected["startDepth"]),
        &format!("{path}.startDepth"),
    );
    close(
        row.summary.amber,
        number(&expected["amber"]),
        &format!("{path}.amber"),
    );
    close(
        row.summary.seconds,
        number(&expected["seconds"]),
        &format!("{path}.seconds"),
    );
    assert_eq!(
        row.summary.settled,
        expected["settled"].as_u64().unwrap() as usize,
        "{path}.settled"
    );
    assert_eq!(
        row.summary.total,
        expected["total"].as_u64().unwrap() as usize,
        "{path}.total"
    );
    assert_eq!(
        row.summary.rope,
        expected["rope"].as_u64().unwrap() as usize,
        "{path}.rope"
    );
    assert_eq!(
        row.summary.toll,
        expected["toll"].as_u64().unwrap() as usize,
        "{path}.toll"
    );
    assert_eq!(
        row.summary.blocked,
        expected["blocked"].as_u64().unwrap() as usize,
        "{path}.blocked"
    );
    close(
        row.summary.recovery_hours,
        number(&expected["recoveryHours"]),
        &format!("{path}.recoveryHours"),
    );
    assert_config(
        &row.summary.config,
        &expected["config"],
        input,
        &format!("{path}.config"),
    );
    for (index, (run, expected)) in row
        .summary
        .runs
        .iter()
        .zip(expected["runs"].as_array().unwrap())
        .enumerate()
    {
        let path = format!("{path}.runs[{index}]");
        assert_eq!(
            run.seed,
            expected["seed"].as_u64().unwrap() as u32,
            "{path}.seed"
        );
        assert_eq!(
            run.status.id(),
            expected["status"].as_str().unwrap(),
            "{path}.status"
        );
        assert_eq!(
            run.settled,
            expected["settled"].as_bool().unwrap(),
            "{path}.settled"
        );
        assert_eq!(
            run.depth,
            expected["depth"].as_u64().unwrap() as usize,
            "{path}.depth"
        );
        assert_eq!(
            run.start_depth,
            expected["startDepth"].as_u64().unwrap() as usize,
            "{path}.startDepth"
        );
        close(
            run.seconds,
            number(&expected["seconds"]),
            &format!("{path}.seconds"),
        );
        close(
            run.spent,
            number(&expected["spent"]),
            &format!("{path}.spent"),
        );
        for (reward, expected) in run
            .rewards
            .iter()
            .zip(expected["rewards"].as_array().unwrap())
        {
            close(*reward, number(expected), &format!("{path}.rewards"));
        }
        close(
            run.grands,
            number(&expected["grands"]),
            &format!("{path}.grands"),
        );
        for (member, expected) in run
            .stamina
            .iter()
            .zip(expected["stamina"].as_array().unwrap())
        {
            assert_eq!(member.id, expected["id"].as_u64().unwrap() as usize);
            close(
                member.start,
                number(&expected["start"]),
                &format!("{path}.stamina.start"),
            );
            close(
                member.end,
                number(&expected["end"]),
                &format!("{path}.stamina.end"),
            );
        }
        match (&run.exit, expected["exit"].as_object()) {
            (None, None) => {}
            (Some(exit), Some(expected)) => {
                assert_eq!(exit.kind.id(), expected["type"].as_str().unwrap());
            }
            _ => panic!("{path}.exit: {:?} != {expected}", run.exit),
        }
        match (&run.amber, expected["amber"].as_object()) {
            (None, None) => {}
            (Some(amber), Some(expected)) => {
                assert_eq!(amber.phase.id(), expected["phase"].as_str().unwrap());
                close(
                    amber.dump_spent,
                    number(&expected["dumpSpent"]),
                    &format!("{path}.amber.dumpSpent"),
                );
                assert_eq!(
                    amber.dump_digs,
                    expected["dumpDigs"].as_u64().unwrap(),
                    "{path}.amber.dumpDigs"
                );
                for (activation, expected) in amber
                    .activations
                    .iter()
                    .zip(expected["activations"].as_array().unwrap())
                {
                    assert_eq!(
                        activation.depth,
                        expected["depth"].as_u64().unwrap() as usize
                    );
                    close(
                        activation.effect,
                        number(&expected["effect"]),
                        &format!("{path}.amber.effect"),
                    );
                }
            }
            _ => panic!("{path}.amber: {:?} != {expected}", run.amber),
        }
    }
    match (&row.party_analysis, expected["partyAnalysis"].as_object()) {
        (None, None) => {}
        (Some(party), Some(expected)) => {
            close(
                party.retention,
                number(&expected["retention"]),
                &format!("{path}.party.retention"),
            );
            assert_eq!(
                party.recommended_size,
                expected["recommendedSize"]
                    .as_u64()
                    .map(|size| size as usize),
                "{path}.party.recommendedSize"
            );
            assert_eq!(
                party.trials,
                expected["trials"].as_u64().unwrap() as usize,
                "{path}.party.trials"
            );
            for (prefix, expected) in party.rows.iter().zip(expected["rows"].as_array().unwrap()) {
                close(
                    prefix.score,
                    number(&expected["score"]),
                    &format!("{path}.party.score"),
                );
                close(
                    prefix.depth,
                    number(&expected["depth"]),
                    &format!("{path}.party.depth"),
                );
                close(
                    prefix.amber,
                    number(&expected["amber"]),
                    &format!("{path}.party.amber"),
                );
                assert_eq!(prefix.allowed, expected["allowed"].as_bool().unwrap());
                match (&prefix.marginal, expected["marginal"].as_object()) {
                    (None, None) => {}
                    (Some(paired), Some(expected)) => {
                        close(
                            paired.objective,
                            number(&expected["objective"]),
                            &format!("{path}.party.marginal"),
                        );
                        close(
                            paired.depth,
                            number(&expected["depth"]),
                            &format!("{path}.party.marginal.depth"),
                        );
                        close(
                            paired.extra_depth as f64,
                            number(&expected["extraDepth"]),
                            &format!("{path}.party.marginal.extraDepth"),
                        );
                        match (&paired.interval, expected["interval"].as_array()) {
                            (None, None) => {}
                            (Some(interval), Some(expected)) => {
                                close(
                                    interval[0],
                                    number(&expected[0]),
                                    &format!("{path}.party.interval"),
                                );
                                close(
                                    interval[1],
                                    number(&expected[1]),
                                    &format!("{path}.party.interval"),
                                );
                            }
                            _ => panic!(
                                "{path}.party.marginal.interval: {:?} != {expected:?}",
                                paired.interval
                            ),
                        }
                    }
                    _ => panic!("{path}.party.marginal: {:?} != {expected}", prefix.marginal),
                }
            }
        }
        _ => panic!(
            "{path}.partyAnalysis: {:?} != {expected:?}",
            row.party_analysis
        ),
    }
}

fn assert_config(actual: &idleon_spelunking::Config, expected: &Value, input: &Inputs, path: &str) {
    let requested = Requested {
        cave: Some(expected["cave"].as_u64().expect("cave") as usize),
        party: Some(indices(&expected["party"])),
        manic: Some(expected["manic"].as_bool().expect("manic")),
        policy: expected["policy"].as_str().and_then(Policy::from_id),
        speed: Some(expected["speed"].as_u64().expect("speed") as u32),
        elixirs: Some(
            expected["elixirs"]
                .as_array()
                .expect("elixirs")
                .iter()
                .map(|id| id.as_i64().expect("an elixir") as i32)
                .collect(),
        ),
        memorized: Some(indices(&expected["memorized"])),
        tool: expected["tool"]
            .as_str()
            .and_then(ToolChoice::from_id)
            .or_else(|| {
                expected["tool"]
                    .as_u64()
                    .map(|id| ToolChoice::Tool(id as usize))
            }),
        entry: expected["entry"].as_str().and_then(Entry::from_id),
        full_clear: Some(expected["fullClear"].as_bool().expect("fullClear")),
        amber: expected["amber"]
            .is_object()
            .then(|| serde_json::from_value(expected["amber"].clone()).expect("amber")),
        reaction_ms: Some(number(&expected["reactionMs"])),
        stamina: expected["stamina"]
            .as_str()
            .and_then(idleon_spelunking::StaminaMode::from_id),
    };
    let expected = normalize(input, &requested).expect("the expected config normalises");
    assert_eq!(actual, &expected, "{path}");
}

#[test]
fn the_search_space_matches_upstream() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let space = &oracle["optimizerSearchSpace"];
    let (count, actual_scope) = search_space(&inputs, &baseline(&oracle), &scope(&space["scope"]))
        .expect("the baseline and scope are valid");
    assert_eq!(count.to_string(), space["configurations"].as_str().unwrap());
    assert_eq!(
        actual_scope.party_min,
        space["scope"]["partyMin"].as_u64().unwrap() as usize
    );
    assert_eq!(
        actual_scope.party_max,
        space["scope"]["partyMax"].as_u64().unwrap() as usize
    );
    assert_eq!(
        actual_scope.characters,
        indices(&space["scope"]["characters"])
    );
    assert!(!actual_scope.ignore_order);
    assert_eq!(actual_scope.elixirs, indices(&space["scope"]["elixirs"]));
    assert_eq!(
        actual_scope.memorized,
        indices(&space["scope"]["memorized"])
    );
}

#[test]
fn the_configuration_search_matches_upstream() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let result = search(&inputs, &baseline(&oracle), &options(&oracle), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    assert_search(&inputs, &result, &oracle["optimizerSearch"]);
    assert!(
        result
            .winner
            .as_ref()
            .is_some_and(|winner| winner.summary.score >= result.baseline.summary.score)
    );
}

#[test]
fn the_amber_search_matches_upstream() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let expected = &oracle["optimizerAmber"];
    let options = AmberSearchOptions {
        objective: expected["objective"]
            .as_str()
            .and_then(Objective::from_id)
            .expect("an objective"),
        screening: expected["info"]["screening"].as_u64().unwrap() as usize,
        trials: expected["info"]["initialTrials"].as_u64().unwrap() as usize,
        max_trials: expected["info"]["maxTrials"].as_u64().unwrap() as usize,
        min_rope_pct: expected["info"]["minRopePct"].as_u64().unwrap() as usize,
        from: expected["info"]["from"].as_u64().unwrap() as usize,
        to: Some(expected["info"]["to"].as_u64().unwrap() as usize),
        seed: 1,
    };
    let result = amber_search(&inputs, &baseline(&oracle), &options, &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    assert_search(&inputs, &result, expected);
    let info = result.amber_search.expect("amber search details");
    let expected_info = &expected["info"];
    assert_eq!(info.from, expected_info["from"].as_u64().unwrap() as usize);
    assert_eq!(info.to, expected_info["to"].as_u64().unwrap() as usize);
    assert_eq!(
        info.effective_from,
        expected_info["effectiveFrom"].as_u64().unwrap() as usize
    );
    assert_eq!(info.depths, indices(&expected_info["depths"]));
    assert_eq!(
        info.min_rope_pct,
        expected_info["minRopePct"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.screening,
        expected_info["screening"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.trials,
        expected_info["trials"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.initial_trials,
        expected_info["initialTrials"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.max_trials,
        expected_info["maxTrials"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.coarse,
        expected_info["coarse"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.refined,
        expected_info["refined"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.racing_trials,
        expected_info["racingTrials"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.adaptive_rounds,
        expected_info["adaptiveRounds"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.observed_behaviors,
        expected_info["observedBehaviors"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.finalist_strategies,
        expected_info["finalistStrategies"].as_u64().unwrap() as usize
    );
    assert_eq!(
        info.unresolved,
        expected_info["unresolved"].as_bool().unwrap()
    );
    assert_eq!(
        info.risk_uncertain,
        expected_info["riskUncertain"].as_bool().unwrap()
    );
}

#[test]
fn the_search_is_reproducible_and_cancellable() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let first = search(&inputs, &baseline(&oracle), &options(&oracle), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    let second = search(&inputs, &baseline(&oracle), &options(&oracle), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    assert_eq!(first.rows, second.rows);
    assert_eq!(first.winner, second.winner);
    let cancelled = search(&inputs, &baseline(&oracle), &options(&oracle), &mut |_| {
        Control::Stop
    })
    .expect("a cancelled search is not an error");
    assert!(cancelled.is_none());
}

#[test]
fn a_belt_scope_enumerates_the_count_it_promises() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let enabled = Enabled {
        party: false,
        modes: false,
        clears: false,
        speeds: false,
        memorized: false,
        tools: false,
        entry: false,
        ..Enabled::default()
    };
    let scope = ScopeRequest {
        enabled: Some(enabled),
        elixirs: Some(vec![0, 1]),
        clears: Some(vec![Clear::First]),
        ..ScopeRequest::default()
    };
    let options = SearchOptions {
        objective: Objective::Depth,
        screening: 1,
        trials: 2,
        seed: 1,
        scope,
    };
    let result = search(&inputs, &baseline(&oracle), &options, &mut |_| {
        Control::Continue
    })
    .expect("the belt search runs")
    .expect("the belt search is not cancelled");
    // Two elixir ids fill the five physical slots under the duplicate limit: 2^5.
    assert_eq!(result.configurations, 32);
    // The saved belt is outside the pool, so the baseline is the one reference
    // measured on top of the exhaustive count.
    assert_eq!(result.screened, 33);
    assert!(!result.rows.is_empty());
}

#[test]
fn the_plan_text_names_the_recommendation() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let result = search(&inputs, &baseline(&oracle), &options(&oracle), &mut |_| {
        Control::Continue
    })
    .expect("the search runs")
    .expect("the search is not cancelled");
    let winner = result.winner.as_ref().expect("a winner");
    let plan = plan_text(&inputs, &winner.summary.config);
    assert!(plan.contains("Pebble Cove"));
    assert!(plan.contains("Character 1"));
    assert!(plan.contains("belt"));
    let text = result_text(&inputs, &result);
    assert!(text.contains("Best measured"));
    assert!(text.contains(&plan));
}

#[test]
fn the_amber_search_needs_an_orange_elixir() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let mut requested = baseline(&oracle);
    requested.elixirs = Some(vec![-1; 5]);
    let error = amber_search(
        &inputs,
        &requested,
        &AmberSearchOptions::default(),
        &mut |_| Control::Continue,
    )
    .expect_err("the belt has no orange");
    assert_eq!(
        error.to_string(),
        "Amber farming needs at least one equipped orange elixir. Apply the Amber belt preset or choose a belt."
    );
}

#[test]
fn a_large_party_scope_counts_every_combination() {
    let oracle = oracle();
    let inputs = inputs(&oracle);
    let scope = ScopeRequest {
        enabled: Some(Enabled::default()),
        party_min: Some(1),
        party_max: Some(2),
        characters: Some(vec![0, 1, 2]),
        ..ScopeRequest::default()
    };
    let (count, _) = search_space(&inputs, &baseline(&oracle), &scope).expect("a valid scope");
    // 3 ordered singles + 6 ordered pairs, every other dimension at its default.
    assert!(count > 9, "the count includes the unfixed dimensions");
}
