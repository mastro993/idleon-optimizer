//! The opal maths, checked against the page's own formulas.

use super::*;

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
