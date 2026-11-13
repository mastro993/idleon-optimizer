//! RG-03: the upgrade list, the global breakdowns and the forecast against the
//! upstream JS.
//!
//! Every figure in `fixtures/royal-guardian.json` under `upgrades`, `breakdowns`
//! and `forecast` was produced by `script/royal-oracle.mjs` running the upstream
//! modules on `fixtures/data.json`. The oracle hands the upstream the three
//! sources this port does not decode (the all-talent levels, the W14 alchemy
//! bubble and the companion roster) at the values the port reads, so a drift
//! here is the port's, not a known input gap.

use idleon_import::import;
use idleon_royal_guardian::{Category, ForecastOptions, RoyalGuardian};
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/royal-guardian.json"
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/royal-guardian.json"))
        .expect("the oracle is JSON")
}

fn royal() -> RoyalGuardian {
    import(&fixture())
        .expect("the fixture imports")
        .royal
        .expect("the fixture carries the Royal Guardian rows")
}

/// Compare a figure the oracle printed, naming it when it drifts.
#[track_caller]
fn near(label: &str, got: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (got - expected).abs() <= tolerance,
        "{label}: got {got}, upstream {expected}"
    );
}

fn number(value: &Value, key: &str) -> f64 {
    value.get(key).and_then(Value::as_f64).unwrap_or(f64::NAN)
}

#[test]
fn the_daily_masterclass_reduction_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let expected = &oracle["upgrades"]["masterclassRedux"];
    let redux = royal.masterclass_redux();
    near("redux value", redux.value, number(expected, "value"));
    near("redux prefix", redux.prefix, number(expected, "prefix"));
    near("redux bargain", redux.bargain, number(expected, "bargain"));
    near(
        "reduction percent",
        redux.reduction_percent,
        number(expected, "reductionPercent"),
    );
    near(
        "purchase counter",
        redux.purchase_counter,
        number(expected, "purchaseCounter"),
    );
    near(
        "daily limit",
        redux.daily_limit,
        number(expected, "dailyLimit"),
    );
    assert_eq!(
        royal.armory_unlocked_count(),
        number(&oracle["upgrades"], "unlockedCount") as usize,
        "the Armory level total unlocks the same shelves"
    );
}

#[test]
fn the_armory_list_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let rows = royal.armory_rows();
    let expected = oracle["upgrades"]["armory"]
        .as_array()
        .expect("the shelf list");
    assert_eq!(rows.len(), expected.len(), "every shelf in the order");
    for (row, expected) in rows.iter().zip(expected) {
        let label = |field: &str| format!("shelf {} {field}", row.index);
        assert_eq!(row.index, number(expected, "index") as usize);
        assert_eq!(row.order, number(expected, "order") as usize);
        assert_eq!(row.name, expected["name"].as_str().expect("a name"));
        near(&label("level"), row.level, number(expected, "level"));
        near(
            &label("max level"),
            row.max_level,
            number(expected, "maxLevel"),
        );
        near(
            &label("bonus"),
            row.current_bonus,
            number(expected, "bonus"),
        );
        near(
            &label("next bonus"),
            row.next_bonus,
            number(expected, "nextBonus"),
        );
        near(
            &label("bonus per level"),
            row.bonus_per_level,
            number(expected, "bonusPerLevel"),
        );
        near(&label("cost"), row.cost.value, number(expected, "cost"));
        assert_eq!(
            row.cost.currency_slot,
            number(expected, "currencySlot") as i64,
            "shelf {} currency slot",
            row.index
        );
        near(&label("balance"), row.balance, number(expected, "balance"));
        assert_eq!(
            row.maxed,
            expected["maxed"].as_bool().expect("a maxed flag"),
            "shelf {} maxed",
            row.index
        );
        assert_eq!(
            row.unlocked,
            expected["unlocked"].as_bool().expect("an unlocked flag"),
            "shelf {} unlocked",
            row.index
        );
        assert_eq!(
            row.affordable,
            expected["affordable"]
                .as_bool()
                .expect("an affordable flag"),
            "shelf {} affordable",
            row.index
        );
        // The seven pricing factors multiply to the price the row reports, so a
        // reader can follow a shelf's cost back to its terms.
        let product: f64 = row.cost.factors.iter().map(|factor| factor.value).product();
        near(&label("factor product"), product, row.cost.value);
    }
}

#[test]
fn the_cost_plan_prices_every_projected_level() {
    let royal = royal();
    let oracle = oracle();
    let orders = oracle["upgrades"]["planOrders"]
        .as_array()
        .expect("the plan orders");
    let plans = oracle["upgrades"]["plans"].as_array().expect("the plans");
    assert_eq!(orders.len(), plans.len());
    for (order, expected) in orders.iter().zip(plans) {
        let order_index = order.as_f64().expect("an order") as usize;
        let plan = royal
            .armory_cost_plan(order_index, 50)
            .expect("every planned shelf has a row");
        near("plan index", plan.index as f64, number(expected, "index"));
        assert_eq!(
            plan.currency_slot,
            number(expected, "currencySlot") as i64,
            "plan currency slot"
        );
        near("plan balance", plan.balance, number(expected, "balance"));
        near(
            "plan daily limit",
            plan.daily_limit,
            number(expected, "dailyPurchaseLimit"),
        );
        near(
            "plan purchase counter",
            plan.purchase_counter,
            number(expected, "purchaseCounter"),
        );
        assert_eq!(
            plan.discount_available,
            expected["discountAvailable"]
                .as_bool()
                .expect("a discount flag")
        );
        let rows = expected["rows"].as_array().expect("the priced levels");
        assert_eq!(plan.rows.len(), rows.len(), "every projected level");
        let mut running_discounted = 0.0;
        let mut running_full = 0.0;
        for (step, expected) in plan.rows.iter().zip(rows) {
            let label = format!("plan {order_index} level {}", step.level);
            near(&label, step.level, number(expected, "level"));
            near(
                &format!("{label} next"),
                step.next_level,
                number(expected, "nextLevel"),
            );
            near(
                &format!("{label} discounted"),
                step.discounted,
                number(expected, "discounted"),
            );
            near(
                &format!("{label} full"),
                step.full,
                number(expected, "full"),
            );
            running_discounted += step.discounted;
            running_full += step.full;
            // The cumulative column is the running sum of the rows above it.
            near(
                &format!("{label} cumulative discounted"),
                step.cumulative_discounted,
                number(expected, "cumulativeDiscounted"),
            );
            near(
                &format!("{label} cumulative full"),
                step.cumulative_full,
                number(expected, "cumulativeFull"),
            );
            near(
                &format!("{label} running discounted"),
                running_discounted,
                step.cumulative_discounted,
            );
            near(
                &format!("{label} running full"),
                running_full,
                step.cumulative_full,
            );
            // The discount is never more expensive than the full price.
            assert!(
                step.discounted <= step.full + 1e-6,
                "{label}: the discounted price exceeds the full price"
            );
        }
        // The plan stops at the level cap, so it never sells a level past it.
        assert!(
            plan.rows
                .last()
                .is_none_or(|step| step.next_level <= plan.max_level),
            "plan {order_index} prices past the cap"
        );
        assert_eq!(
            plan.covered_full(),
            plan.rows
                .iter()
                .filter(|step| step.cumulative_full <= plan.balance)
                .count()
        );
    }
}

#[test]
fn the_orblet_list_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let rows = royal.orblet_rows();
    let expected = oracle["upgrades"]["orblets"]
        .as_array()
        .expect("the orblet list");
    assert_eq!(rows.len(), expected.len(), "every orblet row");
    let owned = &oracle["upgrades"]["owned"];
    near(
        "orblets owned",
        royal.orblet_owned().value,
        number(owned, "value"),
    );
    assert_eq!(
        royal.orblet_owned().partial,
        owned["partial"].as_bool().expect("a partial flag"),
        "the owned count reads every inventory"
    );
    for (row, expected) in rows.iter().zip(expected) {
        assert_eq!(row.index, number(expected, "index") as usize);
        assert_eq!(row.name, expected["name"].as_str().expect("a name"));
        near(
            &format!("orblet {} level", row.index),
            row.level,
            number(expected, "level"),
        );
        near(
            &format!("orblet {} max level", row.index),
            row.max_level,
            number(expected, "maxLevel"),
        );
        near(
            &format!("orblet {} bonus", row.index),
            row.bonus,
            number(expected, "bonus"),
        );
        match (row.next_bonus, expected["nextBonus"].as_f64()) {
            (Some(got), Some(want)) => near(&format!("orblet {} next bonus", row.index), got, want),
            (None, None) => {}
            (got, want) => panic!(
                "orblet {} next bonus: got {got:?}, upstream {want:?}",
                row.index
            ),
        }
        match expected["cost"].as_f64() {
            Some(want) => near(&format!("orblet {} cost", row.index), row.cost, want),
            None => assert!(
                !row.cost.is_finite(),
                "orblet {} is maxed, so it has no price",
                row.index
            ),
        }
        near(
            &format!("orblet {} owned", row.index),
            row.owned,
            number(expected, "owned"),
        );
        assert_eq!(
            row.maxed,
            expected["maxed"].as_bool().expect("a maxed flag"),
            "orblet {} maxed",
            row.index
        );
        assert_eq!(
            row.unlocked,
            expected["unlocked"].as_bool().expect("an unlocked flag"),
            "orblet {} unlocked",
            row.index
        );
        assert_eq!(
            row.affordable,
            expected["affordable"]
                .as_bool()
                .expect("an affordable flag"),
            "orblet {} affordable",
            row.index
        );
        assert_eq!(
            row.action,
            expected["kind"].as_str().expect("a kind") == "action",
            "orblet {} action row",
            row.index
        );
    }
}

#[test]
fn the_global_breakdowns_match_upstream() {
    let royal = royal();
    let oracle = oracle();
    let expected = &oracle["breakdowns"];

    let collection = royal.global_breakdown(Category::Collection);
    near(
        "collection multiplier",
        collection.value,
        number(&expected["collection"], "value"),
    );
    let factors = expected["collection"]["factors"]
        .as_array()
        .expect("the factor list");
    assert_eq!(
        collection.root.children.len(),
        factors.len(),
        "every rate factor is a tree node"
    );
    for (node, expected) in collection.root.children.iter().zip(factors) {
        near(
            &format!("collection factor {:?}", node.label),
            node.value,
            number(expected, "value"),
        );
    }
    // The additive factor expands into the five bonuses it divides by a
    // hundred, so its children add up to the percentage it reports.
    let additive = &collection.root.children[1];
    near(
        "collection additive sum",
        additive.sum_children(),
        additive.value * 100.0 - 100.0,
    );

    let range = royal.global_breakdown(Category::Range);
    near(
        "range rules",
        range.value,
        number(&expected["range"], "value"),
    );
    near(
        "range base",
        range.root.children[0].value,
        number(&expected["range"], "base"),
    );
    near(
        "range orblet",
        range.root.children[1].value,
        number(&expected["range"], "orblet"),
    );
    near(
        "range sum",
        range.root.children[0].value + range.root.children[1].value,
        range.value,
    );
    near(
        "range cap",
        range.root.children[2].value,
        number(&expected["range"], "cap"),
    );
    near(
        "range resource allowance",
        range.root.children[3].value,
        number(&expected["range"], "resourceAllowance"),
    );
    near(
        "range outpost allowance",
        range.root.children[4].value,
        number(&expected["range"], "outpostAllowance"),
    );

    let rank_exp = royal.global_breakdown(Category::RankExp);
    near(
        "rank exp",
        rank_exp.value,
        number(&expected["rankExp"], "value"),
    );
    let bars = expected["rankExp"]["bars"]
        .as_array()
        .expect("the bar list");
    assert_eq!(rank_exp.root.children.len(), bars.len());
    for (node, expected) in rank_exp.root.children.iter().zip(bars) {
        near(
            &format!("rank exp bar {}", node.label),
            node.value,
            number(expected, "baseRate"),
        );
    }
    // The selected bar is the one the heading reports.
    let selected = number(&expected["rankExp"], "selectedBar") as usize;
    near(
        "rank exp selected bar",
        rank_exp.root.children[selected].value,
        rank_exp.value,
    );
    near(
        "rank exp orblet",
        royal.orblet_bonus(6),
        number(&expected["rankExp"], "orblet"),
    );
    near(
        "rank exp purity shop",
        royal.shop_bonus(65),
        number(&expected["rankExp"], "shop65"),
    );
    near(
        "rank exp selected shop",
        royal.shop_bonus(77),
        number(&expected["rankExp"], "shop77"),
    );

    let points = royal.global_breakdown(Category::Points);
    near(
        "point rules",
        points.value,
        number(&expected["points"], "value"),
    );
    near(
        "point base",
        points.root.children[0].value,
        number(&expected["points"], "base"),
    );
    // The per-world node is the sum of its seven Armory bonuses.
    near(
        "point world sum",
        points.root.children[1].sum_children(),
        points.root.children[1].value,
    );
    let worlds = expected["points"]["worlds"]
        .as_array()
        .expect("the world list");
    for (node, expected) in points.root.children[1].children.iter().zip(worlds) {
        near(
            &format!("point world {:?}", node.label),
            node.value,
            expected.as_f64().expect("a bonus"),
        );
    }
    near(
        "point sum",
        points.root.children[0].value + points.root.children[1].value,
        points.value,
    );

    let units = royal.global_breakdown(Category::Units);
    near(
        "unit effects",
        units.value,
        number(&expected["units"], "value"),
    );
    let rules = expected["units"]["rules"]
        .as_array()
        .expect("the unit list");
    assert_eq!(units.root.children.len(), rules.len());
    for (node, expected) in units.root.children.iter().zip(rules) {
        let label = expected["name"].as_str().expect("a unit name");
        near(
            &format!("unit {label} effect"),
            node.value,
            number(expected, "value"),
        );
        near(
            &format!("unit {label} base"),
            node.children[0].value,
            number(expected, "base"),
        );
        near(
            &format!("unit {label} armory"),
            node.children[1].value,
            number(expected, "armory"),
        );
        // The armory subtree's last child is the effect it reports.
        near(
            &format!("unit {label} armory effect"),
            node.children[1].children[2].value,
            node.children[1].value,
        );
    }
    near("unit sum", units.root.sum_children(), units.value);

    let support = royal.global_breakdown(Category::Support);
    near(
        "support bonus",
        support.value,
        number(&expected["support"], "value"),
    );
    near(
        "support base",
        support.root.children[0].value,
        number(&expected["support"], "base"),
    );
    near(
        "support armory",
        support.root.children[1].value,
        number(&expected["support"], "base") * number(&expected["support"], "armory") / 100.0,
    );
    near(
        "support jelly",
        support.root.children[2].value,
        number(&expected["support"], "jelly"),
    );
    // Every child is a share of the heading, so they add up to it.
    near("support sum", support.root.sum_children(), support.value);

    let savage = royal.global_breakdown(Category::Savage);
    near(
        "savage multiplier",
        savage.value,
        number(&expected["savage"], "value"),
    );
    near(
        "savage base",
        savage.root.children[0].value,
        number(&expected["savage"], "base"),
    );
    near(
        "savage armory",
        savage.root.children[1].value,
        number(&expected["savage"], "armory"),
    );
    // The heading is the base grown by the armory share, not a sum.
    near(
        "savage growth",
        savage.root.children[0].value * (1.0 + savage.root.children[1].value / 100.0),
        savage.value,
    );
    near(
        "savage currency",
        savage.root.children[2].value,
        number(&expected["savage"], "currency"),
    );
}

#[test]
fn the_forecast_matches_upstream() {
    let royal = royal();
    let oracle = oracle();
    let planned = ForecastOptions {
        horizon_hours: 48.0,
        collects: vec![6.0, 12.5, 30.0],
        windows: vec![(20.0, 22.0)],
    };
    assert_forecast(
        &royal.forecast(&ForecastOptions::default()),
        &oracle["forecast"]["default"],
        "default",
    );
    assert_forecast(
        &royal.forecast(&planned),
        &oracle["forecast"]["planned"],
        "planned",
    );
}

#[track_caller]
fn assert_forecast(got: &idleon_royal_guardian::Forecast, expected: &Value, label: &str) {
    near(
        &format!("{label} horizon"),
        got.horizon_hours,
        number(expected, "horizonHours"),
    );
    assert_eq!(
        got.banks,
        expected["banks"].as_bool().expect("a banking flag"),
        "{label} banks"
    );
    assert_eq!(
        got.partial,
        expected["partial"].as_bool().expect("a partial flag"),
        "{label} partial"
    );
    let missing: Vec<&str> = expected["missing"]
        .as_array()
        .expect("the missing list")
        .iter()
        .map(|value| value.as_str().expect("a source"))
        .collect();
    assert_eq!(
        got.missing.iter().map(String::as_str).collect::<Vec<_>>(),
        missing,
        "{label} missing"
    );
    near(
        &format!("{label} banked start"),
        got.banked_start_hours,
        number(expected, "bankedStartHours"),
    );
    near(
        &format!("{label} banked end"),
        got.end_banked_hours,
        number(expected, "endBankedHours"),
    );
    assert_eq!(
        got.truncated,
        expected["truncated"].as_bool().expect("a truncated flag"),
        "{label} truncated"
    );
    let resets = expected["resets"].as_array().expect("the reset hours");
    assert_eq!(got.resets.len(), resets.len(), "{label} reset count");
    for (hour, want) in got.resets.iter().zip(resets) {
        near(
            &format!("{label} reset"),
            *hour,
            want.as_f64().expect("an hour"),
        );
    }
    let ignored = expected["ignoredCollects"]
        .as_array()
        .expect("the ignored collects");
    assert_eq!(
        got.ignored_collects.len(),
        ignored.len(),
        "{label} ignored collects"
    );

    let collects = expected["collects"].as_array().expect("the collects");
    assert_eq!(got.collects.len(), collects.len(), "{label} collect count");
    for (commit, expected) in got.collects.iter().zip(collects) {
        near(
            &format!("{label} collect at"),
            commit.at,
            number(expected, "at"),
        );
        assert_eq!(
            commit.kind,
            expected["kind"].as_str().expect("a kind"),
            "{label} collect kind"
        );
        near(
            &format!("{label} collect banked"),
            commit.banked,
            number(expected, "banked"),
        );
        assert_currency(
            &format!("{label} collect at {}", commit.at),
            &commit.currency,
            &expected["currency"],
        );
        assert_indices(
            &commit.drained,
            &expected["drained"],
            &format!("{label} drained"),
        );
        let ups = expected["rankUps"].as_array().expect("the rank-ups");
        assert_eq!(commit.rank_ups.len(), ups.len(), "{label} collect rank-ups");
        for (up, expected) in commit.rank_ups.iter().zip(ups) {
            near(
                &format!("{label} rank map"),
                up.map as f64,
                number(expected, "map"),
            );
            near(
                &format!("{label} rank bar"),
                up.bar as f64,
                number(expected, "bar"),
            );
            near(
                &format!("{label} rank from"),
                up.from,
                number(expected, "from"),
            );
            near(&format!("{label} rank to"), up.to, number(expected, "to"));
            near(
                &format!("{label} rank points before"),
                up.points_before,
                number(expected, "pointsBefore"),
            );
            near(
                &format!("{label} rank points after"),
                up.points_after,
                number(expected, "pointsAfter"),
            );
        }
        assert_map_values(
            &commit.points,
            &expected["points"],
            &format!("{label} points"),
        );
        assert_indices(
            &commit.secured,
            &expected["secured"],
            &format!("{label} secured"),
        );
        assert_affordable(
            &commit.affordable,
            &expected["affordable"],
            &format!("{label} affordable"),
        );
    }

    let windows = expected["windows"].as_array().expect("the windows");
    assert_eq!(got.windows.len(), windows.len(), "{label} window count");
    for (window, expected) in got.windows.iter().zip(windows) {
        near(
            &format!("{label} window start"),
            window.start,
            number(expected, "start"),
        );
        near(
            &format!("{label} window end"),
            window.end,
            number(expected, "end"),
        );
        near(
            &format!("{label} window steps"),
            window.steps as f64,
            number(expected, "steps"),
        );
        assert_eq!(
            window.truncated,
            expected["truncated"].as_bool().expect("a truncated flag"),
            "{label} window truncated"
        );
        match (&window.entry, expected["entry"].as_object()) {
            (Some(entry), Some(expected)) => {
                near(
                    &format!("{label} window entry at"),
                    entry.at,
                    number(&Value::Object(expected.clone()), "at"),
                );
                near(
                    &format!("{label} window entry banked"),
                    entry.banked,
                    number(&Value::Object(expected.clone()), "banked"),
                );
                assert_currency(
                    &format!("{label} window entry"),
                    &entry.currency,
                    &Value::Object(expected.clone())["currency"],
                );
            }
            (None, None) => {}
            (got, want) => panic!("{label} window entry does not travel: {got:?} vs {want:?}"),
        }
        let summary = &expected["summary"];
        assert_currency(
            &format!("{label} window currency"),
            &window.summary.currency,
            &summary["currency"],
        );
        near(
            &format!("{label} window rank-ups"),
            window.summary.rank_ups.len() as f64,
            number(summary, "rankUps"),
        );
        assert_map_values(
            &window.summary.points,
            &summary["points"],
            &format!("{label} window points"),
        );
        assert_indices(
            &window.summary.secured,
            &summary["secured"],
            &format!("{label} window secured"),
        );
        let affordable = summary["affordable"]
            .as_array()
            .expect("the affordable list");
        assert_eq!(
            window.summary.affordable.len(),
            affordable.len(),
            "{label} window affordable"
        );
        for (item, expected) in window.summary.affordable.iter().zip(affordable) {
            near(
                &format!("{label} window affordable cost"),
                item.cost,
                number(expected, "cost"),
            );
            near(
                &format!("{label} window affordable at"),
                item.at,
                number(expected, "at"),
            );
        }
    }

    let resets = expected["resetEvents"]
        .as_array()
        .expect("the reset events");
    assert_eq!(got.reset_events.len(), resets.len(), "{label} reset events");
    for (event, expected) in got.reset_events.iter().zip(resets) {
        near(
            &format!("{label} reset at"),
            event.at,
            number(expected, "at"),
        );
        assert_eq!(
            event.refills,
            expected["refills"].as_bool().expect("a refill flag"),
            "{label} reset refills"
        );
        assert_indices(
            &event.refilled,
            &expected["refilled"],
            &format!("{label} reset refilled"),
        );
        near(
            &format!("{label} reset grade gains"),
            event.grade_gains,
            number(expected, "gradeGains"),
        );
        let missed = expected["missed"].as_array().expect("the missed list");
        assert_eq!(event.missed.len(), missed.len(), "{label} reset missed");
        for (missed, expected) in event.missed.iter().zip(missed) {
            near(
                &format!("{label} missed resource"),
                missed.resource as f64,
                number(expected, "resource"),
            );
            near(
                &format!("{label} missed ready"),
                missed.ready_at,
                number(expected, "readyAt"),
            );
        }
    }

    let segments = expected["segments"].as_array().expect("the segments");
    assert_eq!(got.segments.len(), segments.len(), "{label} segment count");
    for (segment, expected) in got.segments.iter().zip(segments) {
        let name = format!("{label} segment {}..{}", segment.start, segment.end);
        near(
            &format!("{name} start"),
            segment.start,
            number(expected, "start"),
        );
        near(&format!("{name} end"), segment.end, number(expected, "end"));
        near(
            &format!("{name} banked start"),
            segment.banked_at_start,
            number(expected, "bankedAtStart"),
        );
        near(
            &format!("{name} banked end"),
            segment.banked_at_end,
            number(expected, "bankedAtEnd"),
        );
        let ghosts = &expected["ghosts"];
        let fills = ghosts["fills"].as_array().expect("the fill ghosts");
        assert_eq!(segment.ghosts.fills.len(), fills.len(), "{name} fills");
        for (fill, expected) in segment.ghosts.fills.iter().zip(fills) {
            near(
                &format!("{name} fill resource"),
                fill.resource as f64,
                number(expected, "resource"),
            );
            near(&format!("{name} fill at"), fill.at, number(expected, "at"));
        }
        let ranks = ghosts["ranks"].as_array().expect("the rank ghosts");
        assert_eq!(segment.ghosts.ranks.len(), ranks.len(), "{name} ranks");
        for (ghost, expected) in segment.ghosts.ranks.iter().zip(ranks) {
            near(
                &format!("{name} rank map"),
                ghost.map as f64,
                number(expected, "map"),
            );
            near(
                &format!("{name} rank bar"),
                ghost.bar as f64,
                number(expected, "bar"),
            );
            near(
                &format!("{name} rank"),
                ghost.rank,
                number(expected, "rank"),
            );
            near(
                &format!("{name} rank from"),
                ghost.from,
                number(expected, "from"),
            );
            near(&format!("{name} rank at"), ghost.at, number(expected, "at"));
            assert_eq!(
                ghost.ready,
                expected["ready"].as_bool().expect("a ready flag"),
                "{name} rank ready"
            );
        }
        let clears = ghosts["clears"].as_array().expect("the clear ghosts");
        assert_eq!(segment.ghosts.clears.len(), clears.len(), "{name} clears");
        for (clear, expected) in segment.ghosts.clears.iter().zip(clears) {
            near(
                &format!("{name} clear map"),
                clear.map as f64,
                number(expected, "map"),
            );
            near(
                &format!("{name} clear at"),
                clear.at,
                number(expected, "at"),
            );
        }
        let affordable = ghosts["affordable"]
            .as_array()
            .expect("the affordable ghosts");
        assert_eq!(
            segment.ghosts.affordable.len(),
            affordable.len(),
            "{name} affordable"
        );
        for (item, expected) in segment.ghosts.affordable.iter().zip(affordable) {
            near(
                &format!("{name} affordable cost"),
                item.cost,
                number(expected, "cost"),
            );
            near(
                &format!("{name} affordable at"),
                item.at,
                number(expected, "at"),
            );
        }
        let nodes = expected["nodes"].as_array().expect("the node list");
        assert_eq!(segment.nodes.len(), nodes.len(), "{name} nodes");
        for (node, expected) in segment.nodes.iter().zip(nodes) {
            let label = format!("{name} node {}", node.resource);
            near(&label, node.resource as f64, number(expected, "resource"));
            near(
                &format!("{label} capacity"),
                node.capacity,
                number(expected, "capacity"),
            );
            near(
                &format!("{label} grade"),
                node.grade,
                number(expected, "grade"),
            );
            near(
                &format!("{label} quantity"),
                node.quantity,
                number(expected, "quantity"),
            );
            near(
                &format!("{label} committed"),
                node.committed,
                number(expected, "committed"),
            );
            assert_eq!(
                node.drained,
                expected["drained"].as_bool().expect("a drained flag"),
                "{label} drained"
            );
            match (node.fill_at, expected["fillAt"].as_f64()) {
                (Some(got), Some(want)) => near(&format!("{label} fill at"), got, want),
                (None, None) => {}
                (got, want) => panic!("{label} fill at does not travel: {got:?} vs {want:?}"),
            }
            let projected = expected["projected"]
                .as_array()
                .expect("the projected points");
            assert_eq!(
                node.projected.len(),
                projected.len(),
                "{label} projected points"
            );
            for ((hour, fraction), expected) in node.projected.iter().zip(projected) {
                near(
                    &format!("{label} projected t"),
                    *hour,
                    number(expected, "t"),
                );
                near(
                    &format!("{label} projected f"),
                    *fraction,
                    number(expected, "f"),
                );
            }
        }
        if let Some(currency) = expected["currency"].as_object() {
            for (slot, points) in currency {
                let slot: i64 = slot.parse().expect("a slot");
                let got = segment
                    .currency
                    .get(&slot)
                    .unwrap_or_else(|| panic!("{name} slot {slot} is missing"));
                let points = points.as_array().expect("the balance points");
                assert_eq!(got.len(), points.len(), "{name} slot {slot} points");
                for ((hour, value), expected) in got.iter().zip(points) {
                    near(
                        &format!("{name} slot {slot} t"),
                        *hour,
                        number(expected, "t"),
                    );
                    near(
                        &format!("{name} slot {slot} v"),
                        *value,
                        number(expected, "v"),
                    );
                }
            }
        }
    }

    let summary = &expected["summary"];
    near(
        &format!("{label} summary collects"),
        got.summary.collects as f64,
        number(summary, "collects"),
    );
    near(
        &format!("{label} summary windows"),
        got.summary.windows as f64,
        number(summary, "windows"),
    );
    assert_currency(
        &format!("{label} summary currency"),
        &got.summary.currency,
        &summary["currency"],
    );
    near(
        &format!("{label} summary rank-ups"),
        got.summary.rank_ups as f64,
        number(summary, "rankUps"),
    );
    near(
        &format!("{label} summary points"),
        got.summary.points,
        number(summary, "points"),
    );
    assert_indices(
        &got.summary.secured,
        &summary["secured"],
        &format!("{label} summary secured"),
    );
    near(
        &format!("{label} summary refilled"),
        got.summary.refilled as f64,
        number(summary, "refilled"),
    );
    near(
        &format!("{label} summary missed"),
        got.summary.missed as f64,
        number(summary, "missed"),
    );
    near(
        &format!("{label} summary grade gains"),
        got.summary.grade_gains,
        number(summary, "gradeGains"),
    );
    near(
        &format!("{label} summary banked at horizon"),
        got.summary.banked_at_horizon,
        number(summary, "bankedAtHorizon"),
    );
}

#[track_caller]
fn assert_currency(label: &str, got: &std::collections::BTreeMap<i64, f64>, expected: &Value) {
    let expected = expected.as_object().cloned().unwrap_or_default();
    assert_eq!(got.len(), expected.len(), "{label}: currency slots");
    for (slot, value) in &expected {
        let slot: i64 = slot.parse().expect("a slot");
        let value = value.as_f64().expect("a value");
        near(
            &format!("{label}: slot {slot}"),
            got.get(&slot).copied().unwrap_or(f64::NAN),
            value,
        );
    }
}

#[track_caller]
fn assert_map_values(got: &std::collections::BTreeMap<usize, f64>, expected: &Value, label: &str) {
    let expected = expected.as_object().cloned().unwrap_or_default();
    assert_eq!(got.len(), expected.len(), "{label}: map count");
    for (map, value) in &expected {
        let map: usize = map.parse().expect("a map");
        near(
            &format!("{label}: map {map}"),
            got.get(&map).copied().unwrap_or(f64::NAN),
            value.as_f64().expect("a value"),
        );
    }
}

#[track_caller]
fn assert_indices(got: &[usize], expected: &Value, label: &str) {
    let expected: Vec<usize> = expected
        .as_array()
        .expect("an index list")
        .iter()
        .map(|value| value.as_f64().expect("an index") as usize)
        .collect();
    assert_eq!(got, expected.as_slice(), "{label}");
}

#[track_caller]
fn assert_affordable(got: &[idleon_royal_guardian::Affordable], expected: &Value, label: &str) {
    let expected = expected.as_array().expect("the affordable list");
    assert_eq!(got.len(), expected.len(), "{label}: count");
    for (item, expected) in got.iter().zip(expected) {
        near(
            &format!("{label} index"),
            item.index as f64,
            number(expected, "index"),
        );
        near(
            &format!("{label} order"),
            item.order as f64,
            number(expected, "order"),
        );
        near(
            &format!("{label} cost"),
            item.cost,
            number(expected, "cost"),
        );
        near(
            &format!("{label} currency slot"),
            item.currency_slot as f64,
            number(expected, "currencySlot"),
        );
        if expected["at"].is_number() {
            near(&format!("{label} at"), item.at, number(expected, "at"));
        }
    }
}
