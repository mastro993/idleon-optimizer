//! The dungeon totals and their formula tree are pinned against the upstream
//! calculator's own output.

use idleon_common::formula::FormulaNode;
use idleon_drop_rate::{self as drop_rate, Runtime, Total};
use serde_json::Value;

fn read(path: &str) -> String {
    std::fs::read_to_string(format!("{}/../../{}", env!("CARGO_MANIFEST_DIR"), path))
        .unwrap_or_else(|error| panic!("{path} is committed: {error}"))
}

fn oracle() -> Value {
    serde_json::from_str(&read("fixtures/drop-rate.json")).expect("oracle JSON")
}

/// The port's tree against upstream's own `formulaDetails` branch: the same
/// names, the same values in the same order, and a source on every node. Where
/// upstream names a source, the port's node names it too.
fn assert_matches_upstream(ours: &FormulaNode, upstream: &Value, path: &str) {
    let name = upstream["name"].as_str().expect("upstream node name");
    assert_eq!(ours.label, name, "label at {path}");
    let expected = upstream["val"].as_f64().expect("upstream node value");
    assert!(
        (ours.value - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{path} ({name}): {} != {expected}",
        ours.value
    );
    let source = ours.source.as_deref().unwrap_or_default();
    assert!(!source.is_empty(), "{path} ({name}) names its source");
    if let Some(upstream_source) = upstream["source"].as_str() {
        assert!(
            source.contains(upstream_source),
            "{path} ({name}) cites {source}, upstream says {upstream_source}"
        );
    }
    if let Some(note) = upstream["note"].as_str() {
        assert_eq!(ours.note.as_deref(), Some(note), "note at {path}");
    }
    let children = upstream["children"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    assert_eq!(ours.children.len(), children.len(), "children at {path}");
    for (index, (child, upstream_child)) in ours.children.iter().zip(children).enumerate() {
        assert_matches_upstream(child, upstream_child, &format!("{path}/{index}"));
    }
}

#[test]
fn every_dungeon_total_matches_the_upstream_calculator() {
    let oracle = oracle();
    let cases = oracle["cases"].as_array().expect("oracle cases");
    assert!(!cases.is_empty(), "the oracle carries dungeon cases");
    for case in cases {
        let map = case["map"].as_u64().expect("map") as usize;
        let luk = case["luk"].as_f64().expect("luk");
        let drop_rate = case["dropRate"].as_f64().expect("drop rate");
        let expected = case["total"].as_f64().expect("total");
        let outcome = drop_rate::total(
            map,
            Runtime {
                luk: Some(luk),
                drop_rate: Some(drop_rate),
            },
        );
        assert_eq!(
            outcome,
            Total::Value(expected),
            "map {map} at {luk}/{drop_rate}"
        );
        assert!(
            outcome.reason().is_none(),
            "a computed total shows no reason"
        );
    }
}

#[test]
fn every_dungeon_tree_matches_upstreams_formula_expansion() {
    let oracle = oracle();
    let cases = oracle["cases"].as_array().expect("oracle cases");
    assert!(!cases.is_empty(), "the oracle carries dungeon cases");
    for case in cases {
        let map = case["map"].as_u64().expect("map") as usize;
        let runtime = Runtime {
            luk: Some(case["luk"].as_f64().expect("luk")),
            drop_rate: Some(case["dropRate"].as_f64().expect("drop rate")),
        };
        let root = drop_rate::tree(map, runtime).expect("a dungeon tree");
        // Tree-sum: the root is the total card's number and its children add up.
        assert_eq!(
            Some(root.value),
            drop_rate::total(map, runtime).value(),
            "map {map}"
        );
        assert!((root.sum_children() - root.value).abs() < 1e-9, "map {map}");
        assert_matches_upstream(&root, &case["tree"], &format!("map {map}"));
    }
}

#[test]
fn the_blank_tree_is_upstreams_unavailable_branch() {
    let oracle = oracle();
    let missing = &oracle["missingInputs"];
    let map = missing["map"].as_u64().expect("map") as usize;
    let root = drop_rate::tree(map, Runtime::default()).expect("the unavailable branch");
    assert_eq!(root.note.as_deref(), Some(drop_rate::NEEDS_RUNTIME));
    assert_matches_upstream(&root, &missing["tree"], "missing inputs");
}

#[test]
fn a_non_dungeon_map_has_no_tree_to_expand() {
    let oracle = oracle();
    let map = oracle["nonDungeon"]["map"].as_u64().expect("map") as usize;
    assert_eq!(drop_rate::tree(map, Runtime::default()), None);
    assert_eq!(
        drop_rate::tree(
            map,
            Runtime {
                luk: Some(1200.0),
                drop_rate: Some(35.0),
            }
        ),
        None,
        "the live run values do not make a non-dungeon total computable"
    );
}

#[test]
fn the_missing_inputs_reason_is_the_upstream_one() {
    let oracle = oracle();
    let map = oracle["missingInputs"]["map"].as_u64().expect("map") as usize;
    let expected = oracle["missingInputs"]["reason"]
        .as_str()
        .expect("upstream reason");
    let outcome = drop_rate::total(map, Runtime::default());
    assert_eq!(outcome, Total::NeedsRuntime);
    assert_eq!(outcome.reason(), Some(expected));
    assert_eq!(expected, drop_rate::NEEDS_RUNTIME);
}

#[test]
fn a_non_dungeon_map_reports_the_missing_account_stats_engine() {
    let oracle = oracle();
    let map = oracle["nonDungeon"]["map"].as_u64().expect("map") as usize;
    assert!(
        oracle["nonDungeon"]["total"].as_f64().expect("reference") > 1.0,
        "the oracle records the total this build cannot compute yet"
    );
    let outcome = drop_rate::total(
        map,
        Runtime {
            luk: Some(1200.0),
            drop_rate: Some(35.0),
        },
    );
    assert_eq!(outcome, Total::Unsupported);
    assert!(
        outcome
            .reason()
            .is_some_and(|reason| reason.contains("shared stat pipeline")),
        "{:?}",
        outcome.reason()
    );
}
