//! The fountain port against the upstream JS.
//!
//! Every figure in `fixtures/fountain.json` was produced by
//! `script/fountain-oracle.mjs` running the upstream
//! `js/stats/systems/w5/fountain.js` and the `fountain-calc.html` overview
//! formulas on `fixtures/data.json`, so a drift in either the port or the
//! fixture fails here.

use idleon_core::Fountain;
use idleon_core::fountain::{self, CoinStack};
use idleon_core::save::import;
use serde_json::{Value, json};

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/fountain.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/fountain.json"))
        .expect("the oracle is JSON")
}

fn fountain() -> Fountain {
    import(&fixture())
        .expect("the fixture imports")
        .fountain
        .expect("the fixture carries the cavern")
}

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

fn at(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

/// A save carrying only `Research` and a `Holes` array, for edge cases.
fn save(rows: Vec<Value>) -> String {
    json!({ "Research": "[]", "Holes": rows }).to_string()
}

fn holes() -> Vec<Value> {
    vec![json!([]); 35]
}

fn import_save(text: &str) -> Fountain {
    import(text)
        .unwrap_or_else(|error| panic!("the save imports: {error}"))
        .fountain
        .expect("the save carries Holes")
}

#[test]
fn the_summary_matches_upstream() {
    let fountain = fountain();
    let summary = oracle();
    let summary = &summary["summary"];
    close(fountain.level(), at(summary, "fountainLevel"));
    assert_eq!(fountain.waters_owned(), 3);
    assert_eq!(fountain.spaces(), 4);
    close(fountain.max_stack(), at(summary, "maxStack"));
    close(fountain.max_coins(), at(summary, "maxCoins"));
    close(fountain.marble(), at(summary, "marble"));
    assert!(fountain.marble_bar_unlocked());
    close(fountain.marble_per_fill(), at(summary, "marblePerFill"));
    close(fountain.bar_fill_time(), at(summary, "fillTime"));
    close(fountain.bar_speed(), at(summary, "barSpeed"));
    close(fountain.active_speed(), at(summary, "activeSpeed"));
    close(
        fountain.active_speed_display(),
        at(summary, "activeSpeedDisplay"),
    );
    close(
        fountain.currency_fountain_multi(),
        at(summary, "fountainMulti"),
    );
    close(
        fountain.currency_external_multi(),
        at(summary, "externalMulti"),
    );
    close(fountain.currency_all_multi(), at(summary, "totalMulti"));
    close(fountain.royal_chance(), at(summary, "royalChance"));
    close(fountain.effective_royal_multi(), at(summary, "royalMulti"));
    close(fountain.desire_multi(), at(summary, "desireMulti"));
    close(fountain.currency_keep(), at(summary, "keep"));
    close(fountain.size_matter_bonus(), at(summary, "sizeBonus"));
    close(
        fountain.optimal_stacks_per_hr(),
        at(summary, "optimalStacksPerHr"),
    );
    close(fountain.flush_time(), at(summary, "flushTime"));
    close(fountain.stack_fill_time(), at(summary, "stackFillTime"));
    close(
        fountain.all_stacks_fill_time(),
        at(summary, "allStacksFillTime"),
    );
    close(
        fountain.afk_stack_fill_time(),
        at(summary, "afkStackFillTime"),
    );
    close(
        fountain.afk_all_stacks_fill_time(),
        at(summary, "afkAllStacksFillTime"),
    );
    close(fountain.total_redeem_value(), at(summary, "redeemTotal"));
    for (currency, expected) in summary["redeemByType"]
        .as_array()
        .expect("a redeem array")
        .iter()
        .map(|value| value.as_f64().expect("a number"))
        .enumerate()
    {
        close(fountain.redeem_by_currency()[currency], expected);
    }
}

#[test]
fn the_income_table_matches_upstream() {
    let fountain = fountain();
    let oracle = oracle();
    let income = fountain.income();
    let expected = &oracle["income"];
    close(fountain.marble_per_hr(), at(expected, "marblePerHr"));
    close(income.single, at(&expected["totals"], "single"));
    close(income.single_afk, at(&expected["totals"], "singleAfk"));
    close(income.full, at(&expected["totals"], "full"));
    close(income.full_afk, at(&expected["totals"], "fullAfk"));
    close(income.optimal, at(&expected["totals"], "optimal"));
    close(income.optimal_afk, at(&expected["totals"], "optimalAfk"));
    let rows = expected["rows"].as_array().expect("rows");
    assert_eq!(income.rows.len(), rows.len());
    for (row, expected) in income.rows.iter().zip(rows) {
        assert_eq!(
            row.currency,
            expected["type"].as_u64().expect("type") as usize
        );
        close(row.value, at(expected, "value"));
        close(row.single, at(expected, "single"));
        close(row.single_afk, at(expected, "singleAfk"));
        close(row.full, at(expected, "full"));
        close(row.full_afk, at(expected, "fullAfk"));
        close(row.optimal, at(expected, "optimal"));
        close(row.optimal_afk, at(expected, "optimalAfk"));
    }
}

#[test]
fn the_lucky_coins_and_ducks_match_upstream() {
    let fountain = fountain();
    let oracle = oracle();
    let lucky = &oracle["lucky"];
    close(fountain.lucky_coin_base_chance(), at(lucky, "baseChance"));
    close(fountain.lucky_coin_value_per(), at(lucky, "valuePer"));
    let counts = lucky["counts"].as_array().expect("counts");
    for (currency, count) in counts.iter().enumerate() {
        close(
            fountain.lucky_coin_count(currency),
            count.as_f64().expect("count"),
        );
        close(fountain.lucky_coin_value(currency), 1.0);
    }
    let duck = &oracle["duck"];
    assert_eq!(fountain.duck_count(), at(duck, "count"));
    close(fountain.duck_chance(), at(duck, "chance"));
    close(fountain.duck_multi(), at(duck, "multi"));
    close(fountain.duck_bar_progress(), at(duck, "progress"));
    assert!(fountain.duck_bar_unlocked());
}

#[test]
fn the_cross_system_bonuses_match_upstream() {
    let fountain = fountain();
    let oracle = oracle();
    let expected = oracle["cross"].as_array().expect("cross bonuses");
    let bonuses = fountain.cross_bonuses();
    assert_eq!(bonuses.len(), 18);
    for (bonus, expected) in bonuses.iter().zip(expected) {
        assert_eq!(bonus.label, expected[0].as_str().expect("label"));
        close(bonus.percent, expected[1].as_f64().expect("percent"));
    }
}

#[test]
fn the_ground_stacks_match_upstream() {
    let fountain = fountain();
    let oracle = oracle();
    let expected = oracle["stacks"].as_array().expect("stacks");
    let stacks = fountain.stacks();
    assert_eq!(stacks.len(), expected.len());
    for (stack, expected) in stacks.iter().zip(expected) {
        assert_eq!(
            stack.slot,
            expected["slot"].as_u64().expect("slot") as usize
        );
        assert_eq!(
            stack.count(),
            expected["count"].as_u64().expect("count") as usize
        );
        assert_eq!(stack.royal, expected["royal"].as_bool().expect("royal"));
        let coins: Vec<usize> = expected["coins"]
            .as_array()
            .expect("coins")
            .iter()
            .map(|coin| coin.as_u64().expect("coin") as usize)
            .collect();
        assert_eq!(stack.coins, coins);
        close(fountain.stack_redeem_value(stack), at(expected, "redeem"));
    }
}

#[test]
fn the_level_planner_matches_upstream() {
    let fountain = fountain();
    let oracle = oracle();
    let expected = oracle["levels"].as_array().expect("levels");
    let plan = fountain.level_plan(15);
    assert_eq!(plan.len(), expected.len());
    for (step, expected) in plan.iter().zip(expected) {
        assert_eq!(
            step.level,
            expected["level"].as_u64().expect("level") as usize
        );
        close(step.cost, at(expected, "cost"));
        assert_eq!(
            step.currency,
            expected["currency"].as_u64().expect("currency") as usize
        );
        close(step.cumulative, at(expected, "cumulative"));
        assert_eq!(
            step.affordable,
            expected["affordable"].as_bool().expect("affordable")
        );
    }
}

#[test]
fn the_fixture_reports_only_the_missing_companion_roster() {
    let fountain = fountain();
    assert_eq!(fountain.missing_bonuses().len(), 1);
    assert!(fountain.missing_bonuses()[0].contains("Companion 27"));
}

#[test]
fn a_sparse_save_warns_about_every_missing_bonus() {
    let fountain = import_save(&save(holes()));
    let missing = fountain.missing_bonuses();
    for expected in [
        "Upgrade levels",
        "Marble levels",
        "Arcade fountain speed",
        "Lamp wish scaling",
        "Card measurement",
        "Bell coin bonus",
        "Measurement bonus",
        "Bolaia marble rate",
        "Ducks and Cglunko",
        "Companion 27",
    ] {
        assert!(
            missing.iter().any(|warning| warning.contains(expected)),
            "{expected} is warned: {missing:?}"
        );
    }
}

#[test]
fn a_save_without_holes_has_no_fountain() {
    let imported = import(&json!({ "Research": "[]" }).to_string()).expect("the save imports");
    assert!(imported.fountain.is_none());
}

#[test]
fn the_rates_follow_the_saved_upgrade_levels() {
    let mut rows = holes();
    rows[31] = json!([[0, 0, 0, 0, 0, 0, 0, 0, 0, 10]]);
    let slow = import_save(&save(rows.clone()));
    rows[31] = json!([[0, 0, 0, 0, 0, 0, 0, 0, 0, 40]]);
    let fast = import_save(&save(rows));
    assert!(fast.bar_fill_time() < slow.bar_fill_time());
    assert!(fast.income().optimal > slow.income().optimal);
    assert!(fast.bar_speed() > slow.bar_speed());
}

#[test]
fn a_hidden_currency_leaves_the_active_set() {
    let mut rows = holes();
    // Silver and gold unlock with their prerequisites; both are then hidden.
    rows[31] = json!([
        [0, 0, 0, 0, 0, 0, 0, 0, 10, 0, 0, 0, 10],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10],
    ]);
    let mut ignored = vec![json!(0); 84];
    ignored[83] = json!("ab");
    rows[11] = Value::Array(ignored);
    let fountain = import_save(&save(rows));
    assert!(fountain.currency_unlocked(1));
    assert!(fountain.currency_unlocked(2));
    assert!(fountain.currency_unlocked(3));
    assert_eq!(fountain.active_currencies(), vec![0, 3]);
}

#[test]
fn lucky_coins_and_ducks_scale_with_the_saved_counts() {
    let mut rows = holes();
    rows[30] = json!([2]);
    rows[31] = json!([
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0, 1, 0, 0, 0, 0, 0, 0, 0],
    ]);
    rows[33] = json!([0, 0, 0]);
    let mut options = vec![json!(0); 701];
    options[601] = json!(2);
    let text = json!({
        "Research": "[]",
        "Holes": rows,
        "OptLacc": options,
    })
    .to_string();
    let fountain = import_save(&text);
    assert_eq!(fountain.duck_count(), 2.0);
    assert!(fountain.duck_bar_unlocked());
    // Rubber Ducky adds 5 and Lucky Ducky 1, then each duck makes it rarer.
    close(fountain.duck_chance(), 1.0 / 3.0 * 1.06 * 0.2_f64.powi(2));
    close(fountain.lucky_coin_value(0), 1.5);
    assert!(fountain.lucky_coin_chance(0) > 0.0);
    assert!(fountain.lucky_coin_chance(0) < fountain.lucky_coin_chance(1));
    let (ratio, percent) = fountain.duck_odds();
    assert_eq!(ratio, "70.75");
    assert_eq!(percent, "1.41");
}

#[test]
fn the_multi_tree_explains_the_currency_multiplier() {
    let fountain = fountain();
    let tree = fountain.multi_tree();
    close(tree.value, fountain.currency_all_multi());
    assert_eq!(tree.children.len(), 7);
    let labels: Vec<&str> = tree
        .children
        .iter()
        .map(|child| child.label.as_str())
        .collect();
    for expected in [
        "Fountain upgrades",
        "Trench depth",
        "Cosmo Max Coin Stack",
        "Lamp wish",
        "Measurement 16",
        "Bell coin bonus",
        "Cglunko coin value",
    ] {
        assert!(labels.contains(&expected), "{expected} in {labels:?}");
    }
    let fountain_node = &tree.children[0];
    assert_eq!(fountain_node.children.len(), 4);
    close(fountain_node.value, fountain.currency_fountain_multi());
}

#[test]
fn a_royal_stack_multiplies_every_coin() {
    let fountain = fountain();
    let stack = CoinStack {
        slot: 0,
        coins: vec![3, 4, 5],
        royal: false,
    };
    let plain = fountain.stack_redeem_value(&stack);
    let royal = fountain.stack_redeem_value(&CoinStack {
        royal: true,
        ..stack.clone()
    });
    close(royal, plain * fountain.effective_royal_multi());
}

#[test]
fn the_generated_labels_cover_every_currency() {
    assert_eq!(fountain::currency_name(0), "Bronze Coins");
    assert_eq!(fountain::currency_label(5), "T-Bill");
    assert_eq!(fountain::currency_label(9), "?");
    assert_eq!(fountain::water_names()[2], "Green");
    assert_eq!(
        fountain::upgrade(0, 12).expect("Water Bender").name,
        "Water Bender"
    );
}
