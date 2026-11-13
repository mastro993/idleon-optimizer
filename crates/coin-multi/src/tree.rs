//! The coin multiplier formula tree: the 23 groups behind the total card.
//!
//! `combine` in `js/stats/defs/coin-multi.js` multiplies 23 `1 + bonus / 100`
//! groups and returns them as the tree's children, so a breakdown multiplies
//! back to the number the total card shows instead of adding up to it. A group
//! whose own formula sums several sources expands into those terms, in
//! upstream's units; a group this build cannot resolve is neutral (`×1`) and
//! carries the source it still needs, so the partial total stays explained.

use idleon_common::formula::{FormulaNode, ValueFormat};

use crate::{Evaluation, Group, Term};

/// The descriptor whose `combine` builds the tree.
const COMBINE: &str = "js/stats/defs/coin-multi.js · combine";

/// The expansion of the total card's number, group by group.
pub fn tree(evaluation: &Evaluation) -> FormulaNode {
    FormulaNode::new("Coin Multiplier", evaluation.total)
        .with_format(ValueFormat::Multiplier)
        .with_source(COMBINE)
        .with_note(
            "The 23 groups the descriptor multiplies, each 1 + bonus / 100; a group this build \
             cannot resolve is neutral, so the total is a lower bound.",
        )
        .with_children(evaluation.groups.iter().map(group_node))
}

/// One group of the product; `×1` with its reason when the save cannot resolve it.
fn group_node(group: &Group) -> FormulaNode {
    let mut node = FormulaNode::new(group.name, group.value.unwrap_or(1.0))
        .with_format(ValueFormat::Multiplier)
        .with_source(group.source)
        .with_children(group.terms.iter().map(term_node));
    if let Some(reason) = (!group.is_modelled()).then_some(group.reason) {
        return node.with_note(reason);
    }
    if group.terms.is_empty() {
        return node.with_note(group.note.clone());
    }
    let sum: f64 = group.terms.iter().map(|term| term.value).sum();
    node = node.with_input(recombined(sum, group.divisor));
    node.with_note(group.note.clone())
}

/// The group's own `1 + Σ terms / divisor` as the row's input line.
fn recombined(sum: f64, divisor: f64) -> String {
    let sum = ValueFormat::Raw.format(sum);
    if divisor == 100.0 {
        format!("1 + {sum} / 100")
    } else {
        format!("1 + {sum}")
    }
}

/// One additive term of a group.
fn term_node(term: &Term) -> FormulaNode {
    let node = FormulaNode::leaf(term.name, term.value, ValueFormat::Raw);
    match &term.note {
        Some(note) => node.with_note(note.clone()),
        None => node,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Overkill, Term, group};

    fn evaluation(groups: Vec<Group>) -> Evaluation {
        let total = groups
            .iter()
            .filter_map(|group| group.value)
            .product::<f64>();
        let excluded: Vec<&'static str> = groups
            .iter()
            .filter(|group| !group.is_modelled())
            .map(|group| group.name)
            .collect();
        Evaluation {
            total,
            complete: excluded.is_empty(),
            groups,
            excluded,
            overkill: Overkill::default(),
            warnings: Vec::new(),
        }
    }

    #[test]
    fn the_root_multiplies_back_to_the_total() {
        let groups = (0..23)
            .map(|index| group(index, Some(1.5)))
            .collect::<Vec<_>>();
        let evaluation = evaluation(groups);
        let tree = tree(&evaluation);
        assert_eq!(tree.label, "Coin Multiplier");
        assert_eq!(tree.format, ValueFormat::Multiplier);
        assert_eq!(tree.children.len(), 23);
        let product: f64 = tree.children.iter().map(|child| child.value).product();
        assert!((product - evaluation.total).abs() <= 1e-9 * evaluation.total);
        assert!(tree.source.is_some(), "the root names its source");
        assert!(tree.children.iter().all(|child| child.source.is_some()));
    }

    #[test]
    fn a_missing_group_is_neutral_and_names_the_source_it_needs() {
        let groups = (0..23).map(|index| group(index, None)).collect::<Vec<_>>();
        let evaluation = evaluation(groups);
        let tree = tree(&evaluation);
        assert_eq!(tree.value, 1.0);
        assert!(tree.children.iter().all(|child| child.value == 1.0));
        let cash = &tree.children[0];
        assert_eq!(cash.label, "Cash Bubbles from Main Stats");
        assert!(cash.note.as_deref().unwrap().contains("stat engine"));
    }

    #[test]
    fn an_expanded_group_recombines_its_terms() {
        let mut groups = (0..23)
            .map(|index| group(index, Some(1.0)))
            .collect::<Vec<_>>();
        // Group 7 is `1 + etcBonus('77') / 100`; group 17 is `1 + Σ terms`.
        groups[7] = group(7, Some(1.5)).with_terms(
            vec![Term::new("Combined Item Sources: BONUS MONEY", 50.0)],
            100.0,
        );
        groups[17] =
            group(17, Some(1.5)).with_terms(vec![Term::new("Pet Arena: Cash I", 0.5)], 1.0);
        let tree = tree(&evaluation(groups));

        let percent = &tree.children[7];
        assert_eq!(percent.input.as_deref(), Some("1 + 50 / 100"));
        assert_eq!(percent.children[0].value, 50.0);
        assert_eq!(percent.children[0].format, ValueFormat::Raw);

        let fraction = &tree.children[17];
        assert_eq!(fraction.input.as_deref(), Some("1 + 0.5"));
        assert_eq!(fraction.value, 1.5);
    }

    #[test]
    fn search_reaches_the_groups_and_their_terms() {
        let mut groups = (0..23)
            .map(|index| group(index, Some(1.0)))
            .collect::<Vec<_>>();
        groups[18] = group(18, Some(1.0))
            .with_terms(vec![Term::new("Vault: Recipe for Profit", 10.0)], 100.0);
        let tree = tree(&evaluation(groups));
        assert_eq!(tree.match_count("mainframe and vault"), 1);
        assert_eq!(tree.match_count("recipe for profit"), 1);
        // Every node that names a source is searchable by it.
        assert_eq!(tree.match_count("js/stats/"), 24);
        assert_eq!(tree.match_count("nope"), 0);
    }
}
