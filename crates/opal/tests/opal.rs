//! The opal rates, breakdown and plan invariants on the committed synthetic
//! save.

use idleon_import::import;
use idleon_opal::Opal;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn opal() -> Opal {
    import(&fixture())
        .expect("the fixture imports")
        .opal
        .expect("the fixture carries the villagers")
}

/// The rates are doubles; compare to the last few bits they share.
#[track_caller]
fn close(actual: f64, expected: f64) {
    let tolerance = expected.abs().max(1.0) * 1e-9;
    assert!(
        (actual - expected).abs() <= tolerance,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn the_breakdown_sums_and_multiplies_to_the_rate() {
    let opal = opal();
    let opals = opal.opals();
    for villager in (0..idleon_opal::VILLAGERS).filter(|villager| opals[*villager] > 0.0) {
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

#[test]
fn a_plan_is_never_worse_than_the_saved_distribution() {
    let opal = opal();
    let total = opal.total_opals();
    let targets = std::iter::once(idleon_opal::PlanTarget::Total).chain(
        (0..idleon_opal::VILLAGERS)
            .filter(|villager| opal.unlocked(*villager))
            .map(idleon_opal::PlanTarget::Villager),
    );
    for target in targets {
        let plan = opal.plan(idleon_opal::PlanInput {
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
        for (villager, &opals) in plan.opals.iter().enumerate() {
            assert_eq!(opals.fract(), 0.0, "whole opals only");
            if !opal.unlocked(villager) {
                assert_eq!(opals, 0.0, "locked villagers get none");
            }
        }
    }
}

#[test]
fn the_plan_text_lists_every_unlocked_villager() {
    let opal = opal();
    let plan = opal.plan(idleon_opal::PlanInput::saved(
        &opal,
        idleon_opal::PlanTarget::Total,
    ));
    let text = plan.text(&opal);
    for (villager, name) in idleon_opal::NAMES.iter().enumerate() {
        if opal.unlocked(villager) {
            assert!(text.contains(&format!("\n{name}\t")), "{name} is listed");
        }
    }
    assert!(text.contains("Total\t369\t"));
}
