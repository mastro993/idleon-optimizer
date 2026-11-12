//! The research descriptors, from `js/stats/defs/research-exp.js` and
//! `js/stats/defs/research-afk-gains.js`.
//!
//! Both formulas already live in the simulator ([`idleon_research::sim`]); the
//! dashboard only splits them into the tree the All Stats page shows. `research-exp` is
//! the additive pool times every true multiplier, `research-afk-gains` the 1%
//! base plus its additive terms capped at 100%.

use super::{Inputs, Resolution};
use idleon_common::formula::{FormulaNode, ValueFormat};
use idleon_research::breakdown::afk;
use idleon_research::sim;

/// Resolve one research descriptor, when the id is one of the two.
pub(super) fn resolve(inputs: &Inputs<'_>, id: &str) -> Option<Resolution> {
    match id {
        "research-exp" => Some(exp(inputs)),
        "research-afk-gains" => Some(afk_gains(inputs)),
        _ => None,
    }
}

/// `research-exp`: the multiplier `simTotalExp` applies to the observation base.
fn exp(inputs: &Inputs<'_>) -> Resolution {
    let terms = sim::exp_terms(inputs.simulation, inputs.sim_context);
    let additive_total = terms.additive_total();
    let additive = FormulaNode::group(
        "Additive Bonus",
        ValueFormat::Percent,
        terms
            .additive
            .iter()
            .map(|(label, value)| FormulaNode::leaf(label.clone(), *value, ValueFormat::Percent)),
    )
    .with_input(format!(
        "The terms add to {additive_total:+.2}% before the multiplier"
    ))
    .with_multiplier(1.0 + additive_total / 100.0)
    .with_source("js/stats/defs/research-exp.js · additive pool");
    let multipliers = FormulaNode::new("True Multipliers", terms.multiplier())
        .with_format(ValueFormat::Multiplier)
        .with_source("js/stats/defs/research-exp.js · true multiplier chain")
        .with_children(terms.multipliers.iter().map(|(label, value)| {
            FormulaNode::leaf(label.clone(), *value, ValueFormat::Multiplier)
        }));
    let tree = FormulaNode::new("Research EXP Multiplier", terms.total())
        .with_format(ValueFormat::Multiplier)
        .with_input(format!(
            "(1 + {additive_total:.2}%) × {:.4}",
            terms.multiplier()
        ))
        .with_source("js/stats/defs/research-exp.js")
        .with_note(
            "The observation base multiplies this value; the Rates pane shows the same terms.",
        )
        .with_children([additive, multipliers]);
    Resolution::Value {
        tree,
        partial: None,
    }
}

/// `research-afk-gains`: 1% plus the additive terms, capped at 100%.
fn afk_gains(inputs: &Inputs<'_>) -> Resolution {
    let mut tree = afk(inputs.simulation, inputs.sim_context);
    tree.label = "Research AFK Gains Rate".to_string();
    tree.source = Some("js/stats/defs/research-afk-gains.js".to_string());
    Resolution::Value {
        tree,
        partial: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::{SimContext, Simulation};
    use idleon_save::Save;
    use serde_json::json;

    #[test]
    fn the_exp_row_matches_the_multiplier_the_rate_uses() {
        let save = Save::new(json!({ "charNames": ["Ana"] }));
        let mut simulation = Simulation {
            research_level: 100,
            grid: idleon_research::model::GridState::seeded(2),
            ..Simulation::default()
        };
        simulation.grid.set(112, 4);
        simulation.grid.set(70, 3);
        simulation.grid.set(50, 2);
        let context = SimContext {
            extra_pct: 42.0,
            companion_52: 1.5,
            dream14: 5.0,
            ..SimContext::default()
        };
        let resolution = resolve(
            &Inputs::for_test(&save, &simulation, &context),
            "research-exp",
        )
        .expect("a research row");
        let tree = resolution.tree().expect("a tree");
        let rate = idleon_research::sim::exp_rate(&simulation, &context);
        assert!((tree.value - rate.multi).abs() < 1e-9);
        assert_eq!(tree.children.len(), 2);
        // The additive group and the multiplier group reproduce the root.
        let additive = &tree.children[0];
        let multipliers = &tree.children[1];
        assert!((additive.sum_children() - additive.value).abs() < 1e-9);
        assert!(
            (multipliers_product(multipliers) - multipliers.value).abs() < 1e-9,
            "the multipliers are a product"
        );
        let expected = (1.0 + additive.value / 100.0) * multipliers.value;
        assert!((tree.value - expected).abs() < 1e-9);
        assert!(tree.match_count("Companions") >= 1);
    }

    fn multipliers_product(node: &FormulaNode) -> f64 {
        node.children.iter().map(|child| child.value).product()
    }

    #[test]
    fn the_afk_row_matches_the_rate_the_simulator_reports() {
        let save = Save::new(json!({ "charNames": ["Ana"] }));
        let simulation = Simulation::default();
        let context = SimContext {
            companion_28: 30.0,
            ..SimContext::default()
        };
        let resolution = resolve(
            &Inputs::for_test(&save, &simulation, &context),
            "research-afk-gains",
        )
        .expect("a research row");
        let tree = resolution.tree().expect("a tree");
        let rate = idleon_research::sim::afk_rate(&simulation, &context);
        assert!((tree.value - rate.rate * 100.0).abs() < 1e-9);
        assert_eq!(tree.label, "Research AFK Gains Rate");
        assert!(tree.match_count("Gambit") >= 1);
    }

    #[test]
    fn other_ids_are_not_research_rows() {
        let save = Save::new(json!({}));
        let simulation = Simulation::default();
        let context = SimContext::default();
        assert!(resolve(&Inputs::for_test(&save, &simulation, &context), "drop-rate").is_none());
    }
}
