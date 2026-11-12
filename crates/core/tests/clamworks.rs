//! Clamworks formula parity.
//!
//! The expected values below were evaluated with the upstream
//! `js/stats/systems/w7/clamworks.js` against the fixture save's Clamworks
//! state at a multikill total of 1234.5%. They pin the port, not the fixture:
//! save decoding is covered separately in `fixture.rs` and `save.rs`.

use idleon_core::clamworks::{Category, PlanInput};
use idleon_core::{Clamworks, ValueFormat};

/// The fixture save's Clamworks state.
fn fixture_state() -> Clamworks {
    Clamworks {
        pearl_balance: 2_960_058_875.172_09,
        promotion_level: 6,
        upgrades: [123, 12, 30, 43, 79, 33, 9, 0, 0],
        scurge_level: 122,
        scurge_upgrades: [18, 2, 3, 18, 27, 54, 0, 0, 0],
    }
}

fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "{actual} != {expected}"
    );
}

#[test]
fn the_zero_state_has_no_bonuses() {
    let clamworks = Clamworks::default();
    assert_eq!(clamworks.pearl_value(0.0), 1.0);
    assert_eq!(clamworks.pearl_10x_chance(), 0.0);
    assert_eq!(clamworks.pearl_10x_expected(), 1.0);
    assert_eq!(clamworks.black_pearl_value(), 50.0);
    assert_eq!(clamworks.black_pearl_drop_rate(), 0.0);
    assert_eq!(clamworks.cost_reduction(), 1.0);
    assert_eq!(clamworks.clam_count(), 2);
}

#[test]
fn pearl_value_is_the_product_of_its_breakdown() {
    let clamworks = fixture_state();
    let breakdown = clamworks.pearl_value_breakdown(1234.5);
    close(breakdown.base, 124.0);
    close(breakdown.multikill, 1.530_835);
    close(breakdown.shinier, 1.0);
    close(breakdown.scurge, 2.0);
    close(breakdown.product(), 379.647_08);
    close(clamworks.pearl_value(1234.5), 379.647_08);
    assert_eq!(clamworks.pearl_value(1234.5), breakdown.product());
}

#[test]
fn the_pearl_value_tree_multiplies_to_its_root() {
    let clamworks = fixture_state();
    let tree = clamworks.pearl_value_tree(1234.5);
    assert_eq!(tree.children.len(), 4);
    let product: f64 = tree.children.iter().map(|child| child.value).product();
    close(product, tree.value);
    assert_eq!(tree.value, clamworks.pearl_value(1234.5));
    assert!(
        tree.children
            .iter()
            .all(|child| child.format == ValueFormat::Multiplier)
    );
    assert_eq!(tree.keys().len(), 5);
}

#[test]
fn summary_values_match_the_upstream_save() {
    let clamworks = fixture_state();
    close(clamworks.pearl_10x_chance(), 0.473_684_210_526_315_8);
    close(clamworks.pearl_10x_expected(), 5.263_157_894_736_842_5);
    close(clamworks.black_pearl_value(), 1700.0);
    close(
        clamworks.effective_pearls_per_drop(1234.5),
        1_998.142_526_315_789_4,
    );
    close(clamworks.cost_reduction(), 0.095_419_847_328_244_27);
    close(clamworks.promo_penalty(), 0.082_653_950_016_000_02);
    close(clamworks.pearl_drop_rate(), 0.000_785_212_525_152_000_2);
    close(
        clamworks.black_pearl_drop_rate(),
        0.000_003_926_062_625_760_001,
    );
    close(clamworks.pearls_per_kill(1234.5), 1.575_640_845_165_81);
    close(clamworks.promotion_cost(), 100_000_000_000.0);
    close(clamworks.promotion_chance(), 0.0625);
    assert_eq!(clamworks.clam_count(), 14);
    close(clamworks.clam_hp(), 7.29e24);
}

#[test]
fn upgrade_costs_match_the_upstream_save() {
    let clamworks = fixture_state();
    let expected = [
        1_163_189.220_359_348_5,
        7_179_002.006_716_39,
        32_493.597_541_899_777,
        1_489_958.538_990_910_2,
        744_579.965_238_359_5,
        141_513_095.418_493_48,
        24_013_965.205_910_545,
        41_801_726.630_811_94,
        1_142_357_583.996_53,
    ];
    for (index, expected) in expected.into_iter().enumerate() {
        close(clamworks.next_upgrade_cost(index), expected);
    }
}

#[test]
fn the_priority_table_matches_the_upstream_ranking() {
    let clamworks = fixture_state();
    let ranks = clamworks.priority(1234.5);
    let order: Vec<usize> = ranks.iter().map(|rank| rank.index).collect();
    assert_eq!(order, [2, 0, 3, 7, 6, 5, 1, 4, 8]);
    close(ranks[0].efficiency, 4.287_114_759_883_682_7e-7);
    close(ranks[1].efficiency, 6.903_739_522_104_26e-9);
    close(ranks[2].ppk_gain, 0.008_030_066_876_831_832);
    close(ranks[6].cost_save, 0.0);
    close(ranks[7].cost_save, 0.011_320_754_716_980_982);
    close(ranks[8].cost_save, 0.137_931_034_482_758_54);
    assert_eq!(ranks[7].category, Category::Cost);
    assert_eq!(ranks[8].category, Category::Cost);
    assert_eq!(ranks[6].category, Category::Clams);
    assert_eq!(ranks[5].category, Category::Black);
    assert!(ranks[..6].iter().all(|rank| rank.is_pearl_upgrade));
    assert!(ranks[6..].iter().all(|rank| !rank.is_pearl_upgrade));
}

#[test]
fn the_priority_table_is_sorted_and_finite() {
    let ranks = fixture_state().priority(1234.5);
    assert_eq!(ranks.len(), 9);
    assert!(
        ranks
            .windows(2)
            .all(|pair| pair[0].efficiency >= pair[1].efficiency),
        "efficiency must be non-increasing",
    );
    for rank in ranks {
        assert!(rank.next_cost.is_finite() && rank.next_cost > 0.0);
        assert!(rank.efficiency.is_finite() && rank.efficiency >= 0.0);
    }
}

#[test]
fn the_priority_updates_with_the_saved_levels() {
    let mut clamworks = fixture_state();
    assert_eq!(clamworks.priority(1234.5)[0].index, 2);
    // At LUCKY DAY 60 the marginal 10x gain per cost falls below PEARL VALUE.
    clamworks.upgrades[2] = 60;
    assert_eq!(clamworks.priority(1234.5)[0].index, 0);
}

#[test]
fn clam_count_caps_at_25_and_pure_pearls_gate_the_black_rate() {
    let mut clamworks = fixture_state();
    clamworks.upgrades[1] = 40;
    assert_eq!(clamworks.clam_count(), 25);
    clamworks.upgrades[5] = 0;
    assert_eq!(clamworks.black_pearl_drop_rate(), 0.0);
    assert_eq!(clamworks.black_pearl_value(), 50.0);
}

#[test]
fn a_save_without_options_reads_as_the_zero_state() {
    let mut warnings = Vec::new();
    let clamworks = Clamworks::from_save(&serde_json::Map::new(), &mut warnings);
    assert_eq!(clamworks, Clamworks::default());
    assert!(warnings.iter().any(|warning| warning.contains("OptLacc")));
}

#[test]
fn optionslistaccount_wins_over_optlacc_and_menagerie_holds_the_scurge_state() {
    let mut options = vec![0.0; 456];
    options[454] = 2.0;
    options[455] = 3.0;
    let save = serde_json::json!({
        "OptLacc": [99, 99, 99],
        "OptionsListAccount": options,
        "Menagerie": "[[0,0,0,0,0,0,0,0,7],[0],[0],[0],[0],[4,3]]"
    });
    let save = save.as_object().expect("object").clone();
    let mut warnings = Vec::new();
    let clamworks = Clamworks::from_save(&save, &mut warnings);
    assert!(warnings.is_empty());
    assert_eq!(clamworks.pearl_balance, 2.0);
    assert_eq!(clamworks.upgrades[0], 3);
    assert_eq!(clamworks.scurge_level, 7);
    assert_eq!(clamworks.scurge_upgrades[0], 4);
    assert_eq!(clamworks.scurge_upgrades[1], 3);
}

/// `crates/core/tests/clamworks-oracle.json`, written by `script/clamworks-oracle.mjs`.
fn oracle() -> serde_json::Value {
    serde_json::from_str(include_str!("clamworks-oracle.json")).expect("the oracle JSON is valid")
}

fn oracle_number(value: &serde_json::Value, key: &str) -> f64 {
    value
        .get(key)
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_else(|| panic!("the oracle has no {key}"))
}

#[test]
fn the_promotion_tracker_matches_upstream() {
    let oracle = oracle();
    let expected = &oracle["promotion"];
    let promotion = fixture_state().promotion();
    assert_eq!(
        f64::from(promotion.level),
        oracle_number(&oracle, "promotionLevel")
    );
    close(promotion.balance, oracle_number(&oracle, "pearlBalance"));
    close(promotion.chance, oracle_number(expected, "chance"));
    close(promotion.cost, oracle_number(expected, "cost"));
    assert_eq!(
        promotion.expected_attempts,
        oracle_number(expected, "expectedAttempts") as u32
    );
    close(
        promotion.expected_cost,
        oracle_number(expected, "expectedCost"),
    );
    close(
        promotion.clam_hp_after,
        oracle_number(expected, "clamHpAfter"),
    );
    assert_eq!(promotion.level, 6);
    assert_eq!(promotion.expected_attempts, 16);
    assert!(
        !promotion.affordable(),
        "1e11 pearls is more than the balance"
    );
    close(promotion.shortfall(), 1e11 - promotion.balance);
}

#[test]
fn the_rewards_list_every_worker_class_level() {
    let oracle = oracle();
    let expected = oracle["promotion"]["rewards"]
        .as_array()
        .expect("the oracle lists rewards");
    let promotion = fixture_state().promotion();
    assert_eq!(promotion.rewards.len(), 12);
    assert_eq!(promotion.rewards.len(), expected.len());
    for (reward, expected) in promotion.rewards.iter().zip(expected) {
        assert_eq!(
            reward.level,
            expected["level"].as_u64().expect("a reward level") as u32
        );
        assert_eq!(reward.description, expected["description"]);
        assert_eq!(
            reward.unlocked,
            expected["unlocked"].as_bool().expect("a flag")
        );
        assert!(!reward.description.contains('_'), "the text stays readable");
        assert!(!reward.description.is_empty());
    }
    // `clamWorkBonus(t)` is 1 only when the saved level is past `t`.
    assert_eq!(promotion.unlocked(), 6);
    assert!(promotion.rewards[..6].iter().all(|reward| reward.unlocked));
    assert!(promotion.rewards[6..].iter().all(|reward| !reward.unlocked));
    assert_eq!(promotion.rewards[11].level, 12);
    assert_eq!(promotion.rewards[11].description, "2x Spelunking POW");
}

#[test]
fn a_save_without_a_promotion_has_every_reward_locked() {
    let promotion = Clamworks::default().promotion();
    assert_eq!(promotion.level, 0);
    assert_eq!(promotion.unlocked(), 0);
    assert_eq!(promotion.expected_attempts, 4, "0.5 / 2 = 0.25");
    close(promotion.cost, 1e5);
    close(promotion.clam_hp_after, 3e17);
    assert!(promotion.rewards.iter().all(|reward| !reward.unlocked));
}

#[test]
fn the_plan_matches_the_upstream_optimizer() {
    let clamworks = fixture_state();
    for expected in oracle()["plans"]
        .as_array()
        .expect("the oracle lists plans")
    {
        let budget = oracle_number(expected, "budget");
        let multikill = oracle_number(expected, "multikillTotal");
        let plan = clamworks.plan(PlanInput {
            budget,
            multikill_total: multikill,
        });
        assert_eq!(
            plan.steps.len(),
            expected["steps"].as_array().unwrap().len()
        );
        for (step, expected) in plan.steps.iter().zip(expected["steps"].as_array().unwrap()) {
            assert_eq!(step.index, expected["index"].as_u64().unwrap() as usize);
            assert_eq!(
                step.from_level,
                expected["fromLevel"].as_u64().unwrap() as u32
            );
            close(step.cost, oracle_number(expected, "cost"));
            close(step.gain, oracle_number(expected, "gain"));
        }
        close(plan.spent, oracle_number(expected, "spent"));
        let end: Vec<u32> = expected["endUpgrades"]
            .as_array()
            .unwrap()
            .iter()
            .map(|level| level.as_u64().unwrap() as u32)
            .collect();
        assert_eq!(plan.end.upgrades.to_vec(), end);
        assert_eq!(plan.capped, expected["capped"].as_bool().expect("a flag"),);
        close(
            plan.pearls_per_kill_before(),
            oracle_number(expected, "pearlsPerKillBefore"),
        );
        close(
            plan.pearls_per_kill_after(),
            oracle_number(expected, "pearlsPerKillAfter"),
        );
        close(
            plan.pearl_value_before(),
            oracle_number(expected, "pearlValueBefore"),
        );
        close(
            plan.pearl_value_after(),
            oracle_number(expected, "pearlValueAfter"),
        );
        assert!(plan.gain() >= 0.0);
    }
}

#[test]
fn the_plan_respects_every_cost_it_pays() {
    let plan = fixture_state().plan(PlanInput {
        budget: 2_960_058_875.172_09,
        multikill_total: 1234.5,
    });
    assert!(plan.steps.len() > 10);
    let mut levels = plan.start.clone();
    let mut spent = 0.0;
    for step in &plan.steps {
        // The purchase is priced at the levels the plan has reached, so the
        // cost reductions from earlier steps are already applied.
        let cost = levels.next_upgrade_cost(step.index);
        close(step.cost, cost);
        assert!(
            cost <= plan.budget - spent,
            "step {} costs {cost} with {} left",
            step.index,
            plan.budget - spent
        );
        assert_eq!(levels.upgrades[step.index], step.from_level);
        levels.upgrades[step.index] += 1;
        spent += cost;
        assert!(step.gain >= 0.0);
    }
    close(plan.spent, spent);
    assert!(plan.spent <= plan.budget);
    close(plan.remaining(), plan.budget - plan.spent);
    assert_eq!(levels.upgrades, plan.end.upgrades);
    assert_eq!(plan.start.upgrades, fixture_state().upgrades);
    assert!(plan.pearls_per_kill_after() >= plan.pearls_per_kill_before());
    assert_eq!(plan.steps.len(), 71);
}

#[test]
fn a_plan_with_no_budget_buys_nothing() {
    let plan = fixture_state().plan(PlanInput {
        budget: 0.0,
        multikill_total: 1234.5,
    });
    assert!(plan.steps.is_empty());
    assert_eq!(plan.spent, 0.0);
    assert_eq!(plan.remaining(), 0.0);
    assert_eq!(plan.end, plan.start);
    assert_eq!(plan.gain(), 0.0);
    assert!(!plan.capped);
}

#[test]
fn a_plan_without_multikill_skips_multi_scalping() {
    let plan = fixture_state().plan(PlanInput {
        budget: 5_000_000.0,
        multikill_total: 0.0,
    });
    assert!(!plan.steps.is_empty());
    assert!(
        plan.steps.iter().all(|step| step.index != 3),
        "Multi-Scalping is worth nothing at zero multikill"
    );
    assert_eq!(plan.start.multikill_bonus(0.0), 0.0);
}

#[test]
fn a_plan_stops_when_the_reporter_asks() {
    let mut reported = 0;
    let plan = fixture_state().plan_with(
        PlanInput {
            budget: 2_960_058_875.172_09,
            multikill_total: 1234.5,
        },
        |progress| {
            reported += 1;
            assert!(progress.fraction > 0.0 && progress.fraction <= 1.0);
            assert!(progress.message.contains("Lv"));
            if reported == 3 {
                idleon_core::model::Control::Stop
            } else {
                idleon_core::model::Control::Continue
            }
        },
    );
    assert!(plan.is_none(), "a stopped run stores nothing");
    assert_eq!(reported, 3);
}

#[test]
fn the_plan_caps_its_steps() {
    let plan = Clamworks::default().plan(PlanInput {
        budget: 1e30,
        multikill_total: 1000.0,
    });
    assert!(plan.capped);
    assert_eq!(plan.steps.len(), idleon_core::clamworks::plan::MAX_STEPS);
    assert!(plan.spent <= plan.budget);
    assert!(plan.stop_reason().contains("step cap"));
}
