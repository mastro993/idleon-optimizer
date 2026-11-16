//! JO-03: the formula breakdown invariants the acceptance list names: a
//! breakdown multiplies back to the number its section reports, and a breakdown
//! that the save cannot compute says which input is missing instead of printing
//! one.

mod common;

use common::{clear, close, save, small_board};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_jelly::BreakdownCategory;
use idleon_jelly::board::Layout;

/// Every `Additive …` group over percentages turns its children into its factor.
fn assert_additive_groups(node: &FormulaNode, path: &str) {
    let percentages = |node: &FormulaNode| {
        node.label.starts_with("Additive ")
            && node
                .children
                .iter()
                .all(|child| child.format == ValueFormat::Percent)
    };
    if percentages(node) {
        let total: f64 = node.children.iter().map(|child| child.value).sum();
        close(
            node.value,
            1.0 + total / 100.0,
            &format!("{path} adds its children"),
        );
    }
    for (index, child) in node.children.iter().enumerate() {
        assert_additive_groups(child, &format!("{path}/{index}"));
    }
}

/// A breakdown's factors multiply back to the number it reports.
#[track_caller]
fn assert_multiplies(root: &FormulaNode, what: &str) {
    let product: f64 = root.children.iter().map(|child| child.value).product();
    close(
        product,
        root.value,
        &format!("{what} multiplies to its root"),
    );
    assert_additive_groups(root, what);
}

#[test]
fn every_breakdown_multiplies_to_the_number_it_reports() {
    for (name, jelly) in [
        ("save", save()),
        ("small-board", small_board()),
        ("clear", clear()),
    ] {
        let fever = Some(jelly.progress.fever);
        let options = idleon_jelly::MetricOptions {
            fever,
            ..idleon_jelly::MetricOptions::default()
        };
        for category in BreakdownCategory::ALL {
            let breakdown = jelly.breakdown(category, &jelly.layout, options);
            assert_eq!(
                breakdown.category, category,
                "{name}: the tree names itself"
            );
            assert_multiplies(&breakdown.root, &format!("{name} {}", category.id()));
        }
        let layout = jelly.layout_breakdown(&jelly.layout, fever);
        assert_additive_groups(&layout.damage, &format!("{name} layout damage"));
        assert_additive_groups(&layout.speed, &format!("{name} layout speed"));
    }
}

#[test]
fn a_breakdown_the_save_cannot_compute_names_what_is_missing() {
    let mut jelly = save();
    jelly.upgrades[10] = 0;
    jelly.upgrades[23] = 0;
    let fever = Some(jelly.progress.fever);
    let exp = jelly.exp_breakdown(fever);
    assert!(!exp.available, "Cell EXP is locked without Cell Biology");
    assert!(
        exp.reason.contains("Cell Biology"),
        "the reason names the upgrade: {}",
        exp.reason
    );
    let bloodcells = jelly.bloodcell_breakdown(idleon_jelly::MetricOptions {
        fever,
        ..idleon_jelly::MetricOptions::default()
    });
    assert!(
        !bloodcells.available,
        "Bloodcells are locked without upgrade 23"
    );
    assert!(
        bloodcells.reason.contains("Bloodcell Coagulation"),
        "the reason names the upgrade: {}",
        bloodcells.reason
    );

    // Damage, speed and a valid board are always available.
    assert!(
        jelly
            .damage_breakdown(idleon_jelly::MetricOptions {
                fever,
                ..idleon_jelly::MetricOptions::default()
            })
            .available
    );

    // An invalid board says why instead of printing a guessed multiplier.
    let mut overlap = Layout::new();
    overlap.insert(77, 1);
    overlap.insert(78, 0);
    let layout = jelly.layout_breakdown(&overlap, fever);
    assert!(!layout.available);
    assert!(!layout.reason.is_empty(), "the refusal explains itself");
    let category = jelly.breakdown(
        BreakdownCategory::LayoutDamage,
        &overlap,
        idleon_jelly::MetricOptions::default(),
    );
    assert!(!category.available);
    assert_eq!(category.reason, layout.reason);
}

#[test]
fn the_reward_interface_reports_its_own_unlock() {
    let mut jelly = save();
    jelly.upgrades[39] = 0;
    assert!(!jelly.bonuses_unlocked());
    jelly.upgrades[39] = 1;
    assert!(jelly.bonuses_unlocked());
}

/// The helper both breakdown tests share: `Jelly::breakdown` returns the same
/// tree the direct call builds.
#[test]
fn the_category_selector_builds_the_same_trees() {
    let jelly = save();
    let options = idleon_jelly::MetricOptions::default();
    let categories = [
        (
            BreakdownCategory::Damage,
            jelly.damage_breakdown(options).root,
        ),
        (BreakdownCategory::Speed, jelly.speed_breakdown(None).root),
        (BreakdownCategory::Exp, jelly.exp_breakdown(None).root),
        (
            BreakdownCategory::Bloodcells,
            jelly.bloodcell_breakdown(options).root,
        ),
        (
            BreakdownCategory::LayoutDamage,
            jelly.layout_breakdown(&jelly.layout, None).damage,
        ),
        (
            BreakdownCategory::LayoutSpeed,
            jelly.layout_breakdown(&jelly.layout, None).speed,
        ),
    ];
    for (category, root) in categories {
        assert_eq!(
            jelly.breakdown(category, &jelly.layout, options).root,
            root,
            "{}",
            category.id()
        );
    }
}
