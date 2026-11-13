//! The opal maths, checked against the page's own formulas.

use super::optimize::Target;
use super::*;
use idleon_common::Control;

/// A save-shaped state whose terms are round numbers, so a wrong factor shows.
fn sample() -> Opal {
    let terms = Terms {
        base_plus_upg: 125.0,
        exotic: 50.0,
        arcade: 40.0,
        grimoire: 100.0,
        arcane: 200.0,
        mon_bravery: 1_000.0,
        mon_justice: 500.0,
        mon_wisdom: 250.0,
        meas_slab: 100.0,
        meas_tome: 50.0,
        b_upg48: 40.0,
        card_cave_b: 20.0,
        bell1: 10.0,
        summon_win21: 5.0,
        vault73: 25.0,
        cosmo_1_1: 2.0,
        scholars: 10.0,
        cosmo_1_2: 10.0,
        compass59: 100.0,
        legend12: 50.0,
        pristine21: 25.0,
        companion13: 0.5,
        statue28: 200.0,
        jar_sum: 300.0,
        event_shop6: 1.0,
        lore_episode1: 100.0,
        bundle_u: 1.0,
    };
    Opal {
        villagers: [
            Villager {
                level: 20.0,
                opals: 100.0,
                stars: 1.0,
            },
            Villager {
                level: 30.0,
                opals: 24.0,
                stars: 2.0,
            },
            Villager {
                level: 0.0,
                opals: 0.0,
                stars: 0.0,
            },
            Villager {
                level: 5.0,
                opals: 35.0,
                stars: 0.0,
            },
            Villager {
                level: 40.0,
                opals: 60.0,
                stars: 3.0,
            },
        ],
        cosmo: [vec![4.0; 12], vec![5.0, 4.0, 3.0, 4.0, 5.0, 4.0]],
        ext: Ext {
            star_mul: [2.0, 3.0, 1.0, 1.0, 4.0],
            enthusiasm: 150.0,
            equal_spread: 8.0,
            holeoz: [1.0, 1.0, 1.71, 1.0, 1.0],
            terms,
        },
    }
}

#[test]
fn the_additive_part_is_the_constant_terms_added_up() {
    let opal = sample();
    let terms = opal.ext.terms;
    // Contented Creator is its per-level value times the schematics made.
    assert_eq!(
        terms.additive(),
        1_000.0
            + 500.0
            + 250.0
            + 100.0
            + 50.0
            + 2.0 * 10.0
            + 10.0
            + 40.0
            + 20.0
            + 10.0
            + 5.0
            + 25.0
    );
    let summed: f64 = terms.additive_terms().iter().map(|(_, value)| *value).sum();
    assert_eq!(summed, terms.additive());
    assert_eq!(terms.additive_terms().len(), 12);
}

#[test]
fn the_chain_multiplies_its_factors_and_floors_at_one() {
    let opal = sample();
    let terms = opal.ext.terms;
    let factors = terms.chain_factors();
    assert_eq!(factors.len(), CHAIN.len());
    assert_eq!(factors[0], 2.0);
    assert_eq!(factors[3], 2.0, "the companion is applied twice");
    assert_eq!(factors[5], 4.0);
    assert_eq!(factors[8], 1.5);
    assert!((terms.chain() - factors.iter().product::<f64>()).abs() < 1e-9);

    let empty = Terms { ..Terms::default() };
    assert_eq!(empty.chain(), 1.0, "an empty chain never drops below 1");
}

#[test]
fn the_base_multiplier_is_the_base_the_chain_and_the_outer_factors() {
    let terms = sample().ext.terms;
    let expected = terms.base_plus_upg * terms.chain() * 1.5 * 1.4 * 2.0 * 3.0;
    assert!((terms.base_multiplier() - expected).abs() <= expected * 1e-12);
}

#[test]
fn equal_spread_scales_with_the_lowest_unlocked_villager() {
    let opal = sample();
    let opals = opal.opals();
    assert_eq!(
        opal.min_opals(&opals),
        24.0,
        "the locked villager is skipped"
    );
    assert_eq!(opal.equal_spread(&opals), 8.0 * 4.0);
    assert_eq!(opal.equal_spread_mul(&opals), 1.32);

    // Raising the minimum to the next five raises the bonus by one step.
    let mut raised = opals;
    raised[1] = 30.0;
    assert_eq!(opal.equal_spread(&raised), 8.0 * 6.0);
}

#[test]
fn enthusiasm_counts_ten_opals_at_a_time() {
    let opal = sample();
    let opals = opal.opals();
    assert_eq!(opal.enthusiasm(0, &opals), 150.0 * 10.0);
    assert_eq!(opal.enthusiasm(1, &opals), 150.0 * 2.0);
    assert_eq!(opal.enthusiasm(3, &opals), 150.0 * 3.0);
}

#[test]
fn a_rate_is_the_factors_multiplied() {
    let opal = sample();
    let opals = opal.opals();
    let expected = 1.0
        * opal.ext.base_multiplier()
        * 100.0
        * 2.0
        * opal.equal_spread_mul(&opals)
        * (1.0 + opal.additive_pct(0, &opals) / 100.0);
    assert!((opal.exp_rate(0, &opals) - expected).abs() <= expected * 1e-12);
    // The Measurer's rate uses the same factors, with its own star multiplier.
    let measurer = 1.0
        * opal.ext.base_multiplier()
        * 35.0
        * 1.0
        * opal.equal_spread_mul(&opals)
        * (1.0 + opal.additive_pct(3, &opals) / 100.0);
    assert!((opal.exp_rate(3, &opals) - measurer).abs() <= measurer * 1e-12);
}

#[test]
fn the_total_adds_the_unlocked_villagers_only() {
    let opal = sample();
    let opals = opal.opals();
    assert_eq!(opal.unlocked_count(), 4);
    let total: f64 = [0, 1, 3, 4]
        .iter()
        .map(|villager| opal.exp_rate(*villager, &opals))
        .sum();
    assert_eq!(opal.total_exp_rate(&opals), total);
    assert_eq!(
        opal.exp_rate(2, &opals),
        0.0,
        "the locked villager earns nothing"
    );
}

#[test]
fn a_villager_with_no_opals_has_no_factors() {
    let opal = sample();
    let tree = opal.breakdown(Some(2));
    assert_eq!(tree.value, 0.0);
    assert!(tree.children.is_empty());
}

#[test]
fn the_villager_tree_multiplies_to_its_rate_and_the_total_adds_up() {
    let opal = sample();
    let opals = opal.opals();
    for villager in (0..VILLAGERS).filter(|villager| opals[*villager] > 0.0) {
        let tree = opal.breakdown(Some(villager));
        let rate = opal.exp_rate(villager, &opals);
        assert!((tree.value - rate).abs() <= rate * 1e-12);
        let product: f64 = tree.children.iter().map(|child| child.value).product();
        assert!(
            (product - rate).abs() <= rate * 1e-12,
            "{villager}: {product} vs {rate}"
        );
        let factor = tree
            .children
            .iter()
            .find(|child| child.label == "Additive factor")
            .expect("the factor is there");
        let additive = &factor.children[0];
        assert_eq!(additive.sum_children(), additive.value);
        assert_eq!(additive.value, opal.additive_pct(villager, &opals));
        assert_eq!(factor.value, 1.0 + additive.value / 100.0);
    }

    let total = opal.breakdown(None);
    assert!((total.value - opal.total_exp_rate(&opals)).abs() <= total.value * 1e-12);
    assert_eq!(total.sum_children(), total.value);
    assert_eq!(total.children.len(), opal.unlocked_count());
}

#[test]
fn a_share_is_the_villagers_part_of_the_total() {
    let opal = sample();
    let opals = opal.opals();
    let total = opal.total_exp_rate(&opals);
    let expected = opal.exp_rate(0, &opals) / total;
    assert!((opal.share(0) - expected).abs() <= expected * 1e-12);
}

#[test]
fn the_cosmo_bonus_is_the_base_times_the_level_floored() {
    let opal = sample();
    assert_eq!(opal.cosmo_level(1, 0), 5.0);
    assert_eq!(opal.cosmo_level(0, 4), 4.0);
    assert_eq!(opal.cosmo_level(1, 9), 0.0);
    assert_eq!(opal.cosmo_bonus(1, 0), data::cosmo_base(1, 0) * 5.0);
    assert_eq!(opal.cosmo_bonus(0, 0), data::cosmo_base(0, 0) * 4.0);
}

/// Every split of `total` across the unlocked villagers, so a small plan can be
/// checked against the true optimum instead of another heuristic.
fn brute_best(opal: &Opal, target: Target, total: f64) -> f64 {
    fn walk(
        opal: &Opal,
        target: Target,
        unlocked: &[usize],
        index: usize,
        remaining: f64,
        opals: &mut [f64; VILLAGERS],
        best: &mut f64,
    ) {
        let villager = unlocked[index];
        if index + 1 == unlocked.len() {
            opals[villager] = remaining;
            *best = best.max(opal.objective(target, opals));
            opals[villager] = 0.0;
            return;
        }
        let reserved = (unlocked.len() - index - 1) as f64;
        let mut count = 0.0;
        while count + reserved <= remaining {
            opals[villager] = count;
            walk(
                opal,
                target,
                unlocked,
                index + 1,
                remaining - count,
                opals,
                best,
            );
            count += 1.0;
        }
        opals[villager] = 0.0;
    }

    let unlocked: Vec<usize> = (0..VILLAGERS)
        .filter(|villager| opal.unlocked(*villager))
        .collect();
    let mut opals = [0.0; VILLAGERS];
    let mut best = f64::NEG_INFINITY;
    walk(opal, target, &unlocked, 0, total, &mut opals, &mut best);
    best
}

fn best_plan(opal: &Opal, target: Target, total: f64) -> Plan {
    opal.plan(PlanInput {
        target,
        total,
        current: opal.opals(),
    })
}

#[test]
fn a_plan_splits_the_whole_total_across_the_unlocked_villagers() {
    let opal = sample();
    for target in [Target::Total, Target::Villager(0), Target::Villager(4)] {
        for total in [0.0, 1.0, 24.0, 100.0, 369.0] {
            let plan = best_plan(&opal, target, total);
            assert_eq!(plan.total, total);
            assert_eq!(plan.spent(), total, "target {target:?} at {total}");
            assert_eq!(plan.opals[2], 0.0, "the locked villager gets nothing");
            assert_eq!(plan.change(2), 0.0);
        }
    }
}

#[test]
fn a_plan_is_never_worse_than_the_saved_split() {
    let opal = sample();
    let total = opal.total_opals();
    for target in [
        Target::Total,
        Target::Villager(0),
        Target::Villager(1),
        Target::Villager(3),
        Target::Villager(4),
    ] {
        let saved = opal.objective(target, &opal.opals());
        let plan = best_plan(&opal, target, total);
        let after = plan.rate(&opal);
        assert!(
            after >= saved - saved.abs().max(1.0) * 1e-12,
            "{target:?}: {after} beats {saved}"
        );
        assert!(plan.improvement(&opal).is_none_or(|gain| gain >= -1e-12));
    }
}

#[test]
fn a_small_plan_matches_an_exhaustive_search() {
    let opal = sample();
    for target in [
        Target::Total,
        Target::Villager(0),
        Target::Villager(1),
        Target::Villager(4),
    ] {
        for total in [0.0, 7.0, 17.0, 40.0, 60.0] {
            let best = brute_best(&opal, target, total);
            let plan = best_plan(&opal, target, total);
            let rate = plan.rate(&opal);
            assert!(
                rate >= best - best.abs().max(1.0) * 1e-9,
                "{target:?} at {total}: the plan reads {rate}, exhaustive {best}"
            );
        }
    }
}

#[test]
fn a_plan_with_no_target_or_no_villagers_stays_empty() {
    let opal = sample();
    // The second villager is locked, so it cannot be the target.
    let locked = best_plan(&opal, Target::Villager(2), 100.0);
    assert_eq!(locked.spent(), 0.0);

    let mut empty = sample();
    for villager in 0..VILLAGERS {
        empty.villagers[villager].level = 0.0;
    }
    let none = best_plan(&empty, Target::Total, 100.0);
    assert_eq!(none.spent(), 0.0);
}

#[test]
fn a_total_is_whole_and_negative_totals_distribute_nothing() {
    let opal = sample();
    let rounded = best_plan(&opal, Target::Total, 24.7);
    assert_eq!(rounded.total, 24.0);
    assert_eq!(rounded.spent(), 24.0);
    let negative = best_plan(&opal, Target::Total, -5.0);
    assert_eq!(negative.total, 0.0);
    assert_eq!(negative.spent(), 0.0);
    assert_eq!(negative.rate(&opal), 0.0);
}

#[test]
fn the_progress_rises_and_a_cancelled_run_stores_nothing() {
    let opal = sample();
    let mut fractions = Vec::new();
    let input = || PlanInput {
        target: Target::Total,
        total: 1_000.0,
        current: opal.opals(),
    };
    let done = opal.plan_with(input(), |progress| {
        fractions.push(progress.fraction);
        Control::Continue
    });
    assert!(done.is_some());
    assert!(fractions.len() > 1);
    assert!(fractions.windows(2).all(|pair| pair[0] <= pair[1]));
    assert!(
        fractions
            .iter()
            .all(|fraction| (0.0..=1.0).contains(fraction))
    );

    let mut reported = false;
    let cancelled = opal.plan_with(input(), |_| {
        reported = true;
        Control::Stop
    });
    assert!(cancelled.is_none());
    assert!(reported, "the run checks cancellation at its first report");
}

#[test]
fn a_plan_is_stable_when_the_save_already_holds_it() {
    let opal = sample();
    let first = best_plan(&opal, Target::Total, 100.0);
    // Saving the plan and optimizing again must not shuffle equal plans: the
    // tie-break keeps the split that moves the fewest opals.
    let mut saved = opal;
    for villager in 0..VILLAGERS {
        saved.villagers[villager].opals = first.opals[villager];
    }
    let second = best_plan(&saved, Target::Total, 100.0);
    assert_eq!(second.opals, first.opals);
    assert_eq!(second.change(0), 0.0);
}

#[test]
fn the_plan_text_lists_the_unlocked_villagers_and_the_total() {
    let opal = sample();
    let plan = best_plan(&opal, Target::Total, 100.0);
    let text = plan.text(&opal);
    assert!(
        text.starts_with("Opal plan\tAll villagers\t100 opals\nVillager\tOpals\tChange\tEXP/hr\n"),
        "{text}"
    );
    for villager in [0, 1, 3, 4] {
        assert!(
            text.contains(&format!("\n{}\t", name(villager))),
            "{} is listed: {text}",
            name(villager)
        );
    }
    assert!(!text.contains(name(2)), "the locked villager is not listed");
    assert!(text.contains(&format!("Total\t100\t\t{:.0}\n", plan.rate(&opal))));
    assert!(text.contains("Improvement\t"));

    let explorer = best_plan(&opal, Target::Villager(0), 100.0);
    assert!(explorer.text(&opal).starts_with("Opal plan\tExplorer\t"));
}

#[test]
fn a_typed_distribution_reads_whole_non_negative_counts() {
    let opal = sample();
    let simulation = opal.simulation(&[-5.0, 24.9, 7.5, f64::NAN, f64::INFINITY]);
    assert_eq!(simulation.opals, [0.0, 24.0, 7.0, 0.0, 0.0]);
    assert_eq!(simulation.spent(), 31.0);
    assert_eq!(simulation.saved_spent(), opal.total_opals());
    assert!(
        !simulation.balanced(),
        "31 opals do not match the save's 219"
    );
}

#[test]
fn the_saved_split_is_the_balanced_baseline() {
    let opal = sample();
    let simulation = opal.simulation(&opal.opals());
    assert!(simulation.balanced());
    assert_eq!(simulation.min_opals(&opal), opal.min_opals(&opal.opals()));
    assert_eq!(
        simulation.equal_spread(&opal),
        opal.equal_spread(&opal.opals())
    );
    assert_eq!(simulation.total(&opal), opal.total_exp_rate(&opal.opals()));
    assert_eq!(simulation.total_change(&opal), Some(0.0));
    for villager in 0..VILLAGERS {
        let rate = opal.exp_rate(villager, &opal.opals());
        assert_eq!(simulation.rate(&opal, villager), rate);
        assert_eq!(simulation.saved_rate(&opal, villager), rate);
        // A villager without opals earned nothing, so there is no share to report.
        assert_eq!(
            simulation.change(&opal, villager),
            (rate > 0.0).then_some(0.0)
        );
    }
}

#[test]
fn the_typed_total_is_the_rows_added_up() {
    let opal = sample();
    // A balanced swap: the Explorer's ten opals move to the Librarian.
    let typed = [90.0, 24.0, 0.0, 35.0, 70.0];
    let simulation = opal.simulation(&typed);
    assert!(simulation.balanced());
    let sum: f64 = (0..VILLAGERS)
        .map(|villager| simulation.rate(&opal, villager))
        .sum();
    assert!((sum - simulation.total(&opal)).abs() <= simulation.total(&opal).abs() * 1e-12);
}

#[test]
fn the_manual_simulator_reads_the_optimizer_plan_the_same_way() {
    let opal = sample();
    for target in [
        Target::Total,
        Target::Villager(0),
        Target::Villager(1),
        Target::Villager(4),
    ] {
        let plan = opal.plan(PlanInput::saved(&opal, target));
        let simulation = opal.simulation(&plan.opals);
        // The plan's objective is the same rate the simulator prints for those
        // counts, so an equal distribution reads the same in both tabs.
        let read = match target {
            Target::Total => simulation.total(&opal),
            Target::Villager(villager) => simulation.rate(&opal, villager),
        };
        assert_eq!(read, plan.rate(&opal), "{target:?}");
        assert!(simulation.balanced(), "{target:?}");
        assert_eq!(simulation.spent(), plan.total);
        let change = simulation.total_change(&opal).expect("the save earns");
        assert!(change >= -1e-12, "{target:?}: {change}");
    }
}

#[test]
fn a_villager_that_earned_nothing_has_no_share_to_report() {
    let opal = sample();
    // The Conjuror holds no opals in the save, so a typed count has no share to
    // compare against and the page's zero would read as "no change".
    let simulation = opal.simulation(&[100.0, 24.0, 10.0, 35.0, 50.0]);
    assert_eq!(simulation.saved_rate(&opal, 2), 0.0);
    assert!(simulation.rate(&opal, 2) > 0.0);
    assert_eq!(simulation.change(&opal, 2), None);

    // Emptying a villager that did earn reads as the whole loss it is.
    let emptied = opal.simulation(&[0.0, 24.0, 0.0, 35.0, 160.0]);
    assert!(emptied.balanced());
    assert_eq!(emptied.rate(&opal, 0), 0.0);
    assert_eq!(emptied.change(&opal, 0), Some(-1.0));
}
