//! The Cglunko upgrade priority, plan and stone focus against the upstream JS.
//!
//! Every figure below comes from `crates/cglunko/tests/cglunko-oracle.json`, which
//! `script/cglunko-oracle.mjs` regenerates by running the upstream
//! `js/stats/systems/w5/cglunko.js` on `fixtures/data.json`. The priority rows
//! are the upstream `rankUpgrades` table; the plan repeats this app's greedy
//! rule on a fixed stone balance the fixture cannot afford, and the oracle
//! prices every step through `upgNextCost` at the levels the plan reached. The
//! stone and focus rows are the upstream `stoneEfficiency` and
//! `bestStoneFocus` tables at the fixture's own stones and focus slots.

use idleon_cglunko::Page;
use idleon_import::import;
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

/// A JSON level as `u32`.
fn index_of(value: &Value) -> u32 {
    value.as_u64().expect("the oracle writes levels") as u32
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

fn fixture_cglunko() -> idleon_cglunko::Cglunko {
    import(&fixture()).expect("the fixture imports").cglunko
}

/// The page an oracle row names by its upstream colour.
fn page(value: &Value) -> Page {
    Page::from_id(value.as_str().expect("the oracle writes colours"))
        .expect("the oracle names a known page")
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
            idleon_cglunko::upgrade_name(index),
            row["name"].as_str().unwrap(),
            "{at}: name"
        );
        assert_eq!(
            idleon_cglunko::upgrade_description(index),
            row["desc"].as_str().unwrap(),
            "{at}: description"
        );
        close(idleon_cglunko::upgrade_per_lv(index), number(&row["perLv"]));
        close(
            idleon_cglunko::upgrade_cost_base(index),
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

#[test]
fn every_stone_row_matches_upstream() {
    let cglunko = fixture_cglunko();
    let oracle = oracle();
    let expected = oracle["stones"]
        .as_array()
        .expect("the oracle lists the twelve stones");
    let stones = cglunko.stone_rows();
    assert_eq!(stones.len(), 12);
    assert_eq!(stones.len(), expected.len(), "stone rows");
    for (stone, row) in stones.iter().zip(expected) {
        let at = format!("stone {} on {}", stone.index, stone.page.id());
        // The upstream `idx` is the slot within the colour; the app keys its
        // rows by the stone across both pages.
        assert_eq!(stone.slot, index(&row["index"]), "{at}: slot");
        assert_eq!(stone.page, page(&row["color"]), "{at}: colour");
        close(stone.current, number(&row["current"]));
        assert_eq!(
            stone.digits,
            u32::try_from(index(&row["digits"])).unwrap(),
            "{at}: digits"
        );
        close(stone.next_pow10, number(&row["nextPow10"]));
        close(stone.deficit, number(&row["deficit"]));
        close(stone.base_rate, number(&row["baseRate"]));
        close(stone.drops_per_kill, number(&row["dropsPerKill"]));
        close(stone.kills_needed, number(&row["killsNeeded"]));
        close(stone.focus_multi, number(&row["focusMulti"]));
        assert_eq!(
            stone.focused,
            row["focused"]
                .as_bool()
                .expect("the oracle writes booleans"),
            "{at}: focused"
        );
    }
}

#[test]
fn the_focus_recommendation_matches_upstream() {
    let cglunko = fixture_cglunko();
    let oracle = oracle();
    let expected = oracle["focus"]
        .as_array()
        .expect("the oracle lists the focus rows");
    let focus = cglunko.best_focus();
    assert_eq!(focus.len(), expected.len(), "focus rows");
    for (row, want) in focus.iter().zip(expected) {
        let at = format!("focus {}", row.index);
        assert_eq!(row.index, index(&want["index"]), "{at}: index");
        assert_eq!(row.page, page(&want["color"]), "{at}: colour");
        assert_eq!(row.slot, index(&want["stone"]), "{at}: stone slot");
        close(row.deficit, number(&want["deficit"]));
        close(row.kills_with_focus, number(&want["killsWithFocus"]));
        close(row.kills_without, number(&want["killsWithout"]));
        close(row.dr_per_digit, number(&want["drPerDigit"]));
    }
    // The upstream sort: fewest kills with the focus slot first.
    assert!(
        focus
            .windows(2)
            .all(|pair| pair[0].kills_with_focus <= pair[1].kills_with_focus)
    );
}
