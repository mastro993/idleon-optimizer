//! The EXP rate tree behind the Mastery EXP tab (`renderBreakdownTree`).
//!
//! The root is the rate the KPI cards show, and the terms below it are the
//! factors that multiply to it: the base rate, one node per EXP upgrade with its
//! level, per-level bonus and scaling, and the external multipliers. The
//! additive group keeps its children summing to the percentage it reports, so
//! the tree accounts for every factor and not only the group totals.

use super::{EXP_UPGRADES, External, Mastery, UPGRADE_NAMES};
use idleon_common::formula::{FormulaNode, ValueFormat};

/// The full Mastery EXP/hour formula tree.
pub(crate) fn tree(mastery: &Mastery) -> FormulaNode {
    let rate = mastery.exp_rate();
    let mut children = vec![
        FormulaNode::leaf("Base", super::BASE_RATE, ValueFormat::Raw)
            .with_note("computeExpRate starts every account at 2 per hour"),
    ];
    for index in EXP_UPGRADES {
        children.push(upgrade(mastery, index));
    }
    children.push(external(&mastery.external));
    FormulaNode::new("EXP Rate", rate)
        .with_format(ValueFormat::Rate)
        .with_input(format!(
            "{} × {:.2} external",
            super::BASE_RATE,
            mastery.external.total()
        ))
        .with_source("cooking-mastery-calc.html · computeExpRate")
        .with_note("Every factor is a multiplier: the tree's root is the rate the cards show.")
        .with_children(children)
}

/// One EXP upgrade: its level, per-level bonus, scaling and resulting factor.
fn upgrade(mastery: &Mastery, index: usize) -> FormulaNode {
    let level = mastery.upgrades[index];
    let bonus = mastery.bonus(index, level);
    let multiplier = mastery.multiplier(index, level);
    let scaling = mastery.scaling(index);
    let scaling_source = match index {
        0 => "log10(ladles used)",
        1 => "account cooking level − 1000",
        2 => "Divorce Cake level − 75",
        4 => "total ribbon ranks",
        5 => "mastery rank + 1",
        _ => "",
    };
    FormulaNode::new(UPGRADE_NAMES[index], multiplier)
        .with_format(ValueFormat::Multiplier)
        .with_children([
            FormulaNode::leaf("Level", level, ValueFormat::Raw),
            FormulaNode::leaf("Per Level", mastery.per_lv(index), ValueFormat::Raw)
                .with_source("RandoListo2[8]"),
            FormulaNode::leaf("Scaling Factor", scaling, ValueFormat::Raw)
                .with_source(scaling_source),
            FormulaNode::leaf("Scaled Bonus", bonus, ValueFormat::Percent),
        ])
        .with_note(format!(
            "perLv×level×scaling = {} → {}",
            ValueFormat::Percent.format(bonus),
            ValueFormat::Multiplier.format(multiplier)
        ))
        .with_source(if mastery.unlocked(index) {
            "cooking-mastery-calc.html · bonusTotal"
        } else {
            "locked: gains nothing until the mastery rank reaches it"
        })
}

/// The account multipliers: each factor multiplies the rate.
fn external(external: &External) -> FormulaNode {
    FormulaNode::new("External Multipliers", external.total())
        .with_format(ValueFormat::Multiplier)
        .with_source("cooking-mastery-calc.html · computeExternalMulti")
        .with_children([
            factor(
                "Menagerie Boost 21 (Cooking Mastery EXP)",
                1.0 + external.menagerie / 100.0,
                Some(external.menagerie),
                "menagerieBoostQTY(21)",
            ),
            factor(
                "Masterius Cookerius (Research)",
                1.0 + external.grid_190 / 100.0,
                Some(external.grid_190),
                "gbWith(gridLevels, shapeOverlay, 190)",
            ),
            FormulaNode::new(
                "Cooking Master (Zuperbit)",
                1.0 + 40.0 * external.super_bit_68 / 100.0,
            )
            .with_format(ValueFormat::Multiplier)
            .with_source("superBitType(68)")
            .with_note(if external.super_bit_68 > 0.0 {
                "Unlocked (+40%)"
            } else {
                "Not unlocked"
            }),
            factor(
                "Rift Spooker",
                1.0 + 2.0 * external.companion_87,
                Some(external.companion_87),
                "companions(87)",
            ),
            factor(
                "Cook Maxxing (Fountain)",
                1.0 + external.fountain / 100.0,
                Some(external.fountain),
                "Fountain_BonTOT(2, 17)",
            ),
            FormulaNode::new("Additive EXP Bonuses", 1.0 + external.additive() / 100.0)
                .with_format(ValueFormat::Multiplier)
                .with_children([
                    FormulaNode::leaf(
                        format!(
                            "Canteen Read (Vial), Lv {}",
                            ValueFormat::Raw.format(external.vial_level)
                        ),
                        external.vial,
                        ValueFormat::Percent,
                    )
                    .with_source("AlchemyDescription[4] · 7cookmastery"),
                    FormulaNode::leaf(
                        "Cook Mastery EXP (Arcade)",
                        external.arcade,
                        ValueFormat::Percent,
                    )
                    .with_source("arcadeBonus(69)"),
                    FormulaNode::leaf(
                        "Salt Lick: Cooking Mastery EXP",
                        external.salt_lick,
                        ValueFormat::Percent,
                    )
                    .with_source("computeSaltLick(10)"),
                ])
                .with_note("The three terms add, then multiply the rate"),
        ])
}

/// A factor node that also names the percentage it came from.
fn factor(label: impl Into<String>, value: f64, percent: Option<f64>, source: &str) -> FormulaNode {
    let mut node = FormulaNode::new(label, value)
        .with_format(ValueFormat::Multiplier)
        .with_source(source);
    if let Some(percent) = percent {
        node = node.with_input(ValueFormat::Percent.format(percent));
    }
    node
}
