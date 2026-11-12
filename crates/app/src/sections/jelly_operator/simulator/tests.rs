//! The simulator sandbox's board rules and one seeded attempt.

use super::{Simulator, state::fever_modes};
use idleon_jelly::Jelly;
use idleon_jelly::operation::{RevivePolicy, RoidTiming, simulate};

/// A flat save with the eight cell unlocks, the Feverizer and the roid/revive
/// upgrades, every plot bought and two Amoebas on the saved board.
fn save() -> String {
    let mut research: Vec<Vec<f64>> = vec![Vec::new(); 19];
    research[7] = vec![0.0; 15];
    research[7][9] = 2.0;
    research[7][13] = 2.0;
    research[14] = vec![-1.0; 180];
    research[14][75] = 0.0;
    research[14][77] = 0.0;
    research[15] = vec![0.0; 9];
    research[16] = vec![0.0; 9];
    research[17] = vec![0.0; 40];
    research[17][..8].fill(1.0);
    research[17][16] = 3.0;
    research[17][29] = 10.0;
    research[17][35] = 2.0;
    research[17][36] = 1.0;
    research[18] = (0..40).map(f64::from).collect();
    format!(
        "{{\"Research\":{}, \"Lv0_0\":{}}}",
        serde_json::to_string(&research).unwrap(),
        serde_json::to_string(&vec![0.0; 30]).unwrap()
    )
}

fn jelly() -> Jelly {
    idleon_import::import(&save())
        .expect("the save imports")
        .jelly
        .expect("the save carries a board")
}

#[test]
fn the_sandbox_starts_from_the_save_and_relocks_the_second_clear() {
    let jelly = jelly();
    let mut simulator = Simulator::new(&jelly);
    assert_eq!(simulator.layout, jelly.layout, "the saved board loads");
    assert_eq!(simulator.obstruction, 2);
    assert_eq!(simulator.fever, 2, "the save's Fever fits its unlocks");
    assert_eq!(fever_modes(&jelly), 3);
    assert_eq!(simulator.selected, Some(0));
    assert_eq!(simulator.roid_timing, -1.0);
    assert_eq!(simulator.revive, RevivePolicy::ImmunoidFirst);

    // Stepping back to the first stage relocks the second clear's slots.
    simulator.set_stage(&jelly, 0);
    assert_eq!(simulator.obstruction, 0);
    assert!(!simulator.layout.contains_key(&75));
    assert!(simulator.layout.contains_key(&77));
    assert_eq!(
        simulator.message,
        "Selected stage 1. Removed 1 cell that no longer fit unlocked slots."
    );
    assert!(!simulator.message_bad);
}

#[test]
fn clear_board_empties_the_sandbox_and_the_run_goes_stale() {
    let jelly = jelly();
    let mut simulator = Simulator::new(&jelly);
    let inputs = simulator.inputs(Some(&jelly));
    simulator.clear_board();
    assert!(simulator.layout.is_empty());
    assert_eq!(
        simulator.message,
        "Removed every cell from the sandbox board."
    );
    assert_ne!(
        simulator.inputs(Some(&jelly)),
        inputs,
        "clearing the board marks a stored run stale"
    );
    let sandbox = simulator.sandbox(&jelly);
    let trial = simulate(&sandbox, &simulator.layout, simulator.options());
    assert!(trial.valid, "{}", trial.reason);
    assert_eq!(trial.shots, 0.0);
    assert_eq!(trial.damage, 0.0);

    simulator.reset(&jelly);
    assert_eq!(simulator.layout, jelly.layout);
    assert_eq!(simulator.message, "Sandbox reset from the current save.");
}

#[test]
fn a_seeded_sandbox_run_is_deterministic() {
    let jelly = jelly();
    let mut simulator = Simulator::new(&jelly);
    simulator.set_roid(5.0);
    simulator.set_revive(RevivePolicy::DpsFirst);
    assert_eq!(simulator.roid_timing, 5.0);
    assert_eq!(simulator.options().roid_timing, RoidTiming::At(5.0));
    assert_eq!(simulator.options().revive_policy, RevivePolicy::DpsFirst);
    let sandbox = simulator.sandbox(&jelly);
    let first = simulate(&sandbox, &simulator.layout, simulator.options());
    let second = simulate(&sandbox, &simulator.layout, simulator.options());
    assert_eq!(first, second, "the same seed is the same attempt");

    simulator.set_roid(-1.0);
    assert_eq!(simulator.options().roid_timing, RoidTiming::Never);
    assert_eq!(simulator.message, "Stronkroid disabled.");
}

#[test]
fn remove_mode_removes_the_cell_under_the_pointer() {
    let jelly = jelly();
    let mut simulator = Simulator::new(&jelly);
    simulator.clear_board();
    simulator.select(0);
    simulator.click(&jelly, 77);
    assert_eq!(simulator.layout.get(&77), Some(&0));
    assert_eq!(simulator.message, "Placed Amoeba at anchor slot 77.");

    simulator.remove_mode();
    assert_eq!(simulator.selected, None);
    simulator.click(&jelly, 77);
    assert!(simulator.layout.is_empty());
    assert_eq!(simulator.message, "Removed the cell at slot 77.");
    assert!(!simulator.message_bad);

    simulator.click(&jelly, 77);
    assert_eq!(
        simulator.message,
        "There is no cell at that slot to remove."
    );
    assert!(simulator.message_bad);
}

#[test]
fn a_refused_placement_names_the_reason_and_keeps_the_board() {
    let jelly = jelly();
    let mut simulator = Simulator::new(&jelly);
    simulator.set_stage(&jelly, 0);
    simulator.select(0);
    let layout = simulator.layout.clone();
    simulator.click(&jelly, 100);
    assert!(
        simulator.layout == layout,
        "a refused click changes nothing"
    );
    assert!(simulator.message_bad);
    assert!(
        simulator.message.contains("locked") || simulator.message.contains("Cannot place"),
        "the refusal explains itself: {}",
        simulator.message
    );
}
