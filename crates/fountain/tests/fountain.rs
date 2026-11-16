//! The fountain calculator against the fixture account and purpose-built saves.

use idleon_common::{Control, Progress};
use idleon_fountain::Fountain;
use idleon_fountain::{self as fountain, CoinStack, Goal, PlanRequest};
use idleon_import::import;
use serde_json::{Value, json};

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn fountain() -> Fountain {
    import(&fixture())
        .expect("the fixture imports")
        .fountain
        .expect("the fixture carries the cavern")
}

/// Compare doubles to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
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
fn the_level_planner_stays_in_range_past_the_last_currency() {
    // Level 45 and above pay with the "unknown" currency type (index
    // `CURRENCIES`), which a 15-step plan reaches from level 31.
    for level in [31.0, 44.0, 45.0, 60.0] {
        let mut rows = holes();
        let mut row = vec![json!(0); 30];
        row[15] = json!(level);
        rows[7] = json!(row);
        let fountain = import_save(&save(rows));
        assert_eq!(fountain.level(), level);
        let plan = fountain.level_plan(15);
        assert_eq!(plan.len(), 15);
        for step in &plan {
            assert!(step.cumulative >= step.cost);
        }
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

#[test]
fn switching_the_desired_currency_changes_what_a_coin_is_worth() {
    let mut rows = holes();
    // Greatest Desire at ten levels: a 50% bonus, so 1.5x on its own.
    rows[31] = json!([[], [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 10]]);
    let fountain = import_save(&save(rows));
    close(fountain.desire_multi(), 1.5);
    // Bronze loses the desire bonus when another currency is desired.
    close(fountain.currency_total_value_with(0, 0), 1.5);
    close(fountain.currency_total_value_with(0, 1), 1.0);
    // Green currencies keep it inside the square root either way.
    let green = fountain.currency_total_value_with(6, 0);
    close(fountain.currency_total_value_with(6, 1), green);
    close(green, 1.5_f64.sqrt());
}

#[test]
fn the_caps_are_labelled_when_an_effect_maxes() {
    let mut rows = holes();
    rows[31] = json!([[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 16, 47, 1], [1], [1],]);
    let fountain = import_save(&save(rows));
    assert_eq!(fountain.spaces(), 16);
    assert_eq!(fountain.max_stack(), 50.0);
    assert_eq!(fountain.waters_owned(), 3);
    let spaces = fountain.upgrade_row(0, 10).expect("Extra Space").cap;
    assert_eq!(spaces.expect("a cap").label, "Capped");
    let stack = fountain.upgrade_row(0, 11).expect("Taller Stacks").cap;
    assert_eq!(stack.expect("a cap").label, "Capped");
    let bender = fountain.upgrade_row(0, 12).expect("Water Bender").cap;
    let bender = bender.expect("a soft cap");
    assert_eq!(bender.label, "Soft cap");
    assert!(!bender.capped);
    let water = fountain.upgrade_row(0, 0).expect("Yellow Water").cap;
    assert_eq!(water.expect("a cap").label, "Max water");

    // A save early in each track reads as near the cap, with the current value.
    let mut rows = holes();
    rows[31] = json!([[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 10]]);
    let fountain = import_save(&save(rows));
    let spaces = fountain.upgrade_row(0, 10).expect("Extra Space").cap;
    let spaces = spaces.expect("a cap");
    assert_eq!(spaces.label, "Near cap");
    assert_eq!(spaces.detail, "5/16 spaces");
    let stack = fountain.upgrade_row(0, 11).expect("Taller Stacks").cap;
    assert_eq!(stack.expect("a cap").detail, "13/50 stack");
    assert!(
        fountain
            .upgrade_row(0, 12)
            .expect("Water Bender")
            .cap
            .is_none()
    );
}

#[test]
fn every_upgrade_row_carries_its_cost_and_marble_rules() {
    let fountain = fountain();
    let rates = fountain.earn_rates();
    assert_eq!(rates.len(), fountain::CURRENCIES);
    assert!(Fountain::can_marble(0, 0));
    assert!(!Fountain::can_marble(0, 8));
    assert!(!Fountain::can_marble(0, 10));
    assert!(!Fountain::can_marble(0, 11));
    assert!(!Fountain::can_marble(1, 8));
    assert!(!Fountain::can_marble(1, 10));
    assert!(Fountain::can_marble(2, 8));
    close(Fountain::marble_cost(0, 0, 0.0), 500.0);
    close(Fountain::marble_cost(0, 13, 0.0), 1_500.0);
    close(Fountain::marble_cost(2, 0, 0.0), 250_000.0);
    close(Fountain::marble_cost(0, 0, 2.0), 500.0 * 7.0 * 7.0);
    close(Fountain::upgrade_cost(0, 10, 0.0), 5.0);
    close(Fountain::upgrade_cost(0, 10, 2.0), 5.0 * 400.0);
    for water in 0..fountain::WATERS {
        let rows = fountain.upgrade_rows(water);
        assert_eq!(rows.len(), 20);
        for row in rows {
            assert!(!row.name.is_empty());
            assert!(row.next_cost > 0.0);
            if row.seconds_to_afford.is_infinite() {
                assert_eq!(rates[row.currency], 0.0);
            }
        }
    }
}

#[test]
fn the_planner_ranks_by_the_unit_cost_of_the_gain() {
    let fountain = fountain();
    let rows = fountain.planner(PlanRequest::default());
    assert!(rows.len() > 1);
    for pair in rows.windows(2) {
        assert!(
            pair[0].efficiency >= pair[1].efficiency,
            "{} ({}) ranks above {} ({})",
            pair[1].name,
            pair[1].efficiency,
            pair[0].name,
            pair[0].efficiency
        );
    }
    let baseline = fountain.measure_goal(Goal::Currency, fountain.desired_currency());
    for row in &rows {
        assert!(row.delta > 0.0);
        assert!((row.pct_gain - row.delta / baseline * 100.0).abs() < 1e-9);
    }
}

#[test]
fn switching_the_goal_changes_the_ranking() {
    let fountain = fountain();
    let currency = fountain.planner(PlanRequest::default());
    let royal = fountain.planner(PlanRequest {
        goal: Goal::Royal,
        ..PlanRequest::default()
    });
    let fill = fountain.planner(PlanRequest {
        goal: Goal::Fill,
        ..PlanRequest::default()
    });
    let top =
        |rows: &[idleon_fountain::PlannerRow]| rows.first().map(|row| (row.water, row.upgrade));
    assert!(top(&currency).is_some());
    assert!(top(&royal).is_some());
    assert!(top(&fill).is_some());
    assert_ne!(
        top(&currency),
        top(&royal),
        "the royal goal ranks a different upgrade first"
    );
    assert_ne!(
        top(&currency),
        top(&fill),
        "the fill goal ranks a different upgrade first"
    );
}

#[test]
fn the_optimizer_respects_its_budget_and_lists_its_steps() {
    let fountain = fountain();
    let plan = fountain.optimize(PlanRequest {
        max_steps: 50,
        affordable: true,
        ..PlanRequest::default()
    });
    assert!(!plan.steps.is_empty());
    let mut spent = [0.0; fountain::CURRENCIES];
    let mut levels = [[0.0; 20]; 3];
    for (water, upgrade) in (0..3).flat_map(|water| (0..20).map(move |upgrade| (water, upgrade))) {
        levels[water][upgrade] = fountain.upgrade_level(water, upgrade);
    }
    for step in &plan.steps {
        spent[step.currency] += step.cost;
        assert!(
            spent[step.currency] <= fountain.level_currency_balance(step.currency),
            "{} spends {} of {} {} but the save holds {}",
            step.name,
            spent[step.currency],
            step.currency,
            step.cost,
            fountain.level_currency_balance(step.currency)
        );
        assert_eq!(
            step.level,
            levels[step.water][step.upgrade] + 1.0,
            "{} levels one at a time",
            step.name
        );
        levels[step.water][step.upgrade] = step.level;
    }
    let grouped: usize = plan.groups.iter().map(|group| group.count).sum();
    assert_eq!(grouped, plan.steps.len(), "every step is in one group");
    for group in &plan.groups {
        assert!(group.end_level - group.start_level + 1.0 >= group.count as f64);
    }
    for (currency, left) in plan.remaining.iter().enumerate() {
        close(
            *left,
            fountain.level_currency_balance(currency) - spent[currency],
        );
    }
}

#[test]
fn the_optimizer_reports_progress_and_honours_cancel() {
    let fountain = fountain();
    let mut reports: Vec<Progress> = Vec::new();
    let outcome = fountain.optimize_with(
        PlanRequest {
            max_steps: 50,
            affordable: true,
            ..PlanRequest::default()
        },
        |progress| {
            reports.push(progress);
            if reports.len() == 2 {
                Control::Stop
            } else {
                Control::Continue
            }
        },
    );
    assert!(outcome.is_none(), "a cancelled plan stores nothing");
    assert_eq!(reports.len(), 2, "the job stops at the second report");
    assert_eq!(reports[0].fraction, 1.0 / 50.0);
    assert!(reports[1].fraction > reports[0].fraction);
    assert!(reports[0].message.contains("Step 1 of 50"));

    let plan = fountain
        .optimize_with(
            PlanRequest {
                max_steps: 5,
                affordable: true,
                ..PlanRequest::default()
            },
            |_| Control::Continue,
        )
        .expect("a report that never stops finishes");
    assert!(!plan.steps.is_empty());
}

#[test]
fn the_marble_planner_ranks_by_the_gain_per_marble() {
    let fountain = fountain();
    let rows = fountain.marble_planner(fountain::MarbleRequest {
        affordable: false,
        ..fountain::MarbleRequest::default()
    });
    assert!(!rows.is_empty());
    for pair in rows.windows(2) {
        assert!(
            pair[0].efficiency >= pair[1].efficiency,
            "{} ({}) ranks above {} ({})",
            pair[1].name,
            pair[1].efficiency,
            pair[0].name,
            pair[0].efficiency
        );
    }
    let baseline = fountain.measure_goal(Goal::Currency, fountain.desired_currency());
    for row in &rows {
        assert!(row.delta > 0.0);
        assert!(
            fountain.upgrade_level(row.water, row.upgrade) > 0.0,
            "{} is owned",
            row.name
        );
        assert!(
            Fountain::can_marble(row.water, row.upgrade),
            "{} has a marble track",
            row.name
        );
        assert_eq!(
            row.marble_level,
            fountain.marble_level(row.water, row.upgrade)
        );
        close(
            row.cost,
            Fountain::marble_cost(row.water, row.upgrade, row.marble_level),
        );
        close(row.pct_gain, row.delta / baseline * 100.0);
        close(row.efficiency, row.pct_gain / row.cost);
    }
    let fill = fountain.marble_planner(fountain::MarbleRequest {
        goal: Goal::Fill,
        affordable: false,
        ..fountain::MarbleRequest::default()
    });
    assert_ne!(
        rows.first().map(|row| (row.water, row.upgrade)),
        fill.first().map(|row| (row.water, row.upgrade)),
        "the fill goal ranks a different marble level first"
    );
}

#[test]
fn the_marble_optimizer_respects_the_marble_budget() {
    let fountain = fountain();
    let plan = fountain.marble_optimize(fountain::MarbleRequest::default());
    assert!(!plan.steps.is_empty());
    let spent: f64 = plan.steps.iter().map(|step| step.cost).sum();
    assert!(
        spent <= fountain.marble(),
        "the plan spends {spent} marble of {}",
        fountain.marble()
    );
    close(plan.remaining, fountain.marble() - spent);
    let grouped: usize = plan.groups.iter().map(|group| group.count).sum();
    assert_eq!(grouped, plan.steps.len(), "every step is in one group");
}

#[test]
#[ignore = "long simulation: CI runs it on main only"]
fn the_simulation_is_deterministic() {
    let fountain = fountain();
    let request = fountain::SimRequest {
        hours: 1.0,
        desired: None,
    };
    let first = fountain.simulate(request);
    let second = fountain.simulate(request);
    assert_eq!(first, second, "the same request replays the same run");
    assert_eq!(first.steps, second.steps);
    assert!(!first.events.is_empty());
}

#[test]
#[ignore = "long simulation: CI runs it on main only"]
fn the_simulation_reports_progress_and_honours_cancel() {
    let fountain = fountain();
    let marble = fountain.marble();
    let level = fountain.upgrade_level(0, 12);
    let mut reports: Vec<Progress> = Vec::new();
    let outcome = fountain.simulate_with(
        fountain::SimRequest {
            hours: 8.0,
            desired: None,
        },
        |progress| {
            reports.push(progress);
            if reports.len() == 2 {
                Control::Stop
            } else {
                Control::Continue
            }
        },
    );
    assert!(outcome.is_none(), "a cancelled run stores nothing");
    assert_eq!(reports.len(), 2, "the run stops at the second report");
    assert!(reports[1].fraction >= reports[0].fraction);
    assert!(reports[0].message.contains("Simulated"));
    assert_eq!(fountain.marble(), marble, "the source cavern is untouched");
    assert_eq!(
        fountain.upgrade_level(0, 12),
        level,
        "the source levels are untouched"
    );

    let mut fractions = Vec::new();
    let result = fountain
        .simulate_with(
            fountain::SimRequest {
                hours: 8.0,
                desired: None,
            },
            |progress| {
                fractions.push(progress.fraction);
                Control::Continue
            },
        )
        .expect("a report that never stops finishes");
    assert!(!result.events.is_empty());
    assert!(
        fractions.windows(2).all(|pair| pair[0] <= pair[1]),
        "progress never goes backwards: {fractions:?}"
    );
}
