//! The opal port against the upstream JS.
//!
//! Every figure in `fixtures/opal.json` was produced by `script/opal-oracle.mjs`
//! evaluating `opal-calc.html`'s own `buildExt`, `villagerExpRate` and
//! `totalExpRate` on `fixtures/data.json`, so a drift in either the port or the
//! fixture fails here.

use idleon_core::Opal;
use idleon_core::save::import;
use serde_json::Value;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn oracle() -> Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/opal.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("fixtures/opal.json"))
        .expect("the oracle is JSON")
}

fn opal() -> Opal {
    import(&fixture())
        .expect("the fixture imports")
        .opal
        .expect("the fixture carries the villagers")
}

/// Upstream figures are doubles; compare to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    within(actual, expected, 1e-9);
}

/// The Tome score the save decoder builds omits the star-talent slot (the
/// talent system is not ported), so the Lore Episode behind it and everything
/// that episode multiplies carry a documented tolerance.
#[track_caller]
fn close_tome(actual: f64, expected: f64) {
    within(actual, expected, 2e-3);
}

#[track_caller]
fn within(actual: f64, expected: f64, relative: f64) {
    let tolerance = expected.abs().max(1.0) * relative;
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

#[test]
fn the_villagers_match_upstream() {
    let opal = opal();
    let oracle = oracle();
    for (villager, expected) in oracle["villagers"]
        .as_array()
        .expect("the oracle lists villagers")
        .iter()
        .enumerate()
    {
        let state = opal.villagers[villager];
        close(state.level, at(expected, "level"));
        close(state.opals, at(expected, "opals"));
        close(state.stars, at(expected, "stars"));
        assert_eq!(expected["name"], idleon_core::opal::NAMES[villager]);
    }
    close(opal.ext.terms.scholars, at(&oracle["cosmo"], "scholars"));
    for (index, level) in oracle["cosmo"]["tier1"]
        .as_array()
        .expect("the oracle lists the cosmo levels")
        .iter()
        .enumerate()
    {
        let expected = level.as_f64().expect("a cosmo level is a number");
        close(opal.cosmo_level(1, index), expected);
    }
}

#[test]
fn the_account_bonuses_match_upstream() {
    let opal = opal();
    let terms = opal.ext.terms;
    let ext = &oracle()["ext"];
    close(opal.ext.enthusiasm, at(ext, "cosmoBonus_1_0"));
    close(opal.ext.equal_spread, at(ext, "cosmoBonus_1_5"));
    close_tome(terms.additive(), at(ext, "additiveBase"));
    close_tome(terms.base_multiplier(), at(ext, "baseMulti"));
    for villager in 0..idleon_core::opal::VILLAGERS {
        close(
            opal.ext.star_mul[villager],
            ext["starMul"][villager].as_f64().unwrap(),
        );
        close(
            opal.ext.holeoz[villager],
            ext["holeozArr"][villager].as_f64().unwrap(),
        );
        assert_eq!(
            opal.unlocked(villager),
            ext["unlocked"][villager].as_bool().unwrap()
        );
    }
    assert_eq!(
        opal.unlocked_count(),
        ext["numUnlocked"].as_u64().unwrap() as usize
    );
}

#[test]
fn every_tree_term_matches_upstream() {
    let opal = opal();
    let terms = opal.ext.terms;
    let oracle = oracle();
    let expected = &oracle["terms"];
    let named = [
        ("basePlusUpg", terms.base_plus_upg),
        ("exotic51", terms.exotic),
        ("arcade38", terms.arcade),
        ("grimoire29", terms.grimoire),
        ("arcane32", terms.arcane),
        ("mon03", terms.mon_bravery),
        ("mon13", terms.mon_justice),
        ("mon23", terms.mon_wisdom),
        ("meas7", terms.meas_slab),
        ("meas0", terms.meas_tome),
        ("bUpg48", terms.b_upg48),
        ("cardCaveB", terms.card_cave_b),
        ("bell1_0", terms.bell1),
        ("summWin21", terms.summon_win21),
        ("vaultUpg73", terms.vault73),
        ("cosmoBonus_1_1", terms.cosmo_1_1),
        ("cosSchBlt", terms.scholars),
        ("cosmoBonus_1_2", terms.cosmo_1_2),
        ("compass59", terms.compass59),
        ("legend12", terms.legend12),
        ("pristine21", terms.pristine21),
        ("comp13", terms.companion13),
        ("statue28", terms.statue28),
        ("jarSum", terms.jar_sum),
        ("evShop6", terms.event_shop6),
        ("bundleU", terms.bundle_u),
    ];
    for (name, value) in named {
        expected
            .get(name)
            .unwrap_or_else(|| panic!("{name} is in the oracle"));
        close(value, at(expected, name));
    }
    // The chain and the lore episode inside it read the Tome score.
    close_tome(terms.chain(), at(expected, "multiChain"));
    close_tome(terms.lore_episode1, at(expected, "loreEpi1"));
}

#[test]
fn the_current_state_matches_upstream() {
    let opal = opal();
    let oracle = oracle();
    let current = &oracle["current"];
    let opals = opal.opals();
    close(opal.min_opals(&opals), at(current, "minOpals"));
    close(opal.equal_spread(&opals), at(current, "equalSpread"));
    close(opal.total_opals(), at(current, "totalOpals"));
    for (villager, expected) in current["rates"]
        .as_array()
        .expect("the oracle lists the rates")
        .iter()
        .enumerate()
    {
        close_tome(
            opal.exp_rate(villager, &opals),
            expected.as_f64().expect("a rate is a number"),
        );
    }
    close_tome(opal.total_exp_rate(&opals), at(current, "total"));
}

#[test]
fn the_breakdown_sums_and_multiplies_to_the_rate() {
    let opal = opal();
    let opals = opal.opals();
    for villager in (0..idleon_core::opal::VILLAGERS).filter(|villager| opals[*villager] > 0.0) {
        let rate = opal.exp_rate(villager, &opals);
        let tree = opal.breakdown(Some(villager));
        close(tree.value, rate);
        let product: f64 = tree.children.iter().map(|child| child.value).product();
        assert!(
            (product - rate).abs() <= rate.abs() * 1e-9,
            "villager {villager}: the factors multiply to the rate, {product} vs {rate}"
        );
        let factor = tree
            .children
            .iter()
            .find(|child| child.label == "Additive factor")
            .expect("the tree carries the additive factor");
        let additive = &factor.children[0];
        close(additive.sum_children(), additive.value);
        close(additive.value, opal.additive_pct(villager, &opals));
        close(factor.value, 1.0 + additive.value / 100.0);
    }

    let total = opal.breakdown(None);
    close(total.value, opal.total_exp_rate(&opals));
    close(total.sum_children(), total.value);
    assert_eq!(total.children.len(), opal.unlocked_count());
}

#[test]
fn a_villager_with_no_opals_earns_nothing() {
    let opal = opal();
    let mut opals = opal.opals();
    opals[0] = 0.0;
    assert_eq!(opal.exp_rate(0, &opals), 0.0);
    // The tree always describes the saved opals, so it keeps reporting them.
    let tree = opal.breakdown(Some(0));
    close(tree.value, opal.exp_rate(0, &opal.opals()));
}

#[test]
fn the_chain_and_the_outer_factors_multiply_to_the_base() {
    let opal = opal();
    let terms = opal.ext.terms;
    let chain = terms.chain();
    close(
        terms.chain_factors().iter().product::<f64>().max(1.0),
        chain,
    );
    let base = terms.base_plus_upg
        * chain
        * terms
            .outer_factors()
            .iter()
            .map(|(_, factor)| *factor)
            .product::<f64>();
    close(terms.base_multiplier(), base);
}

/// The fixture's total, every villager unlocked, and the page's own optimizer
/// output for the total and each villager target.
#[test]
fn the_plan_at_least_matches_the_upstream_optimizer() {
    let opal = opal();
    let oracle = oracle();
    let optimizer = &oracle["optimizer"];
    let total = at(&oracle["current"], "totalOpals");
    let input = |target| idleon_core::opal::PlanInput {
        target,
        total,
        current: opal.opals(),
    };

    let upstream = &optimizer["total"];
    let plan = opal.plan(input(idleon_core::opal::PlanTarget::Total));
    close(plan.spent(), total);
    // The page's two greedy shapes miss plans, so the port has to be at least
    // as good, not identical: the same tolerance the rates use, in one direction.
    assert!(
        plan.rate(&opal) >= at(upstream, "rate") * (1.0 - 2e-3),
        "the plan beats the page's own total split: {} vs {}",
        plan.rate(&opal),
        at(upstream, "rate")
    );
    let expected = upstream["opals"]
        .as_array()
        .expect("the oracle lists opals");
    assert_eq!(expected.len(), idleon_core::opal::VILLAGERS);

    for (villager, expected) in optimizer["targets"]
        .as_array()
        .expect("the oracle lists targets")
        .iter()
        .enumerate()
    {
        let target = idleon_core::opal::PlanTarget::Villager(villager);
        let plan = opal.plan(input(target));
        close(plan.spent(), total);
        assert!(
            plan.rate(&opal) >= at(expected, "rate") * (1.0 - 2e-3),
            "villager {villager}: {} beats {}",
            plan.rate(&opal),
            at(expected, "rate")
        );
        let opals = expected["opals"]
            .as_array()
            .expect("the oracle lists opals");
        assert_eq!(opals.len(), idleon_core::opal::VILLAGERS);
    }
}

#[test]
fn a_plan_is_never_worse_than_the_saved_distribution() {
    let opal = opal();
    let total = opal.total_opals();
    let targets = std::iter::once(idleon_core::opal::PlanTarget::Total).chain(
        (0..idleon_core::opal::VILLAGERS)
            .filter(|villager| opal.unlocked(*villager))
            .map(idleon_core::opal::PlanTarget::Villager),
    );
    for target in targets {
        let plan = opal.plan(idleon_core::opal::PlanInput {
            target,
            total,
            current: opal.opals(),
        });
        let before = opal.objective(target, &opal.opals());
        let after = plan.rate(&opal);
        assert!(
            after >= before - before * 1e-12,
            "{target:?}: the plan reads {after}, the save {before}"
        );
        close(plan.spent(), total);
        assert_eq!(plan.unallocated(), 0.0);
        for villager in 0..idleon_core::opal::VILLAGERS {
            assert_eq!(plan.opals[villager].fract(), 0.0, "whole opals only");
            if !opal.unlocked(villager) {
                assert_eq!(plan.opals[villager], 0.0, "locked villagers get none");
            }
        }
    }
}

#[test]
fn the_plan_text_lists_every_unlocked_villager() {
    let opal = opal();
    let plan = opal.plan(idleon_core::opal::PlanInput::saved(
        &opal,
        idleon_core::opal::PlanTarget::Total,
    ));
    let text = plan.text(&opal);
    for (villager, name) in idleon_core::opal::NAMES.iter().enumerate() {
        if opal.unlocked(villager) {
            assert!(text.contains(&format!("\n{name}\t")), "{name} is listed");
        }
    }
    assert!(text.contains("Total\t369\t"));
}
