//! The overview's selection and plan wording.
//!
//! The numbers themselves are pinned in `crates/core/tests/royal_guardian.rs`
//! against the upstream JS; these cover what the section decides on top of them:
//! which outpost the range panel explains and how it words the plan.

use super::overview::{plan_text, selected_outpost};
use idleon_core::RoyalGuardian;
use idleon_core::royal_guardian::RangeTarget;
use serde_json::{Map, Value, json};

/// A save with one built W1 outpost and one map still clearing.
fn save() -> Map<String, Value> {
    let mut armory = vec![json!(0.0); 83];
    armory[21] = json!(20.0);
    let mut g = vec![json!([]); 24];
    g[2] = json!(armory);
    g[6] = json!([4.0, 0.0]);
    g[7] = json!([2.0, 0.0]);
    let mut maps = vec![json!([]); 4];
    // Map 1 built with three guards and one militia on map 2.
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, -1, -1, 0, 444_111_111, 0]);
    maps[2] = json!([100.0]);
    let mut save = Map::new();
    save.insert("RoyalG".into(), Value::String(json!(g).to_string()));
    save.insert("RoyalMaps".into(), Value::String(json!(maps).to_string()));
    save
}

fn royal() -> RoyalGuardian {
    RoyalGuardian::from_save(&save(), None).expect("the rows read")
}

#[test]
fn the_panel_explains_the_saved_outpost_or_the_worlds_first() {
    let royal = royal();
    let built = royal
        .world_outposts(0)
        .into_iter()
        .filter(|outpost| outpost.built)
        .map(|outpost| outpost.map)
        .collect::<Vec<_>>();
    assert_eq!(built, vec![1], "one built outpost in W1");
    assert_eq!(selected_outpost(&royal, 0, None), 1);
    assert_eq!(selected_outpost(&royal, 0, Some(1)), 1);
    // An outpost that is not built in this world falls back to this world's first.
    assert_eq!(selected_outpost(&royal, 0, Some(2)), 1);
    // A world with nothing built has no selection.
    assert_eq!(selected_outpost(&royal, 4, None), 0);
}

#[test]
fn the_plan_reads_as_the_upstream_panel_words_it() {
    let royal = royal();
    let target = royal
        .next_range_target(1)
        .expect("a W1 outpost has a node past its reach");
    let plan = plan_text(Some(&target));
    assert!(plan.starts_with("+"), "{plan}");
    assert!(plan.contains("level"), "{plan}");
    // A node the outpost reaches plans nothing.
    let nearby = RangeTarget {
        resource: 0,
        distance: 10.0,
        range: 200.0,
        reach: 215.0,
        plan: royal.logistics_plan(1, 10.0),
    };
    assert_eq!(plan_text(Some(&nearby)), "Already in range");
    // A node past the maximum Advanced Logistics reach names why.
    let far = RangeTarget {
        resource: 0,
        distance: 5_000.0,
        range: 200.0,
        reach: 215.0,
        plan: royal.logistics_plan(1, 5_000.0),
    };
    assert_eq!(
        plan_text(Some(&far)),
        "beyond maximum Advanced Logistics range"
    );
    assert_eq!(plan_text(None), "—");
}
