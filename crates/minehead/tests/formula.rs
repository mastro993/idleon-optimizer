//! The Minehead formulas, simulation and searches against the upstream oracle
//! in `tests/fixtures/upstream.json`.
//!
//! The fixture comes from `script/minehead-oracle.mjs`, which imports the
//! upstream JS and runs it on the same inputs.

use idleon_minehead::data::game;
use idleon_minehead::infer::{self, Recorded, Spatial};
use idleon_minehead::math::{
    self, CurrencyInputs, DamageInputs, ShapeContext, ShapeKind, ShapeValue,
};
use idleon_minehead::optimize::{Grid, get, optimize, set};
use idleon_minehead::sim::{Decision, Options, monte_carlo};
use idleon_minehead::{Ctx, Params, Rng};
use serde_json::Value;

fn oracle() -> Value {
    serde_json::from_str(include_str!("fixtures/upstream.json")).expect("the oracle fixture")
}

fn levels() -> Vec<f64> {
    // The fixture save's Research row 8, which the oracle read too.
    let save: Value = serde_json::from_str(include_str!("../../../fixtures/data.json"))
        .expect("the save fixture");
    let rows: Vec<Vec<Value>> =
        serde_json::from_str(save["Research"].as_str().expect("the research string"))
            .expect("the research rows");
    rows[8]
        .iter()
        .map(|value| value.as_f64().unwrap_or(0.0))
        .collect()
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

fn close(actual: f64, expected: f64, tolerance: f64) -> bool {
    (actual - expected).abs() <= tolerance.max(expected.abs() * 1e-9)
}

#[test]
fn the_formula_table_matches_the_oracle() {
    let fixture = oracle();
    let formulas = &fixture["formulas"];
    let data = game();
    let levels = levels();
    let qty = levels.clone();

    for (t, expected) in formulas["upgLvReq"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        assert_eq!(math::upg_lv_req(t), expected.as_u64().unwrap_or(0) as u32);
    }
    for (t, expected) in formulas["upgradeQTY"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let actual = math::upgrade_qty(data, t, levels[t], 1.0);
        assert!(close(actual, number(expected), 1e-9), "upgradeQTY {t}");
    }
    for (t, expected) in formulas["qty"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let actual = math::upgrade_qty(data, t, levels[t], 2.0);
        assert!(close(actual, number(expected), 1e-9), "doubled {t}");
    }
    for (t, expected) in formulas["costs"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let actual = math::upg_cost(data, t, levels[t], levels[26], 1.25, 12.0);
        assert!(
            close(actual, number(expected), 1e-6),
            "cost {t}: {actual} vs {}",
            number(expected)
        );
    }
    for (t, expected) in formulas["canBuy"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let actual = math::can_buy(data, t, levels[t], 5e9, levels[26], 1.25, 12.0);
        assert_eq!(actual, expected.as_bool().unwrap_or(false), "canBuy {t}");
    }
    for (index, expected) in formulas["gridDims"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let (cols, rows) = math::grid_dims(data, index);
        assert_eq!(
            cols as u64,
            expected["cols"].as_u64().unwrap_or(0),
            "cols {index}"
        );
        assert_eq!(
            rows as u64,
            expected["rows"].as_u64().unwrap_or(0),
            "rows {index}"
        );
    }
    assert!(close(
        math::gold_tiles_total(data, &qty, 3.0),
        number(&formulas["goldTiles"]),
        1e-9
    ));
    assert!(close(
        math::blocks_total(data, &qty),
        number(&formulas["blocks"]),
        1e-9
    ));
    assert!(close(
        math::flags_total(data, &qty),
        number(&formulas["flags"]),
        1e-9
    ));
    assert!(close(
        math::insta_reveals_total(data, &qty),
        number(&formulas["instas"]),
        1e-9
    ));
    assert!(close(
        math::daily_tries(4.0),
        number(&formulas["dailyTries"]),
        1e-9
    ));
    assert!(close(
        math::max_hp_you(data, &qty),
        number(&formulas["maxHP"]),
        1e-9
    ));
    for (floor, expected) in formulas["floors"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        assert!(
            close(math::floor_hp(floor, 1.15), number(&expected["hp"]), 1e-9),
            "hp {floor}"
        );
        assert!(
            close(
                math::mines_on_floor(floor, 1.0, 17.0),
                number(&expected["mines"]),
                1e-9
            ),
            "mines {floor}"
        );
    }
    assert!(close(
        math::base_dmg(data, &qty, 37.5, 3.0, 12.0, 8.0, 25.0),
        number(&formulas["baseDamage"]),
        1e-9
    ));
    assert!(close(
        math::bonus_dmg_per_tile_pct(data, &qty, 21.5),
        number(&formulas["perTilePct"]),
        1e-9
    ));
    assert!(close(
        math::bluecrown_multi(data, &qty),
        number(&formulas["bluecrownMulti"]),
        1e-9
    ));
    assert!(close(
        math::bluecrown_odds(data, &qty),
        number(&formulas["bluecrownOdds"]),
        1e-12
    ));
    assert!(close(
        math::jackpot_odds(data, &qty),
        number(&formulas["jackpotOdds"]),
        1e-12
    ));
    assert!(close(
        math::jackpot_tiles(data, &qty),
        number(&formulas["jackpotTiles"]),
        1e-9
    ));
    let damage = DamageInputs {
        grid_167: 37.5,
        grid_146: 21.5,
        wep_pow_dmg_pct: 10.0,
        sailing38: 3.0,
        dancing_coral_5: 12.0,
        jelly_damage_9: 8.0,
        legend40: 25.0,
    };
    for (index, expected) in formulas["outgoing"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        let value = [0u32, 2, 5, 9, 12, 14, 19, 20, 23, 30, 41][index];
        let actual = math::current_outgoing_dmg(data, &[value], 1, false, &qty, &damage);
        assert!(
            close(actual, number(expected), 1e-6),
            "outgoing {value}: {actual} vs {}",
            number(expected)
        );
    }
    assert!(close(
        math::wiggle_max_per_game(4.0),
        number(&formulas["wiggleMax"]),
        1e-9
    ));
    for (lv, expected) in formulas["glimbo"]
        .as_array()
        .expect("a table")
        .iter()
        .enumerate()
    {
        assert!(
            close(
                math::glimbo_cost(lv as f64, 1.4, 1.0),
                number(expected),
                1e-9
            ),
            "glimbo {lv}"
        );
    }
}

#[test]
fn currency_per_hour_matches_the_oracle() {
    let fixture = oracle();
    let levels = levels();
    let inputs = CurrencyInputs {
        grid_129: 100.0,
        grid_148: 25.0,
        grid_147: 10.0,
        grid_166: 6.0,
        comp_143: 1.5,
        comp_143_level2: 1.0,
        bonus_qty_6: 40.0,
        atom13: 2.0,
        meal_mine_curr: 12.0,
        arcade62: 30.0,
        rog_bonus_12: 8.0,
        button_bonus_1: 5.0,
        event_shop_44: 1.0,
        royal_currency_multi: 1.25,
        jelly_currency_8: 6.0,
        bundle_j: true,
        task_currency_level: 2.0,
        dancing_coral_5: 15.0,
        legend40: 25.0,
        levels: math::qty_levels(&levels, &[1, 22]),
        highest_dmg: 17_691_570_892_187.5,
    };
    let actual = math::currency_per_hour(game(), &inputs);
    assert!(
        close(actual, number(&fixture["currency"]), 1e-9),
        "{actual}"
    );
}

#[test]
fn the_shape_ranking_matches_the_oracle() {
    let fixture = oracle();
    let rows = fixture["shapeValues"].as_array().expect("a table");
    let bonus = |index: usize| number(&rows[index]["bonus"]);
    let per_tile = math::upgrade_qty(game(), 9, levels()[9], 1.0);
    let context = ShapeContext {
        grid: [
            bonus(0),
            bonus(1),
            bonus(2),
            bonus(3),
            bonus(4),
            bonus(5),
            bonus(6),
        ],
        meal_mine_curr: 12.0,
        button_no_grid: 15.0,
        button1: 15.0 * (1.0 + bonus(4) / 100.0),
        per_tile,
        shape_pct: 30.0,
        tiles: 5.0,
    };
    let values: Vec<ShapeValue> = math::grid_shape_values(&context);
    assert_eq!(values.len(), rows.len());
    for (actual, expected) in values.iter().zip(rows) {
        assert_eq!(actual.idx, expected["idx"].as_u64().unwrap_or(0) as usize);
        assert!(close(actual.bonus, number(&expected["bonus"]), 1e-9));
        assert!(close(
            actual.shaped_bonus,
            number(&expected["shapedBonus"]),
            1e-9
        ));
        assert!(
            close(actual.ratio, number(&expected["ratio"]), 1e-9),
            "ratio {}: {} vs {}",
            actual.idx,
            actual.ratio,
            number(&expected["ratio"])
        );
        let kind = if expected["kind"].as_str() == Some("currency") {
            ShapeKind::Currency
        } else {
            ShapeKind::Damage
        };
        assert_eq!(actual.kind, kind);
    }
    let priority: Vec<usize> = fixture["priority"]
        .as_array()
        .expect("a table")
        .iter()
        .map(|value| value.as_u64().unwrap_or(0) as usize)
        .collect();
    assert_eq!(math::shape_priority(&context), priority);
}

fn options(fixture: &Value) -> Options {
    let save: Value =
        serde_json::from_str(include_str!("../../../fixtures/data.json")).expect("the save");
    let rows: Vec<Vec<Value>> =
        serde_json::from_str(save["Research"].as_str().expect("rows")).expect("the research rows");
    let floor = rows[7][4].as_f64().unwrap_or(0.0) as usize;
    let _ = fixture;
    Options {
        floor,
        levels: levels(),
        doublers: Vec::new(),
        grid_bonus_166_1: 4.0,
        damage: DamageInputs {
            grid_167: 37.5,
            grid_146: 21.5,
            wep_pow_dmg_pct: 10.0,
            sailing38: 3.0,
            dancing_coral_5: 12.0,
            jelly_damage_9: 8.0,
            legend40: 25.0,
        },
        svar_hp: 1.15,
        max_turns: 200,
        mine_reduction: 1.0,
        legend40_pts: 17.0,
        jelly_gold_tiles45: 3.0,
    }
}

#[test]
fn the_seeded_simulation_matches_the_oracle() {
    let fixture = oracle();
    let options = options(&fixture);
    let params = Params::default();
    for run in fixture["monteCarlo"].as_array().expect("the runs") {
        let mut trial = options.clone();
        trial.floor = run["floor"].as_u64().unwrap_or(0) as usize;
        let seed = run["seed"].as_u64().unwrap_or(0) as u32;
        let trials = run["trials"].as_u64().unwrap_or(0) as u32;
        let actual = monte_carlo(&trial, &params, trials, seed);
        let expected = &run["result"];
        assert!(close(actual.win_rate, number(&expected["winRate"]), 1e-12));
        assert!(
            close(actual.avg_dmg, number(&expected["avgDmg"]), 1e-9),
            "avgDmg {} vs {}",
            actual.avg_dmg,
            number(&expected["avgDmg"])
        );
        assert!(close(actual.avg_turns, number(&expected["avgTurns"]), 1e-9));
        assert!(close(
            actual.avg_commits,
            number(&expected["avgCommits"]),
            1e-9
        ));
        assert!(close(
            actual.avg_mine_hits,
            number(&expected["avgMineHits"]),
            1e-9
        ));
        assert!(close(
            actual.avg_blocks_used,
            number(&expected["avgBlocksUsed"]),
            1e-9
        ));
        assert!(close(
            actual.avg_first_click_mines,
            number(&expected["avgFirstClickMines"]),
            1e-9
        ));
        assert!(close(
            actual.avg_first_turn_dmg,
            number(&expected["avgFirstTurnDmg"]),
            1e-9
        ));
        assert!(close(
            actual.avg_dmg_per_commit,
            number(&expected["avgDmgPerCommit"]),
            1e-9
        ));
        assert!(close(
            actual.timeout_rate,
            number(&expected["timeoutRate"]),
            1e-12
        ));
    }
}

fn params_from(value: &Value) -> Params {
    let mut params = Params::default();
    for (key, field) in [
        ("ev_multiplier", "evMultiplier"),
        ("min_reveal", "minReveal"),
        ("mine_cap_pct", "mineCapPct"),
        ("golden_mine_pct", "goldenMinePct"),
        ("hp_threshold", "hpThreshold"),
        ("block_aggro", "blockAggro"),
        ("commit_min", "commitMin"),
        ("life_aggro", "lifeAggro"),
        ("turn1_ev_mul", "turn1EvMul"),
        ("crown_chase", "crownChase"),
        ("commit_turns", "commitTurns"),
        ("insta_mode", "instaMode"),
        ("post_mine_panic", "postMinePanic"),
        ("last_turn_aggro", "lastTurnAggro"),
        ("target_reveals", "targetReveals"),
        ("golden_first", "goldenFirst"),
        ("crown_or_die", "crownOrDie"),
        ("hot_streak", "hotStreak"),
        ("block_hoard", "blockHoard"),
        ("corner_bias", "cornerBias"),
        ("edge_bias", "edgeBias"),
        ("cluster_bias", "clusterBias"),
    ] {
        let raw = match value.get(field) {
            Some(Value::Bool(flag)) => {
                if *flag {
                    1.0
                } else {
                    0.0
                }
            }
            Some(other) => number(other),
            None => continue,
        };
        set(&mut params, key, raw);
    }
    params
}

#[test]
fn the_optimizer_matches_the_oracle() {
    let fixture = oracle();
    let options = options(&fixture);
    let mut grid = Grid { knobs: Vec::new() };
    grid.knobs.push(("ev_multiplier", vec![0.8, 1.3]));
    grid.knobs.push(("min_reveal", vec![0.0, 1.0]));
    grid.knobs.push(("mine_cap_pct", vec![1.0]));
    grid.knobs.push(("golden_mine_pct", vec![1.0]));
    grid.knobs.push(("hp_threshold", vec![1.0]));
    grid.knobs.push(("block_aggro", vec![1.0]));
    grid.knobs.push(("commit_min", vec![0.0]));
    grid.knobs.push(("life_aggro", vec![1.0]));
    grid.knobs.push(("turn1_ev_mul", vec![0.0]));
    grid.knobs.push(("crown_chase", vec![0.0]));
    grid.knobs.push(("insta_mode", vec![0.0]));
    let expected = &fixture["optimized"];
    let result = optimize(&options, &grid, 20, 30, 2, 7, &mut |_| {
        idleon_common::Control::Continue
    })
    .expect("a result");
    assert_eq!(
        result.total_combos as u64,
        expected["totalCombos"].as_u64().unwrap_or(0)
    );
    assert_eq!(result.best.params, params_from(&expected["bestParams"]));
    assert!(close(
        result.best.result.avg_dmg,
        number(&expected["bestResult"]["avgDmg"]),
        1e-9
    ));
    assert!(close(
        result.best.result.win_rate,
        number(&expected["bestResult"]["winRate"]),
        1e-12
    ));
    let top = expected["topResults"].as_array().expect("the top results");
    assert_eq!(result.top.len(), top.len());
    for (actual, expected) in result.top.iter().zip(top) {
        assert_eq!(actual.params, params_from(&expected["params"]));
        assert!(close(
            actual.result.avg_dmg,
            number(&expected["result"]["avgDmg"]),
            1e-9
        ));
    }
}

#[test]
fn the_cost_path_spends_until_nothing_is_affordable() {
    let fixture = oracle();
    let options = options(&fixture);
    let inputs = idleon_minehead::PathInputs {
        currency: 1e12,
        research_level: 200,
        qty26: options.levels[26],
        server_var_cost: 1.0,
        rog_cost_pct: 0.0,
        max_steps: 3,
    };
    let path =
        idleon_minehead::cost_path(&options, &Params::default(), &inputs, 10, 7, &mut |_| {
            idleon_common::Control::Continue
        })
        .expect("a path");
    assert_eq!(path.steps.len(), 3);
    assert!(path.spent > 0.0);
    assert!(path.remaining_currency < inputs.currency);
    for step in &path.steps {
        assert!(path.final_levels[step.upg_idx] > options.levels[step.upg_idx]);
    }
    assert!(path.baseline.trials > 0);
}

fn recorded(value: &Value) -> Recorded {
    let ctx = &value;
    let spatial = value
        .get("spatial")
        .filter(|spatial| !spatial.is_null())
        .map(|spatial| Spatial {
            idx: spatial["idx"].as_u64().unwrap_or(0) as usize,
            row: spatial["r"].as_u64().unwrap_or(0) as usize,
            col: spatial["c"].as_u64().unwrap_or(0) as usize,
            cols: spatial["cols"].as_u64().unwrap_or(0) as usize,
            rows: spatial["rows"].as_u64().unwrap_or(0) as usize,
            is_corner: spatial["isCorner"].as_bool().unwrap_or(false),
            is_edge: spatial["isEdge"].as_bool().unwrap_or(false),
            is_center: spatial["isCenter"].as_bool().unwrap_or(false),
            adj_mines: spatial["adjMines"].as_u64().unwrap_or(0) as u32,
            adj_safe: spatial["adjSafe"].as_u64().unwrap_or(0) as u32,
            adj_unrevealed: spatial["adjUnrevealed"].as_u64().unwrap_or(0) as u32,
            click_order: spatial["clickOrder"].as_u64().unwrap_or(0) as u32,
        });
    let input = Ctx {
        safe_revealed_this_turn: ctx["safeRevealedThisTurn"].as_u64().unwrap_or(0) as u32,
        mines_total: number(&ctx["minesTotal"]),
        mines_remaining: number(&ctx["minesRemaining"]),
        unrevealed_count: number(&ctx["unrevealedCount"]),
        lives_left: number(&ctx["livesLeft"]),
        total_dmg_so_far: number(&ctx["totalDmgSoFar"]),
        boss_hp: number(&ctx["bossHP"]),
        turns_played: ctx["turnsPlayed"].as_u64().unwrap_or(0) as u32,
        blocks_left: number(&ctx["blocksLeft"]),
        goldens_on_grid: number(&ctx["goldensOnGrid"]),
        current_turn_dmg: number(&ctx["currentTurnDmg"]),
        remaining_hp: number(&ctx["remainingHP"]),
        per_tile_pct: number(&ctx["perTilePct"]),
        tile_count: ctx["tileCount"].as_u64().unwrap_or(0) as u32,
        crown_progress: ctx["crownProgress"].as_u64().unwrap_or(0) as u32,
        last_turn_mine_hit: ctx["lastTurnMineHit"].as_bool().unwrap_or(false),
    };
    let choice = match ctx["choice"].as_str() {
        Some("attack") => Decision::Attack,
        Some("golden") => Decision::Golden,
        _ => Decision::Reveal,
    };
    Recorded {
        ctx: input,
        choice,
        spatial,
    }
}

#[test]
fn the_inferrer_matches_the_oracle() {
    let fixture = oracle();
    let decisions: Vec<Recorded> = fixture["decisions"]
        .as_array()
        .expect("the decisions")
        .iter()
        .map(recorded)
        .collect();
    let inferred = infer::infer(&decisions);
    let expected = &fixture["inferred"];
    let params = inferred.params.expect("params");
    assert_eq!(params, params_from(&expected["params"]));
    assert!(close(
        inferred.agreement,
        number(&expected["agreement"]),
        1e-12
    ));
    assert_eq!(
        inferred.total_decisions as u64,
        expected["totalDecisions"].as_u64().unwrap_or(0)
    );
    assert_eq!(inferred.profile, expected["profile"].as_str().unwrap_or(""));
    let spatial = inferred.spatial.expect("a spatial report");
    let expected_spatial = &expected["spatial"];
    assert!(close(
        spatial.zone_bias.corner,
        number(&expected_spatial["zoneBias"]["corner"]),
        1e-12
    ));
    assert!(close(
        spatial.zone_bias.edge,
        number(&expected_spatial["zoneBias"]["edge"]),
        1e-12
    ));
    assert!(close(
        spatial.zone_bias.center,
        number(&expected_spatial["zoneBias"]["center"]),
        1e-12
    ));
    assert!(close(
        spatial.adj_mine_bias,
        number(&expected_spatial["adjMineBias"]),
        1e-12
    ));
    assert!(close(
        spatial.adj_safe_bias,
        number(&expected_spatial["adjSafeBias"]),
        1e-12
    ));
}

#[test]
fn a_decision_matches_the_upstream_strategy() {
    // The oracle's recorded decisions were generated by `tunableStrategy`; a
    // default parameter set must reproduce each one.
    let fixture = oracle();
    let params = Params::default();
    for value in fixture["decisions"].as_array().expect("the decisions") {
        let record = recorded(value);
        assert_eq!(
            idleon_minehead::sim::decide(&params, &record.ctx),
            record.choice,
            "decision at {}",
            record.ctx.unrevealed_count
        );
    }
}

#[test]
fn the_rng_is_the_upstream_stream() {
    let mut rng = Rng::new(42);
    assert!((rng.draw() - 0.601_103_751_920_163_6).abs() < 1e-12);
    let _ = get(&Params::default(), "ev_multiplier");
}
