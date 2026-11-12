//! The section's selection rules: the tab ids and what a new cavern keeps.
//!
//! The numbers themselves are pinned in `crates/core/tests/fountain.rs` against
//! the upstream JS; these cover what the section decides on top of them.

use super::{State, Tab};
use idleon_core::Fountain;
use idleon_core::save::import;
use serde_json::{Value, json};

/// A cavern with all three waters unlocked and every upgrade row present.
fn fountain() -> Fountain {
    let mut holes = vec![json!([]); 35];
    // Every water bought, and the prerequisites that unlock silver and gold.
    holes[31] = json!([[1, 0, 0, 1, 1, 0, 0, 0, 10, 0, 0, 0, 10], [1], [1],]);
    let save = json!({ "Research": "[]", "Holes": Value::Array(holes) }).to_string();
    import(&save)
        .expect("the save imports")
        .fountain
        .expect("the save carries Holes")
}

#[test]
fn the_tab_ids_match_the_strip() {
    for tab in Tab::ALL {
        assert_eq!(Tab::from_id(tab.id()), Some(tab));
    }
    assert_eq!(Tab::from_id("planner"), None);
    assert_eq!(Tab::default(), Tab::Overview);
}

#[test]
fn adopting_a_cavern_keeps_the_selections_it_can_resolve() {
    let fountain = fountain();
    let mut state = State {
        tab: Tab::Currency,
        water: 9,
        desired: Some(6),
        upgrade: Some(30),
    };
    state.adopt(Some(&fountain));
    assert_eq!(state.tab, Tab::Currency);
    assert_eq!(state.water, 2);
    // Moolah is locked in this save and row 30 does not exist.
    assert_eq!(state.desired, None);
    assert_eq!(state.upgrade, None);

    let mut state = State {
        tab: Tab::Upgrades,
        water: 1,
        desired: Some(2),
        upgrade: Some(11),
    };
    state.adopt(Some(&fountain));
    assert_eq!(state.water, 1);
    assert_eq!(state.desired, Some(2), "gold is unlocked");
    assert_eq!(state.upgrade, Some(11));

    let mut state = State {
        desired: Some(2),
        ..State::default()
    };
    state.adopt(None);
    assert_eq!(
        state.desired,
        Some(2),
        "no cavern leaves the selection alone"
    );
}
