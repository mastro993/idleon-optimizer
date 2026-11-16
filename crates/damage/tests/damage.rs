//! The Damage Breakdown summary against the committed synthetic account: every
//! character on its current map.

use idleon_damage::{self as damage, LayerId};
use idleon_import::{Imported, import};
use idleon_stats::{self as stats, Snapshot};

fn imported() -> Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    import(&text).expect("the fixture imports")
}

/// Every character and the map it stands on.
fn contexts(imported: &Imported) -> Vec<(usize, usize)> {
    let snapshot = Snapshot::from_save(&imported.save);
    (0..snapshot.characters())
        .map(|character| (character, snapshot.current_map(character).unwrap_or(0)))
        .collect()
}

fn close(left: f64, right: f64) -> bool {
    (left - right).abs() <= 1e-9 * left.abs().max(right.abs()).max(1.0)
}

/// The All Stats `damage` row and this page read the same engine, so the row's
/// value is the page's maximum and its partial note names the same metadata.
#[test]
fn the_stats_registry_row_is_the_summary_maximum() {
    let imported = imported();
    for (character, map) in contexts(&imported) {
        let inputs = stats::BreakdownInputs {
            save: &imported.save,
            simulation: &imported.simulation,
            sim_context: &imported.context,
            character,
            map,
        };
        let sheet = stats::Sheet::build(&inputs);
        let row = sheet
            .get("damage")
            .expect("the stats registry lists damage");
        let summary = damage::evaluate(
            &imported.save,
            &imported.simulation,
            damage::Context { character, map },
        );
        assert_eq!(
            row.resolution.value(),
            Some(summary.max),
            "character {character}"
        );
        assert_eq!(
            row.warning(),
            Some(summary.partial_reason().as_str()),
            "character {character}"
        );
    }
}

/// DMG-02's acceptance, on the engine's own layers: every layer is present, the
/// three values recombine into the summary the cards show, and the mastery tree
/// is the ratio the bar reads.
#[test]
fn every_engine_layer_is_present_and_recombines_into_the_summary() {
    let imported = imported();
    for (character, map) in contexts(&imported) {
        let summary = damage::evaluate(
            &imported.save,
            &imported.simulation,
            damage::Context { character, map },
        );
        let layers = summary
            .layers()
            .unwrap_or_else(|| panic!("character {character}: no layers"));

        for id in LayerId::ALL {
            let layer = layers.get(id);
            assert!(
                layer.value.is_finite() && layer.value > 0.0,
                "character {character} {}: {}",
                id.id(),
                layer.value
            );
        }

        // Layer sum matches the summary: the three values are the maximum, and
        // the minimum is that maximum times the ratio.
        assert!(
            close(layers.max(), summary.max),
            "character {character}: {} != {}",
            layers.max(),
            summary.max
        );
        assert!(
            close(summary.min, summary.max * summary.mastery),
            "character {character}: {} != {} × {}",
            summary.min,
            summary.max,
            summary.mastery
        );

        // The mastery tree's root is the ratio the bar reads.
        let tree = summary
            .mastery_tree()
            .unwrap_or_else(|| panic!("character {character}: no mastery tree"));
        assert!(close(tree.value, summary.mastery));
    }
}

/// DMG-03: one expandable tree per layer. Every node carries the source the
/// search looks at, the roots are the layer values the summary multiplies, and
/// the term text the page searches ("Weapon Power", "softcap") finds nodes.
#[test]
fn every_layer_has_a_formula_tree_rooted_at_its_value() {
    use idleon_common::formula::FormulaNode;

    fn every_node_has_a_source(node: &FormulaNode) -> bool {
        node.source
            .as_deref()
            .is_some_and(|source| !source.is_empty())
            && node.children.iter().all(every_node_has_a_source)
    }

    fn has_multiplier(node: &FormulaNode) -> bool {
        node.multiplier.is_some() || node.children.iter().any(has_multiplier)
    }

    fn has_input(node: &FormulaNode) -> bool {
        node.input.is_some() || node.children.iter().any(has_input)
    }

    let imported = imported();
    for (character, map) in contexts(&imported) {
        let summary = damage::evaluate(
            &imported.save,
            &imported.simulation,
            damage::Context { character, map },
        );
        let layers = summary.layers().expect("the layers resolve");
        for id in LayerId::ALL {
            let tree = summary
                .layer_tree(id)
                .unwrap_or_else(|| panic!("character {character} {}: no tree", id.id()));
            assert_eq!(
                tree.value,
                layers.get(id).value,
                "character {character} {}",
                id.id()
            );
            assert!(tree.node_count() > 8, "character {character} {}", id.id());
            assert!(
                every_node_has_a_source(&tree),
                "character {character} {}: a node lost its source",
                id.id()
            );
            assert!(has_input(&tree) || has_multiplier(&tree));
        }
        let base = summary.layer_tree(LayerId::Base).expect("base tree");
        assert!(base.match_count("weapon power") > 0);
        let multiplier = summary
            .layer_tree(LayerId::Multiplier)
            .expect("multiplier tree");
        assert!(multiplier.match_count("softcap") > 0);
    }
}
