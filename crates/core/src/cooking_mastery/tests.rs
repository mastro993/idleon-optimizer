//! The Mastery EXP tab's maths, checked against the page's own formulas.

use super::*;

/// A save-shaped state with every upgrade unlocked except SAVORY.
fn sample() -> Mastery {
    Mastery {
        per_lv: [200.0, 1.0, 30.0, 10.0, 2.0, 20.0],
        level: 30.0,
        ladles: 1_000_000_000.0,
        cooking_levels: 1_163.0,
        divorce_cake: 150.0,
        ribbons: 646.0,
        upgrades: [7.0, 3.0, 5.0, 10.0, 2.0, 0.0],
        companion_87_owned: false,
        external: External {
            menagerie: 60.0,
            grid_190: 303.0,
            super_bit_68: 1.0,
            companion_87: 0.5,
            fountain: 120.0,
            vial: 40.0,
            arcade: 41.860_465_116_279_07,
            salt_lick: 73.0,
            vial_level: 200.0,
        },
    }
}

#[test]
fn scaling_is_what_each_upgrade_scales_with() {
    let mastery = sample();
    // `getLOG` divides by the game's truncated `ln 10`, so 1e9 is a hair over 9.
    assert!((mastery.scaling(0) - 9.0).abs() < 1e-4, "log10(1e9) = 9");
    assert!((mastery.scaling(1) - 163.0).abs() < 1e-9);
    assert!((mastery.scaling(2) - 75.0).abs() < 1e-9);
    assert!((mastery.scaling(4) - 646.0).abs() < 1e-9);
    assert!((mastery.scaling(5) - 31.0).abs() < 1e-9);
    // SMOKY scales with ribbon ranks too, and is still not an EXP upgrade.
    assert!(!EXP_UPGRADES.contains(&RIBBON_CHANCE));
    assert_eq!(mastery.scaling(RIBBON_CHANCE), 0.0);
}

#[test]
fn a_bonus_is_per_level_times_level_times_scaling() {
    let mastery = sample();
    assert!((mastery.bonus(0, 7.0) - 200.0 * 7.0 * mastery.scaling(0)).abs() < 1e-9);
    assert!((mastery.bonus(2, 5.0) - 30.0 * 5.0 * 75.0).abs() < 1e-9);
    assert!((mastery.multiplier(2, 5.0) - (1.0 + 11_250.0 / 100.0)).abs() < 1e-9);
}

#[test]
fn the_rate_is_the_base_times_every_factor() {
    let mastery = sample();
    let upgrades: f64 = EXP_UPGRADES.iter().fold(2.0, |rate, index| {
        rate * (1.0 + mastery.bonus(*index, mastery.upgrades[*index]) / 100.0)
    });
    let expected = upgrades * mastery.external.total();
    assert!((mastery.exp_rate() - expected).abs() <= expected * 1e-12);
}

#[test]
fn the_breakdown_root_is_the_rate_and_its_factors_multiply_to_it() {
    let mastery = sample();
    let tree = mastery.breakdown();
    assert_eq!(tree.label, "EXP Rate");
    assert!((tree.value - mastery.exp_rate()).abs() < 1e-6);

    // Every child of the root is a factor, not an addend.
    let product: f64 = tree.children.iter().map(|child| child.value).product();
    assert!(
        (product - tree.value).abs() <= tree.value.abs() * 1e-12,
        "the factors multiply to the root: {product} vs {}",
        tree.value
    );

    // One node per factor: the base, the five EXP upgrades and the externals.
    assert_eq!(tree.children.len(), 1 + EXP_UPGRADES.len() + 1);
    let building = tree
        .children
        .iter()
        .find(|child| child.label == "External Multipliers")
        .expect("the external group is there");
    let externals: f64 = building.children.iter().map(|child| child.value).product();
    assert!((externals - building.value).abs() <= building.value * 1e-12);

    // The additive group's children sum to the percentage it reports.
    let additive = building
        .children
        .iter()
        .find(|child| child.label == "Additive EXP Bonuses")
        .expect("the additive group is there");
    assert!((additive.sum_children() - mastery.external.additive()).abs() < 1e-9);
    assert!((additive.value - (1.0 + mastery.external.additive() / 100.0)).abs() < 1e-12);
}

#[test]
fn the_ranking_puts_the_highest_gain_first_and_is_stable() {
    let mastery = sample();
    let ranking = mastery.ranking();
    assert_eq!(ranking.len(), EXP_UPGRADES.len());
    assert!(
        ranking.windows(2).all(|pair| {
            pair[0].gain_pct > pair[1].gain_pct
                || (pair[0].gain_pct == pair[1].gain_pct && pair[0].index < pair[1].index)
        }),
        "{:?}",
        ranking.iter().map(|row| row.gain_pct).collect::<Vec<_>>()
    );
    // The same state ranks the same way every time.
    assert_eq!(mastery.ranking(), ranking);
    for row in ranking.iter().filter(|row| !row.locked) {
        assert!((row.multiplier - mastery.multiplier(row.index, row.level)).abs() < 1e-12);
        assert!((row.rate_after - mastery.exp_rate_with(row.index, row.level + 1.0)).abs() < 1e-6);
    }
    let best = &ranking[0];
    assert!(
        best.gain > 0.0,
        "the best unlocked upgrade adds EXP: {:?}",
        best.name
    );
}

#[test]
fn a_locked_upgrade_gains_nothing_until_the_rank_unlocks_it() {
    let locked = Mastery {
        level: 24.0,
        ..sample()
    };
    let ranking = locked.ranking();
    let source = ranking
        .iter()
        .find(|row| row.index == 4)
        .expect("SOUR is ranked");
    assert!(source.locked, "SOUR needs mastery 25");
    assert_eq!(source.required_level, 25);
    assert_eq!(source.gain, 0.0);
    assert!((source.rate_after - locked.exp_rate()).abs() < 1e-9);
    // A locked row still reports the bonus a point would give, like the page.
    assert!(source.next_bonus > source.bonus);
    // SAVORY needs rank 100 and is ranked too, also without a gain.
    let savoury = ranking
        .iter()
        .find(|row| row.index == 5)
        .expect("SAVORY is an EXP upgrade");
    assert!(savoury.locked);
    assert_eq!(savoury.gain, 0.0);

    let unlocked = Mastery {
        level: 25.0,
        ..sample()
    };
    let ranking = unlocked.ranking();
    let source = ranking
        .iter()
        .find(|row| row.index == 4)
        .expect("SOUR is ranked");
    assert!(!source.locked);
    assert!(source.gain > 0.0);
}

#[test]
fn exp_required_follows_the_page_curve() {
    let mastery = Mastery {
        level: 18.0,
        ..sample()
    };
    let expected = 100.0 * 2.5f64.powf(18.0);
    assert!((mastery.exp_required() - expected).abs() <= expected * 1e-12);
    // Past 40 the level adds the second factor.
    let high = Mastery {
        level: 50.0,
        ..sample()
    };
    let expected = 100.0 * 2.5f64.powf(50.0) * 5f64.powf(10.0);
    assert!((high.exp_required() / expected - 1.0).abs() < 1e-12);
}

#[test]
fn points_available_subtracts_what_the_upgrades_hold() {
    let mastery = sample();
    // 30 + 1 points from the rank, 17 spent on the upgrades.
    assert!((mastery.points_spent() - 27.0).abs() < 1e-9);
    assert!((mastery.points_available() - 4.0).abs() < 1e-9);
    let with_companion = Mastery {
        companion_87_owned: true,
        ..mastery
    };
    assert!((with_companion.points_earned() - 36.0).abs() < 1e-9);
    assert!((with_companion.points_available() - 9.0).abs() < 1e-9);
    // A save that has spent more than it earned never goes negative.
    let overspent = Mastery {
        upgrades: [100.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        ..with_companion
    };
    assert_eq!(overspent.points_available(), 0.0);
}

#[test]
fn hours_to_the_next_level_is_the_requirement_over_the_rate() {
    let mastery = sample();
    assert!(
        (mastery.hours_to_next_level() * mastery.exp_rate() - mastery.exp_required()).abs() < 1e-6
    );
    // A fresh account with no points spent and no external bonus: the rate is
    // the bare base, and the hours are the level's requirement over it.
    let bare = Mastery {
        per_lv: sample().per_lv,
        level: 18.0,
        upgrades: [0.0; 6],
        ..Mastery::default()
    };
    assert_eq!(bare.exp_rate(), 2.0);
    assert!((bare.hours_to_next_level() - bare.exp_required() / 2.0).abs() < 1e-9);
}

#[test]
fn the_tree_carries_the_source_of_every_factor() {
    let tree = sample().breakdown();
    let text = tree.copy_text();
    for expected in [
        "EXP Rate",
        "Base: 2",
        "SALTY — Ladles Used",
        "Masterius Cookerius (Research)",
        "Canteen Read (Vial)",
        "Salt Lick: Cooking Mastery EXP",
        "RandoListo2[8]",
        "cooking-mastery-calc.html · computeExternalMulti",
    ] {
        assert!(
            text.contains(expected),
            "{expected} is in the tree:\n{text}"
        );
    }
    // Every factor is named, so the tree is searchable by its source too.
    assert!(tree.match_count("superBitType(68)") >= 1);
    assert_eq!(tree.keys().len(), tree.node_count());
}
