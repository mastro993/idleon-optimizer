//! Clamworks formulas.
//!
//! The tests evaluate the fixture save's Clamworks state at a multikill total
//! of 1234.5%. Fixture decoding is covered in `crates/import/tests/fixture.rs`.

use idleon_clamworks::Clamworks;
use idleon_clamworks::PlanInput;
use idleon_common::ValueFormat;

mod common;
use common::close;

/// The fixture save's Clamworks state.
fn fixture_state() -> Clamworks {
    Clamworks {
        pearl_balance: 2_960_058_875.172_09,
        promotion_level: 6,
        upgrades: [123, 12, 30, 43, 79, 33, 9, 0, 0],
        unlocked: 6,
        scurge_unlocked: true,
        scurge_level: 122,
        scurge_upgrades: [18, 2, 3, 18, 27, 54, 0, 0, 0],
    }
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
        "Menagerie": "[[0,0,0,0,0,0,0,0,7],[0],[0,0,0,0,1],[0],[0],[4,3]]"
    });
    let save = save.as_object().expect("object").clone();
    let mut warnings = Vec::new();
    let clamworks = Clamworks::from_save(&save, &mut warnings);
    assert!(warnings.is_empty());
    assert_eq!(clamworks.pearl_balance, 2.0);
    assert_eq!(clamworks.upgrades[0], 3);
    assert_eq!(clamworks.scurge_level, 7);
    assert!(clamworks.scurge_unlocked);
    assert_eq!(clamworks.scurge_upgrades[0], 4);
    assert_eq!(clamworks.scurge_upgrades[1], 3);
}

#[test]
fn a_save_without_the_menagerie_rows_names_every_missing_one() {
    let save = serde_json::json!({ "OptLacc": vec![0.0; 470] });
    let mut warnings = Vec::new();
    let clamworks = Clamworks::from_save(save.as_object().expect("object"), &mut warnings);
    assert!(!clamworks.scurge_unlocked);
    assert_eq!(
        warnings,
        ["Menagerie: not in the save; Scurge progress reads as zero",]
    );
    let save = serde_json::json!({ "OptLacc": vec![0.0; 470], "Menagerie": "[[0],[0],[0]]" });
    let mut warnings = Vec::new();
    let _ = Clamworks::from_save(save.as_object().expect("object"), &mut warnings);
    assert_eq!(
        warnings,
        [
            "Menagerie[0][8]: Scurge level not in the save; it reads as zero",
            "Menagerie[2][4]: Scurge unlock not in the save; Scurge reads as locked",
            "Menagerie[5]: Scurge upgrade levels not in the save; they read as zero",
        ]
    );
}

#[test]
fn the_rewards_list_every_worker_class_level() {
    let promotion = fixture_state().promotion();
    assert_eq!(promotion.rewards.len(), 12);
    for reward in &promotion.rewards {
        assert!(!reward.description.contains('_'), "the text stays readable");
        assert!(!reward.description.is_empty());
    }
    // A reward unlocks only once the saved level is past its threshold.
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
                idleon_common::Control::Stop
            } else {
                idleon_common::Control::Continue
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
    assert_eq!(plan.steps.len(), idleon_clamworks::plan::MAX_STEPS);
    assert!(plan.spent <= plan.budget);
    assert!(plan.stop_reason().contains("step cap"));
}
