//! The overview's selection and plan wording.
//!
//! The numbers themselves are pinned in `crates/royal-guardian/tests/royal_guardian.rs`
//! against the upstream JS; these cover what the section decides on top of them:
//! which outpost the range panel explains and how it words the plan.

use super::overview::{plan_text, selected_outpost};
use idleon_royal_guardian as royal_guardian;
use idleon_royal_guardian::RangeTarget;
use idleon_royal_guardian::RoyalGuardian;
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
    // Map 1 built with three guards, resource 7 in its first link slot and one
    // militia on map 2.
    maps[1] = json!([0, 0, 0, 0, 0, 0, 0, 0, 7, -1, 0, 444_111_111, 0]);
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

// ----- RG-02: the map, the node inspector and the currency picker -----

use super::currencies::selected_currency;
use super::resources::selected_resource;

#[test]
fn the_node_inspector_falls_back_to_a_drained_node_in_the_world() {
    let royal = royal();
    // The save's only built outpost links the first W1 node it reaches.
    let streams = royal.resource_streams().0;
    assert!(!streams.is_empty(), "the fixture links a node");
    let linked = streams[0].resource;
    assert_eq!(RoyalGuardian::resource_world(linked), 0, "a W1 node");
    assert_eq!(selected_resource(&royal, 0, None), linked);
    // A saved node in another world falls back to this world's stream.
    let other = royal_guardian::resource_count() - 1;
    assert_eq!(selected_resource(&royal, 0, Some(other)), linked);
    // A saved node in the world wins over the stream.
    let first = royal.world_resources(0)[0].resource;
    assert_eq!(selected_resource(&royal, 0, Some(first)), first);
    // A world with nothing draining still lists its first node.
    let world = royal_guardian::resource_count() / 20 - 1;
    let first = royal.world_resources(world)[0].resource;
    assert_eq!(selected_resource(&royal, world, None), first);
}

#[test]
fn the_currency_picker_keeps_a_paid_slot_and_falls_back_to_the_first() {
    let royal = royal();
    let slots = royal_guardian::currency_slots();
    assert!(!slots.is_empty());
    assert_eq!(selected_currency(&slots, None), slots[0]);
    assert_eq!(selected_currency(&slots, Some(slots[1])), slots[1]);
    // A slot no node pays is not selectable.
    assert_eq!(selected_currency(&slots, Some(4_000)), slots[0]);
    assert_eq!(
        royal.currency_income(slots[0]).slot,
        slots[0],
        "the income reads the slot it is asked for"
    );
}
