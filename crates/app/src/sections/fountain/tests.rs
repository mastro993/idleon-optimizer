//! The section's selection rules: the tab ids and what a new cavern keeps.
//!
//! The numbers themselves are pinned in `crates/fountain/tests/fountain.rs` against
//! the upstream JS; these cover what the section decides on top of them.

use super::{STEP_CHOICES, State, Tab};
use idleon_fountain::{Fountain, Goal};
use idleon_import::import;
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
    assert_eq!(Tab::from_id("planner"), Some(Tab::Planner));
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
        planner_desired: Some(6),
        optimizer_steps: 3,
        ..State::default()
    };
    state.adopt(Some(&fountain));
    assert_eq!(state.tab, Tab::Currency);
    assert_eq!(state.water, 2);
    // Moolah is locked in this save and row 30 does not exist.
    assert_eq!(state.desired, None);
    assert_eq!(state.upgrade, None);
    assert_eq!(state.planner_desired, None);
    assert_eq!(state.optimizer_steps, 50, "an unknown cap falls back");

    let mut state = State {
        tab: Tab::Upgrades,
        water: 1,
        desired: Some(2),
        upgrade: Some(11),
        ..State::default()
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

#[test]
fn the_planner_and_optimizer_requests_carry_their_own_controls() {
    let mut state = State {
        planner_goal: Goal::Fill,
        planner_desired: Some(2),
        planner_affordable: true,
        planner_normalize: true,
        planner_per_hr: true,
        optimizer_goal: Goal::Optimal,
        optimizer_steps: 25,
        optimizer_affordable: false,
        optimizer_normalize: false,
        optimizer_per_hr: true,
        ..State::default()
    };
    let planner = state.planner_request();
    assert_eq!(planner.goal, Goal::Fill);
    assert_eq!(planner.desired, Some(2));
    assert!(planner.affordable);
    assert!(planner.normalize);
    assert!(planner.per_hr);

    let plan = state.plan_request();
    assert_eq!(plan.goal, Goal::Optimal);
    assert_eq!(plan.desired, None, "the optimizer reads the save's desired");
    assert_eq!(plan.max_steps, 25);
    assert!(!plan.affordable);
    assert!(plan.per_hr);

    let defaults = State::default();
    assert!(defaults.optimizer_affordable, "the page starts affordable");
    assert_eq!(defaults.optimizer_steps, 50);
    assert!(STEP_CHOICES.contains(&defaults.optimizer_steps));

    state.adopt(None);
    assert_eq!(state.optimizer_steps, 25, "no cavern keeps a valid cap");
}
