//! The drop-rate formula tree: the arithmetic expansion behind the total card.
//!
//! A dungeon map ignores every saved term, so the descriptor's `combine` hands
//! back the expression `computeDungeonDropRate` evaluates:
//!
//! ```text
//! Dungeon Drop Rate = 1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100
//! ```
//!
//! This is the `formulaDetails` branch of `js/stats/defs/drop-rate.js`
//! (`combine`), the tree upstream's formula inspector renders: the same node
//! names, operation order and constants, one per operand. Every node also
//! carries where its value comes from, so the breakdown names the two live run
//! entries and the constants the formula adds.
//!
//! Any other map's tree is the descriptor's whole pool chain - the account stats
//! engine this build does not decode - so [`tree`] returns `None` for it and the
//! card reports the same gap the total does instead of inventing terms.

use idleon_common::formula::{FormulaNode, ValueFormat};

use super::{NEEDS_RUNTIME, Runtime, dungeon_total, is_dungeon_map};

/// The descriptor branch the tree expands: `drop-rate.js` `combine`.
const COMBINE: &str = "js/stats/defs/drop-rate.js · combine (dungeon map)";

/// `js/stats/systems/w2/dungeon.js` `computeDungeonDropRate`.
const FORMULA: &str = "js/stats/systems/w2/dungeon.js · computeDungeonDropRate";

/// The expansion of the page's total for `map`, or `None` when there is nothing
/// to expand: a map outside the dungeons needs the account stats engine, and an
/// entry outside its range is refused by the total card instead.
pub fn tree(map: usize, runtime: Runtime) -> Option<FormulaNode> {
    if !is_dungeon_map(map) || runtime.invalid().is_some() {
        return None;
    }
    match (runtime.luk, runtime.drop_rate) {
        (Some(luk), Some(drop_rate)) => Some(expansion(luk, drop_rate)),
        _ => Some(required_inputs()),
    }
}

/// `formulaOperation('Dungeon Drop Rate', 'add', constant 1, …)`.
fn expansion(luk: f64, drop_rate: f64) -> FormulaNode {
    let bonuses = luk + drop_rate;
    FormulaNode::new("Dungeon Drop Rate", dungeon_total(luk, drop_rate))
        .with_format(ValueFormat::Multiplier)
        .with_input(format!("1 + ({}) / 100", sum(luk, drop_rate)))
        .with_source(COMBINE)
        .with_note("1 + (Dungeon Total LUK + Dungeon Drop Rate stat) / 100")
        .with_children([
            FormulaNode::leaf("Constant", 1.0, ValueFormat::Raw)
                .with_source(FORMULA)
                .with_note("The 1 the dungeon formula adds to the percentage."),
            FormulaNode::new("Percent to fraction", bonuses / 100.0)
                .with_format(ValueFormat::Raw)
                .with_input(format!("({}) / 100", sum(luk, drop_rate)))
                .with_source(COMBINE)
                .with_note("The run's dungeon bonuses over 100.")
                .with_children([
                    FormulaNode::new("Dungeon bonuses", bonuses)
                        .with_format(ValueFormat::Raw)
                        .with_input(sum(luk, drop_rate))
                        .with_source(COMBINE)
                        .with_note("The two values the active run shows, added.")
                        .with_children([
                            FormulaNode::leaf("Dungeon Total LUK", luk, ValueFormat::Raw)
                                .with_source(
                                    "drop-rate-calc.html #dungeon-luk · \
                                     Live dungeon Total LUK input",
                                ),
                            FormulaNode::leaf(
                                "Dungeon Drop Rate stat",
                                drop_rate,
                                ValueFormat::Raw,
                            )
                            .with_source(
                                "drop-rate-calc.html #dungeon-dr · \
                                     Live dungeon Drop Rate input",
                            ),
                        ]),
                    FormulaNode::leaf("Constant", 100.0, ValueFormat::Raw)
                        .with_source(FORMULA)
                        .with_note("The run's percentage, over 100."),
                ]),
        ])
}

/// `combine`'s `unavailable` branch: the run values the save cannot carry.
fn required_inputs() -> FormulaNode {
    FormulaNode::new("Drop Rate", 0.0)
        .with_format(ValueFormat::Multiplier)
        .with_source(COMBINE)
        .with_note(NEEDS_RUNTIME)
        .with_children([FormulaNode::new("Dungeon Runtime Inputs Required", 0.0)
            .with_format(ValueFormat::Raw)
            .with_source(COMBINE)
            .with_note("Enter the two values shown in the active dungeon run.")])
}

/// Two live values as the expression reads them.
fn sum(luk: f64, drop_rate: f64) -> String {
    format!(
        "{} + {}",
        ValueFormat::Raw.format(luk),
        ValueFormat::Raw.format(drop_rate)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DUNGEON_MAPS, Total, total};

    /// Every node of the tree, depth first, with the path it sits at.
    fn walk<'a>(node: &'a FormulaNode, path: &str, out: &mut Vec<(String, &'a FormulaNode)>) {
        out.push((path.to_string(), node));
        for (index, child) in node.children.iter().enumerate() {
            walk(child, &format!("{path}/{index}"), out);
        }
    }

    fn nodes(root: &FormulaNode) -> Vec<(String, &FormulaNode)> {
        let mut out = Vec::new();
        walk(root, "root", &mut out);
        out
    }

    fn run(luk: f64, drop_rate: f64) -> Runtime {
        Runtime {
            luk: Some(luk),
            drop_rate: Some(drop_rate),
        }
    }

    #[test]
    fn the_tree_sums_to_the_total() {
        for (map, luk, drop_rate) in [
            (39, 0.0, 0.0),
            (39, 1200.0, 35.0),
            (40, 25_000.0, 750.5),
            (70, 3.5, 0.25),
            (71, 999_999.0, 12_345.0),
            (118, 12_345.5, 678.25),
            (119, 1.0, 99.0),
        ] {
            let root = tree(map, run(luk, drop_rate)).expect("a dungeon tree");
            let total = total(map, run(luk, drop_rate)).value().expect("a total");
            assert_eq!(root.value, total, "map {map} at {luk}/{drop_rate}");
            assert!((root.sum_children() - total).abs() < 1e-9, "{root:?}");
            // The one additive group below the root adds up to its own value.
            let bonuses = nodes(&root)
                .into_iter()
                .find(|(_, node)| node.label == "Dungeon bonuses")
                .expect("the bonuses group")
                .1;
            assert_eq!(bonuses.value, luk + drop_rate);
            assert_eq!(bonuses.sum_children(), bonuses.value);
        }
    }

    #[test]
    fn every_term_names_its_source() {
        let root = tree(39, run(1200.0, 35.0)).expect("a dungeon tree");
        for (path, node) in nodes(&root) {
            let source = node.source.as_deref().unwrap_or_default();
            assert!(
                !source.is_empty(),
                "{path} ({}) names its source",
                node.label
            );
        }
        let luk = nodes(&root)
            .into_iter()
            .find(|(_, node)| node.label == "Dungeon Total LUK")
            .expect("the LUK term")
            .1;
        assert!(
            luk.source
                .as_deref()
                .unwrap_or_default()
                .contains("Live dungeon Total LUK input"),
            "{:?}",
            luk.source
        );
    }

    #[test]
    fn search_finds_the_live_inputs_and_the_arithmetic() {
        let root = tree(39, run(1200.0, 35.0)).expect("a dungeon tree");
        // Each label, input, source and note is searchable in the shared tree.
        assert_eq!(root.match_count("luk"), 2, "the LUK term and the formula");
        assert_eq!(root.match_count("live dungeon"), 2, "both live run entries");
        assert_eq!(
            root.match_count("dungeon bonuses"),
            2,
            "the group and its note"
        );
        assert_eq!(
            root.match_count("1200"),
            4,
            "the value and the inputs it feeds"
        );
        assert_eq!(
            root.match_count("computeDungeonDropRate"),
            2,
            "the constants"
        );
        assert_eq!(root.match_count("nothing of the sort"), 0);
    }

    #[test]
    fn a_blank_live_value_asks_for_the_run_values() {
        for runtime in [
            Runtime::default(),
            Runtime {
                luk: Some(500.0),
                drop_rate: None,
            },
            Runtime {
                luk: None,
                drop_rate: Some(50.0),
            },
        ] {
            for map in DUNGEON_MAPS {
                let root = tree(map, runtime).expect("the upstream unavailable branch");
                assert_eq!(root.label, "Drop Rate");
                assert_eq!(root.value, 0.0);
                assert_eq!(root.source.as_deref(), Some(COMBINE));
                assert_eq!(root.note.as_deref(), Some(NEEDS_RUNTIME));
                let child = root.children.first().expect("the required-inputs node");
                assert_eq!(child.label, "Dungeon Runtime Inputs Required");
                assert_eq!(child.value, 0.0);
                assert_eq!(
                    child.note.as_deref(),
                    Some("Enter the two values shown in the active dungeon run.")
                );
            }
        }
    }

    #[test]
    fn there_is_no_tree_outside_the_dungeons_or_the_range() {
        for map in [0, 1, 38, 41, 216] {
            assert_eq!(tree(map, run(1200.0, 35.0)), None, "map {map}");
        }
        for runtime in [
            Runtime {
                luk: Some(-1.0),
                drop_rate: Some(10.0),
            },
            Runtime {
                luk: Some(1.0),
                drop_rate: Some(f64::INFINITY),
            },
        ] {
            assert_eq!(tree(39, runtime), None, "{runtime:?}");
            assert!(matches!(total(39, runtime), Total::Invalid(_)));
        }
    }
}
