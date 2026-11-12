//! The shared formula tree must report exactly what the section's totals show.

use idleon_common::ValueFormat;
use idleon_import::import;
use idleon_research::breakdown;
use idleon_research::sim;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

#[test]
fn the_fixture_breakdown_matches_the_rates_kpis() {
    let imported = import(&fixture()).expect("the fixture imports");
    let rate = sim::exp_rate(&imported.simulation, &imported.context);
    let tree = breakdown::exp(&imported.simulation, &imported.context);

    assert_eq!(tree.label, "Research EXP/hr");
    assert_eq!(tree.format, ValueFormat::Rate);
    assert!((tree.value - rate.total).abs() < 1e-6);
    assert_eq!(tree.multiplier, Some(rate.multi));

    let observation = &tree.children[0];
    assert!((observation.value - rate.obs_base).abs() < 1e-6);
    assert!((observation.sum_children() - rate.obs_base).abs() < 1e-6);

    let additive = &tree.children[1];
    assert!((additive.sum_children() - additive.value).abs() < 1e-6);
}

#[test]
fn every_additive_term_is_named_and_searchable() {
    let imported = import(&fixture()).expect("the fixture imports");
    let tree = breakdown::exp(&imported.simulation, &imported.context);
    let additive = &tree.children[1];

    for term in &additive.children {
        assert!(!term.label.is_empty());
        assert!(
            term.source.is_some(),
            "{} needs a provenance line",
            term.label
        );
        assert!(
            tree.match_count(&term.label) >= 1,
            "{} should be searchable",
            term.label
        );
    }
    assert!(tree.match_count("grid") > 0);
    assert!(tree.match_count("observation") > 0);
}

#[test]
fn copying_the_tree_keeps_one_line_per_node() {
    let imported = import(&fixture()).expect("the fixture imports");
    let tree = breakdown::exp(&imported.simulation, &imported.context);
    let text = tree.copy_text();
    assert_eq!(text.lines().count(), tree.node_count());
    assert!(text.starts_with("Research EXP/hr: "));
    assert!(text.contains("Observation base: "));
    assert!(text.contains("Takin' Notes: "));
}
