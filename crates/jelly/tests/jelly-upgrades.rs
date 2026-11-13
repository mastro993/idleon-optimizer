//! JO-03: the upgrade list, the obstruction rewards and the formula
//! breakdowns against the upstream JS.
//!
//! Every expected figure and string comes from `fixtures/jelly.json`, which
//! `script/jelly-oracle.mjs` writes by running the upstream calculator on the
//! committed save and on its documented overrides. The oracle prints the page's
//! own `jellyUpgradeDisplay`, `jellyRewardTarget`, `gameDescription`,
//! `rewardEffectText` and the five breakdown functions, so a drift here is the
//! port's.
//!
//! The last two tests hold the invariants the acceptance list names: a breakdown
//! multiplies back to the number its section reports, and a breakdown that the
//! save cannot compute says which input is missing instead of printing one.

mod common;

use common::{close, jelly};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_jelly::BreakdownCategory;
use idleon_jelly::board::Layout;
use serde_json::Value;

const CASES: [&str; 3] = ["save", "small-board", "clear"];

fn number(value: &Value, key: &str) -> f64 {
    value[key]
        .as_f64()
        .unwrap_or_else(|| panic!("{key} is a number"))
}

fn flag(value: &Value, key: &str) -> bool {
    value[key]
        .as_bool()
        .unwrap_or_else(|| panic!("{key} is a bool"))
}

fn text<'a>(value: &'a Value, key: &str) -> &'a str {
    value[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is a string"))
}

#[test]
fn the_upgrade_list_matches_upstream() {
    for name in CASES {
        let (jelly, case) = jelly(name);
        let expected = case["upgrades"]
            .as_array()
            .expect("the oracle lists upgrades");
        let rows = jelly.upgrade_rows();
        assert_eq!(rows.len(), expected.len(), "{name}: upgrade rows");
        for (row, expected) in rows.iter().zip(expected) {
            let label = format!("{name} #{} {}", row.order + 1, row.name);
            assert_eq!(
                row.order,
                number(expected, "order") as usize,
                "{label}: order"
            );
            assert_eq!(row.id, number(expected, "id") as usize, "{label}: id");
            assert_eq!(row.name, text(expected, "name"), "{label}: name");
            assert_eq!(
                row.description,
                text(expected, "description"),
                "{label}: description"
            );
            assert_eq!(row.effect, text(expected, "effectText"), "{label}: effect");
            assert_eq!(
                row.per_level,
                text(expected, "perLevelText"),
                "{label}: per level"
            );
            assert_eq!(row.note, text(expected, "sourceNote"), "{label}: note");
            close(
                row.level,
                number(expected, "level"),
                &format!("{label}: level"),
            );
            close(
                row.quantity,
                number(expected, "quantity"),
                &format!("{label}: quantity"),
            );
            close(
                row.max_level,
                number(expected, "maxLevel"),
                &format!("{label}: max level"),
            );
            assert_eq!(
                row.doubled,
                flag(expected, "doubled"),
                "{label}: Upgrade Doubler"
            );
            close(
                jelly.upgrade_cost(row.order),
                number(expected, "cost"),
                &format!("{label}: cost"),
            );
            let status = &expected["status"];
            let got = row.status;
            close(
                got.required_research_level,
                number(status, "requiredResearchLevel"),
                &format!("{label}: required Research level"),
            );
            close(
                got.research_level,
                number(status, "researchLevel"),
                &format!("{label}: Research level"),
            );
            for (key, got) in [
                ("levelVisible", got.level_visible),
                ("prerequisiteMet", got.prerequisite_met),
                ("belowMax", got.below_max),
                ("affordable", got.affordable),
                ("canBuy", got.can_buy),
            ] {
                assert_eq!(got, flag(status, key), "{label}: {key}");
            }
        }
    }
}

#[test]
fn the_obstruction_rewards_match_upstream() {
    for name in CASES {
        let (jelly, case) = jelly(name);
        let expected = case["bonuses"]
            .as_array()
            .expect("the oracle lists rewards");
        let bonuses = jelly.bonuses();
        assert_eq!(bonuses.len(), expected.len(), "{name}: reward rows");
        for (bonus, expected) in bonuses.iter().zip(expected) {
            let label = format!("{name} #{} {}", bonus.index + 1, bonus.name);
            assert_eq!(
                bonus.index,
                number(expected, "index") as usize,
                "{label}: index"
            );
            assert_eq!(bonus.name, text(expected, "name"), "{label}: name");
            assert_eq!(bonus.target, text(expected, "target"), "{label}: target");
            assert_eq!(
                bonus.description,
                text(expected, "description"),
                "{label}: description"
            );
            assert_eq!(
                bonus.effect,
                text(expected, "effectText"),
                "{label}: effect"
            );
            assert_eq!(bonus.active, flag(expected, "active"), "{label}: active");
            close(
                bonus.value,
                number(expected, "value"),
                &format!("{label}: value"),
            );
        }
    }
}

/// Every field of one tree, node by node.
#[track_caller]
fn assert_tree(got: &FormulaNode, expected: &Value, path: &str) {
    assert_eq!(got.label, text(expected, "label"), "{path}: label");
    close(
        got.value,
        number(expected, "value"),
        &format!("{path}: value"),
    );
    let format = match text(expected, "unit") {
        "x" => ValueFormat::Multiplier,
        "%" => ValueFormat::Percent,
        other => {
            assert_eq!(other, "", "{path}: unexpected unit");
            ValueFormat::Raw
        }
    };
    assert_eq!(got.format, format, "{path}: format");
    let children = expected["children"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or_default();
    assert_eq!(got.children.len(), children.len(), "{path}: children");
    for (index, (child, expected)) in got.children.iter().zip(children).enumerate() {
        assert_tree(child, expected, &format!("{path}/{index}"));
    }
}

#[test]
fn the_breakdowns_match_upstream() {
    for name in CASES {
        let (jelly, case) = jelly(name);
        let expected = &case["breakdowns"];
        let fever = Some(jelly.progress.fever);
        let damage = jelly.damage_breakdown(idleon_jelly::MetricOptions {
            fever,
            ..idleon_jelly::MetricOptions::default()
        });
        assert_tree(&damage.root, &expected["damage"], &format!("{name} damage"));
        let speed = jelly.speed_breakdown(fever);
        close(
            speed.root.value,
            number(&expected["speed"], "value"),
            &format!("{name} speed"),
        );
        assert_tree(&speed.root, &expected["speed"], &format!("{name} speed"));
        let exp = jelly.exp_breakdown(fever);
        assert_tree(&exp.root, &expected["exp"], &format!("{name} exp"));
        let bloodcells = jelly.bloodcell_breakdown(idleon_jelly::MetricOptions {
            fever,
            ..idleon_jelly::MetricOptions::default()
        });
        assert_tree(
            &bloodcells.root,
            &expected["bloodcells"],
            &format!("{name} bloodcells"),
        );

        let layout = jelly.layout_breakdown(&jelly.layout, fever);
        assert!(layout.available, "{name}: the saved board is valid");
        assert_tree(
            &layout.damage,
            &expected["layout"]["damage"],
            &format!("{name} layout damage"),
        );
        assert_tree(
            &layout.speed,
            &expected["layout"]["speed"],
            &format!("{name} layout speed"),
        );
        let rows = layout.cell_rows();
        let expected_rows = expected["layout"]["cellRows"]
            .as_array()
            .expect("the oracle lists the cell rows");
        assert_eq!(rows.len(), expected_rows.len(), "{name}: cell rows");
        for (row, expected) in rows.iter().zip(expected_rows) {
            let label = format!("{name} {}", row.name);
            assert_eq!(
                row.cell_type,
                number(expected, "type") as usize,
                "{label}: type"
            );
            assert_eq!(row.name, text(expected, "name"), "{label}: name");
            assert_eq!(
                row.placed,
                number(expected, "placed") as usize,
                "{label}: placed"
            );
            close(
                row.average_hit,
                number(expected, "averageHit"),
                &format!("{label}: hit"),
            );
            close(
                row.attacks_per_second,
                number(expected, "attacksPerSecond"),
                &format!("{label}: attacks"),
            );
            close(row.dps, number(expected, "dps"), &format!("{label}: DPS"));
        }
        let placed: usize = rows.iter().map(|row| row.placed).sum();
        assert_eq!(
            placed,
            jelly.layout.len(),
            "{name}: every placed cell is in one row"
        );
        let dps: f64 = rows.iter().map(|row| row.dps).sum();
        close(
            dps,
            layout.metrics.dps,
            &format!("{name}: the rows add up to the board DPS"),
        );
    }
}

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
    for name in CASES {
        let (jelly, case) = jelly(name);
        let fever = Some(jelly.progress.fever);
        let options = idleon_jelly::MetricOptions {
            fever,
            ..idleon_jelly::MetricOptions::default()
        };
        for category in BreakdownCategory::ALL {
            let breakdown = jelly.breakdown(category, &jelly.layout, options);
            assert_multiplies(&breakdown.root, &format!("{name} {}", category.id()));
        }
        let layout = jelly.layout_breakdown(&jelly.layout, fever);
        assert_additive_groups(&layout.damage, &format!("{name} layout damage"));
        assert_additive_groups(&layout.speed, &format!("{name} layout speed"));
        close(
            layout.damage_multiplier(),
            case["breakdowns"]["layout"]["damagePassive"]
                .as_f64()
                .unwrap()
                * case["breakdowns"]["layout"]["virusMultiplier"]
                    .as_f64()
                    .unwrap(),
            &format!("{name}: the layout damage multiplier"),
        );
    }
}

#[test]
fn a_breakdown_the_save_cannot_compute_names_what_is_missing() {
    let (mut jelly, _) = jelly("save");
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
    let (mut jelly, _) = jelly("save");
    jelly.upgrades[39] = 0;
    assert!(!jelly.bonuses_unlocked());
    jelly.upgrades[39] = 1;
    assert!(jelly.bonuses_unlocked());
}

/// The helper both breakdown tests share: `Jelly::breakdown` returns the same
/// tree the direct call builds.
#[test]
fn the_category_selector_builds_the_same_trees() {
    let (jelly, _) = jelly("save");
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
