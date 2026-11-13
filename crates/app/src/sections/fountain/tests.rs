//! The section's selection rules: the tab ids and what a new cavern keeps.
//!
//! The numbers themselves are pinned in `crates/fountain/tests/fountain.rs` against
//! the upstream JS; these cover what the section decides on top of them.

use super::{SIM_DEFAULT_HOURS, SIM_MAX_HOURS, SIM_MIN_HOURS, STEP_CHOICES, State, Tab};
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
    assert_eq!(Tab::from_id("marble"), Some(Tab::Marble));
    assert_eq!(Tab::from_id("sim"), Some(Tab::Sim));
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
        marble_desired: Some(6),
        marble_optimizer_steps: 3,
        sim_hours: 0.0,
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
    assert_eq!(state.marble_desired, None);
    assert_eq!(
        state.marble_optimizer_steps, 50,
        "an unknown cap falls back"
    );
    assert_eq!(
        state.sim_hours, SIM_MIN_HOURS,
        "a time limit below the minimum is lifted"
    );

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
    assert_eq!(state.marble_optimizer_steps, 50);
}

#[test]
fn the_marble_planner_and_simulation_requests_carry_their_controls() {
    let mut state = State {
        marble_goal: Goal::Fill,
        marble_desired: Some(2),
        marble_affordable: true,
        marble_optimizer_goal: Goal::Royal,
        marble_optimizer_steps: 25,
        marble_optimizer_affordable: false,
        sim_hours: 48.0,
        ..State::default()
    };
    let planner = state.marble_planner_request();
    assert_eq!(planner.goal, Goal::Fill);
    assert_eq!(planner.desired, Some(2));
    assert_eq!(planner.max_steps, 1, "the planner ranks one level per row");
    assert!(planner.affordable);

    let plan = state.marble_plan_request();
    assert_eq!(plan.goal, Goal::Royal);
    assert_eq!(plan.desired, None, "the optimizer reads the save's desired");
    assert_eq!(plan.max_steps, 25);
    assert!(!plan.affordable);

    let sim = state.sim_request();
    assert_eq!(sim.hours, 48.0);
    assert_eq!(sim.desired, None);

    let defaults = State::default();
    assert!(!defaults.marble_affordable, "the page starts unfiltered");
    assert!(
        defaults.marble_optimizer_affordable,
        "the optimizer starts affordable"
    );
    assert_eq!(defaults.sim_hours, SIM_DEFAULT_HOURS);

    state.sim_hours = 1e9;
    state.adopt(Some(&fountain()));
    assert_eq!(state.sim_hours, SIM_MAX_HOURS, "a huge limit is capped");
}
