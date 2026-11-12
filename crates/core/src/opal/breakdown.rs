//! The opal state's formula tree: the rate the cards show, its villagers and
//! every account term behind them.
//!
//! Two shapes, matching the section's villager selector:
//!
//! - the total tree adds the unlocked villagers' rates, so its children sum to
//!   the root;
//! - a villager tree multiplies the factors `villagerExpRate` applies, so its
//!   children multiply to the root.
//!
//! The additive group inside each villager sums to the percentage the rate
//! applies, and the chain group multiplies to `multiChain`, so a deeper node
//! always agrees with the value the section reports.

use super::{CHAIN, Opal, RATE, VILLAGERS, name};
use crate::formula::{FormulaNode, ValueFormat};

/// The tree for the selected villager, or the total when `selected` is `None`.
pub(super) fn tree(opal: &Opal, selected: Option<usize>) -> FormulaNode {
    let opals = opal.opals();
    match selected.filter(|villager| *villager < VILLAGERS) {
        None => total(opal, &opals),
        Some(villager) => villager_node(opal, villager, &opals),
    }
}

/// `totalExpRate`: one child per unlocked villager, whose rates add up.
fn total(opal: &Opal, opals: &[f64; VILLAGERS]) -> FormulaNode {
    let rate = opal.total_exp_rate(opals);
    let mut children = Vec::new();
    for villager in (0..VILLAGERS).filter(|villager| opal.unlocked(*villager)) {
        children.push(villager_node(opal, villager, opals));
    }
    FormulaNode::new("Total EXP/hr (all villagers)", rate)
        .with_format(RATE)
        .with_input(format!(
            "the {} unlocked villagers' rates, added",
            opal.unlocked_count()
        ))
        .with_source("opal-calc.html · totalExpRate")
        .with_note("Each villager's own rate is its factors multiplied; the villagers add up.")
        .with_children(children)
}

/// `villagerExpRate`: the factors one villager's rate multiplies.
fn villager_node(opal: &Opal, villager: usize, opals: &[f64; VILLAGERS]) -> FormulaNode {
    let rate = opal.exp_rate(villager, opals);
    let count = opals[villager];
    let mut node = FormulaNode::new(format!("{} EXP/hr", name(villager)), rate)
        .with_format(RATE)
        .with_source("opal-calc.html · villagerExpRate");
    if count <= 0.0 {
        return node.with_input("no opals invested").with_note(
            "The page returns zero before it reads any factor, so this villager earns nothing.",
        );
    }
    node = node
        .with_input(format!(
            "{} opals × star ×{} × equal spread {:.4} × additive {:.4}",
            raw(count),
            raw(opal.ext.star_mul[villager]),
            opal.equal_spread_mul(opals),
            1.0 + opal.additive_pct(villager, opals) / 100.0
        ))
        .with_note("The factors multiply; the additive group inside adds up.");
    node.with_children([
        FormulaNode::leaf(
            "Holeoz EXP multiplier",
            opal.ext.holeoz[villager],
            ValueFormat::Multiplier,
        )
        .with_source("holeozDxp")
        .with_note(
            "Explorer scales with Caverns Owned; the Conjuror with the Cosmo fountain upgrade.",
        ),
        base_multiplier(&opal.ext.terms),
        FormulaNode::leaf("Opals invested", count, ValueFormat::Raw).with_source("Holes[3]"),
        FormulaNode::leaf(
            "Star multiplier",
            opal.ext.star_mul[villager],
            ValueFormat::Multiplier,
        )
        .with_source("Holes[23]")
        .with_note("Every star doubles the villager's EXP."),
        FormulaNode::leaf(
            "Equal Spread",
            opal.equal_spread_mul(opals),
            ValueFormat::Multiplier,
        )
        .with_input(format!(
            "{}% per 5 opals of the lowest villager",
            raw(opal.ext.equal_spread)
        ))
        .with_source("Cosmo 1/5")
        .with_note(format!(
            "Lowest villager: {} opals",
            raw(opal.min_opals(opals))
        )),
        additive_factor(opal, villager, opals),
    ])
}

/// `baseMulti`: the base, the multiplicative chain and the four outer factors.
fn base_multiplier(terms: &super::Terms) -> FormulaNode {
    let mut children = vec![
        FormulaNode::leaf(
            "Base + Opal Dividends",
            terms.base_plus_upg,
            ValueFormat::Raw,
        )
        .with_note("100 base; 125 once Opal Dividends is built"),
    ];
    let factors = terms.chain_factors();
    let chain: Vec<FormulaNode> = CHAIN
        .iter()
        .zip(factors)
        .map(|((label, source), factor)| {
            FormulaNode::leaf(*label, factor, ValueFormat::Multiplier).with_source(*source)
        })
        .collect();
    children.push(
        FormulaNode::new("Multiplicative chain", terms.chain())
            .with_format(ValueFormat::Multiplier)
            .with_note("The chain's factors multiply; floored at 1.")
            .with_children(chain),
    );
    for (label, factor) in terms.outer_factors() {
        children.push(FormulaNode::leaf(label, factor, ValueFormat::Multiplier));
    }
    FormulaNode::new("Base multiplier", terms.base_multiplier())
        .with_format(ValueFormat::Multiplier)
        .with_source("buildExt · baseMulti")
        .with_children(children)
}

/// `addPct`: the constant terms plus the villager's own Opal Enthusiasm.
fn additive_pct(opal: &Opal, villager: usize, opals: &[f64; VILLAGERS]) -> FormulaNode {
    let terms = &opal.ext.terms;
    let constant = FormulaNode::group(
        "Additive % (constant part)",
        ValueFormat::Percent,
        terms
            .additive_terms()
            .map(|(label, value)| FormulaNode::leaf(label, value, ValueFormat::Percent)),
    );
    let enthusiasm = FormulaNode::leaf(
        "Opal Enthusiasm",
        opal.enthusiasm(villager, opals),
        ValueFormat::Percent,
    )
    .with_input(format!(
        "{} per 10 opals",
        ValueFormat::Percent.format(opal.ext.enthusiasm)
    ))
    .with_source("Cosmo 1/0");
    FormulaNode::group("Additive %", ValueFormat::Percent, [constant, enthusiasm])
        .with_note("The terms add up; the factor above turns the sum into a multiplier.")
}

/// `1 + addPct / 100`: the factor the villager's rate multiplies.
fn additive_factor(opal: &Opal, villager: usize, opals: &[f64; VILLAGERS]) -> FormulaNode {
    FormulaNode::new(
        "Additive factor",
        1.0 + opal.additive_pct(villager, opals) / 100.0,
    )
    .with_format(ValueFormat::Multiplier)
    .with_note("(1 + sum/100) of the additive percentage below.")
    .with_children([additive_pct(opal, villager, opals)])
}

/// A number as the page prints it, without a trailing `.0`.
fn raw(value: f64) -> String {
    ValueFormat::Raw.format(value)
}
