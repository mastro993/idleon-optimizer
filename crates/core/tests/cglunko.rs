//! The Cglunko upgrade priority and plan against the upstream JS.
//!
//! Every figure below comes from `crates/core/tests/cglunko-oracle.json`, which
//! `script/cglunko-oracle.mjs` regenerates by running the upstream
//! `js/stats/systems/w5/cglunko.js` on `fixtures/data.json`. The priority rows
//! are the upstream `rankUpgrades` table; the plan repeats this app's greedy
//! rule on a fixed stone balance the fixture cannot afford, and the oracle
//! prices every step through `upgNextCost` at the levels the plan reached.

use idleon_core::cglunko::Page;
use idleon_core::save::import;
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/cglunko-oracle.json");
    serde_json::from_str(
        &std::fs::read_to_string(path).expect("script/cglunko-oracle.mjs writes the fixture"),
    )
    .expect("the oracle fixture is valid JSON")
}

fn number(value: &Value) -> f64 {
    value.as_f64().expect("the oracle writes numbers")
}

fn index(value: &Value) -> usize {
    value.as_u64().expect("the oracle writes indexes") as usize
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

fn fixture_cglunko() -> idleon_core::Cglunko {
    import(&fixture()).expect("the fixture imports").cglunko
}

#[test]
fn the_overview_drop_rate_matches_upstream() {
    let oracle = oracle();
    close(fixture_cglunko().dr(), number(&oracle["dr"]));
}

#[test]
fn every_upgrade_row_matches_upstream() {
    let cglunko = fixture_cglunko();
    let oracle = oracle();
    let rows = oracle["upgrades"]
        .as_array()
        .expect("the oracle lists upgrades");
    assert_eq!(rows.len(), 24);
    for row in rows {
        let index = index(&row["index"]);
        let at = format!("upgrade {index}");
        assert_eq!(
            idleon_core::cglunko::upgrade_name(index),
            row["name"].as_str().unwrap(),
            "{at}: name"
        );
        assert_eq!(
            idleon_core::cglunko::upgrade_description(index),
            row["desc"].as_str().unwrap(),
            "{at}: description"
        );
        close(
            idleon_core::cglunko::upgrade_per_lv(index),
            number(&row["perLv"]),
        );
        close(
            idleon_core::cglunko::upgrade_cost_base(index),
            number(&row["costBase"]),
        );
        assert_eq!(cglunko.level(index), index_of(&row["level"]), "{at}: level");
        close(cglunko.bonus(index), number(&row["bonus"]));
        close(cglunko.next_cost(index), number(&row["nextCost"]));
        close(cglunko.dr_gain(index), number(&row["drGain"]));
        close(cglunko.stone_balance(index), number(&row["balance"]));
    }
}

#[test]
fn the_priority_order_matches_the_upstream_rank_per_page() {
    let cglunko = fixture_cglunko();
    let oracle = oracle();
    for (page, key) in [(Page::Blue, "blue"), (Page::Purple, "purple")] {
        let expected: Vec<usize> = oracle["priority"][key]
            .as_array()
            .expect("the oracle ranks both pages")
            .iter()
            .map(index)
            .collect();
        let actual: Vec<usize> = cglunko.priority(page).iter().map(|row| row.index).collect();
        assert_eq!(actual, expected, "{key}: priority order");
    }
}

#[test]
fn the_plan_matches_the_upstream_costs_and_never_drops_dr() {
    let fixture = fixture_cglunko();
    let oracle = oracle();
    let scenario = &oracle["scenario"];
    let blue: Vec<f64> = scenario["blueBalances"]
        .as_array()
        .expect("the oracle writes blue balances")
        .iter()
        .map(number)
        .collect();
    let purple: Vec<f64> = scenario["purpleBalances"]
        .as_array()
        .expect("the oracle writes purple balances")
        .iter()
        .map(number)
        .collect();

    for (page, key, balances) in [
        (Page::Blue, "blue", &blue),
        (Page::Purple, "purple", &purple),
    ] {
        // The oracle tops up only the page it plans from, so each plan starts
        // from the fixture with that page's six stones replaced.
        let mut cglunko = fixture.clone();
        match page {
            Page::Blue => cglunko.blue_stones.copy_from_slice(balances),
            Page::Purple => cglunko.purple_stones.copy_from_slice(balances),
        }
        let expected = &oracle["plans"][key];
        let plan = cglunko.plan(page);
        assert_eq!(
            plan.steps.len(),
            expected["steps"].as_array().unwrap().len(),
            "{key}: steps"
        );
        for (step, row) in plan.steps.iter().zip(expected["steps"].as_array().unwrap()) {
            let at = format!("{key} step {} ({})", step.index, row["index"]);
            assert_eq!(step.index, index(&row["index"]), "{at}: upgrade");
            assert_eq!(step.from_level, index_of(&row["from"]), "{at}: level");
            close(step.cost, number(&row["cost"]));
            close(step.dr_gain, number(&row["drGain"]));
            close(step.balance_before, number(&row["balanceBefore"]));
            close(step.balance_after, number(&row["balanceAfter"]));
            assert!(
                step.cost <= step.balance_before,
                "{at}: spends more than the stone holds"
            );
        }
        close(plan.dr_before(), number(&expected["start"]));
        close(plan.dr_after(), number(&expected["dr"]));
        close(plan.spent, number(&expected["spent"]));
        assert!(plan.dr_after() >= plan.dr_before(), "{key}: never worse");
        assert!(!plan.capped);
    }
}

/// A JSON level as `u32`.
fn index_of(value: &Value) -> u32 {
    value.as_u64().expect("the oracle writes levels") as u32
}
