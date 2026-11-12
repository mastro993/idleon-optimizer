//! Monument formula edges: run-state effects, reward charge branches, partial
//! saves and rejected inputs.
//!
//! The saved cards, the odds for every run state and the reward charges are
//! compared against the upstream calculator in `crates/core/tests/monument.rs`.

use super::*;
use crate::save;
use serde_json::{Value, json};

fn fixture() -> Save {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("the fixture is committed");
    let imported = save::import(&text).expect("the fixture imports");
    Save::from_parts(&imported.save, imported.companion.as_ref())
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn the_boosts_and_rerolls_change_the_damage_range_and_odds() {
    let save = fixture();
    let base = bravery_odds(&save, 0, Runtime::default(), None).expect("the run is valid");
    let boosted = bravery_odds(
        &save,
        0,
        Runtime {
            boosts: 10,
            ..Runtime::default()
        },
        None,
    )
    .expect("the run is valid");
    assert!(boosted.stats.max_dmg > base.stats.max_dmg);
    assert!(boosted.next_damage_factor < base.next_damage_factor);

    let spent = bravery_odds(
        &save,
        0,
        Runtime {
            rerolls: Some(0),
            ..Runtime::default()
        },
        None,
    )
    .expect("the run is valid");
    assert!(spent.stats.max_dmg < base.stats.max_dmg);

    // More stacks raise the damage, so the right-most sword kills earlier and
    // the chance it instead grants a boost falls.
    let tail = bravery_odds(&save, 52, Runtime::default(), None).expect("the run is valid");
    let tail_boosted = bravery_odds(
        &save,
        52,
        Runtime {
            boosts: 1,
            ..Runtime::default()
        },
        None,
    )
    .expect("the run is valid");
    assert!(tail_boosted.boost_chance < tail.boost_chance);
}

#[test]
fn a_partial_save_gets_milestone_etas_instead_of_infinities() {
    let mut holes = vec![Value::Null; 27];
    holes[7] = json!([0.0, 0.0, 0.0, 0.0]);
    holes[11] = json!([]);
    holes[14] = json!([100.0, 2.0]);
    holes[15] = json!(vec![0.0; 30]);
    holes[22] = json!(vec![0.0; 20]);
    holes[24] = json!(vec![0.0; 40]);
    holes[26] = json!(vec![0.0; 30]);
    let save = Save {
        holes,
        ..Save::default()
    };
    let training = training(&save, 0);
    close(training.rate, 1.0);
    let rows = milestones(&save, training.rate);
    assert!(rows[0].unlocked && rows[0].ready);
    assert!(rows[1].unlocked && rows[1].ready);
    assert!(!rows[2].unlocked && !rows[2].ready);
    assert_eq!(rows[2].hours_needed, 200.0);
    assert_eq!(rows[2].real_hours, 200.0);
    assert_eq!(rows[7].real_hours, 23_900.0);
}

#[test]
fn invalid_inputs_are_rejected() {
    let save = fixture();
    assert_eq!(
        bravery_odds(&save, 1001, Runtime::default(), None),
        Err(Error::WaveTooLarge)
    );
    let stats = bravery_stats(&save, Runtime::default()).expect("the fixture is valid");
    assert_eq!(
        bravery_stats(
            &save,
            Runtime {
                rerolls: Some(stats.max_rerolls + 1),
                ..Runtime::default()
            }
        ),
        Err(Error::RerollsExceedMaximum)
    );
    assert_eq!(
        bravery_odds(&save, 0, Runtime::default(), Some(0.0)),
        Err(Error::RemainingHp)
    );
    assert_eq!(
        bravery_odds(&save, 0, Runtime::default(), Some(f64::NAN)),
        Err(Error::RemainingHp)
    );
    // The run stats are read before the HP, so the rethrow error wins when both
    // inputs are invalid.
    assert_eq!(
        bravery_odds(
            &save,
            0,
            Runtime {
                rerolls: Some(stats.max_rerolls + 1),
                ..Runtime::default()
            },
            Some(0.0)
        ),
        Err(Error::RerollsExceedMaximum)
    );
}

#[test]
fn a_save_without_the_monument_rows_is_unavailable() {
    let save = Save::default();
    let snapshot = snapshot(&save, 0);
    assert!(!snapshot.available);
    assert!(snapshot.milestones.is_empty());
    assert!(snapshot.bonuses.is_empty());
    let wisdom = wisdom_snapshot(&save, 0);
    assert!(!wisdom.available);
    assert!(wisdom.milestones.is_empty());
    assert!(wisdom.bonuses.is_empty());
}

/// The three rows every monument tab reads, with Wisdom's saved hours and
/// milestones.
fn wisdom_save() -> Save {
    let mut holes = vec![Value::Null; 27];
    holes[7] = json!(vec![0.0; 13]);
    holes[11] = json!(vec![0.0; 70]);
    holes[13] = json!(vec![0.0; 100]);
    holes[14] = json!([0.0, 0.0, 0.0, 0.0, 30_000.0, 8.0]);
    holes[15] = json!(vec![0.0; 30]);
    holes[22] = json!(vec![0.0; 20]);
    holes[24] = json!(vec![0.0; 40]);
    holes[26] = json!(vec![0.0; 30]);
    if let Value::Array(study) = &mut holes[26] {
        study[12] = json!(2.0);
    }
    Save {
        holes,
        ..Save::default()
    }
}

#[test]
fn wisdom_stats_follow_the_saved_milestones_and_study() {
    let mut save = wisdom_save();
    let stats = wisdom_stats(&save);
    assert_eq!(stats.hours, 30_000.0);
    assert_eq!(stats.milestones, 8);
    assert_eq!(stats.opal_chance, 0.5);
    // Bolaia study 12 is 1% per level: two levels add 2 to the five base attempts
    // and the five milestone attempts, while the saved hours add eight more.
    assert_eq!(stats.attempts, 19.0);
    assert_eq!(stats.attempts_per_board, 3.0);
    assert_eq!(stats.instamatches, 9.0);
    assert!(stats.reveal_row && stats.reveal_square);
    close(stats.seconds_per_match, 12.6);

    if let Value::Array(study) = &mut save.holes[26] {
        study[12] = json!(0.0);
    }
    assert_eq!(wisdom_stats(&save).attempts, 17.0);
}

#[test]
fn wisdom_milestones_read_the_wisdom_text_and_first_row_attempts() {
    let save = wisdom_save();
    let rows = wisdom_milestones(&save, 20.0);
    assert_eq!(rows.len(), 8);
    assert_eq!(rows[0].text, "Start_with_#_Attempts");
    assert_eq!(rows[7].hours, 24_000.0);
    assert!(rows[0].ready && rows[0].unlocked);
    assert_eq!(rows[7].hours_needed, 0.0);
    // Wisdom rows carry their own permanent reward text and saved levels.
    let bonuses = bonuses_for(&save, 2);
    assert_eq!(bonuses.len(), 10);
    assert_eq!(bonuses[0].level, save.hole(15, 20));
    assert_eq!(bonuses[0].text, data().bonus_text(20));
}

#[test]
fn the_reward_charge_grows_with_the_wait_and_beyond_the_linear_window() {
    let mut save = wisdom_save();
    if let Value::Array(row) = &mut save.holes[11] {
        row[13] = json!(100_000.0);
    }
    let now = reward_charge(&save, 2, 0, 0.0).expect("the charge is valid");
    let hour = reward_charge(&save, 2, 0, 3_600.0).expect("the charge is valid");
    let day = reward_charge(&save, 2, 0, 86_400.0).expect("the charge is valid");
    assert!(now.multi < hour.multi && hour.multi < day.multi);
    assert!(now.can_start);
    // A wait under the two-day linear window pays one 72000th per second.
    close(now.base, 100_000.0 / 72_000.0);
    close(hour.base, 103_600.0 / 72_000.0);
    close(now.max_linear, 2.0 * 86_400.0);

    // Beyond the window the wait keeps paying, but only a cube-root tail.
    if let Value::Array(row) = &mut save.holes[11] {
        row[13] = json!(1_036_800.0);
    }
    let waited = reward_charge(&save, 2, 0, 0.0).expect("the charge is valid");
    let week = reward_charge(&save, 2, 0, 7.0 * 86_400.0).expect("the charge is valid");
    assert!(week.multi > waited.multi);
    assert!(week.multi - waited.multi < 7.0 * 86_400.0 / 72_000.0);
}

#[test]
fn the_charge_reads_the_meritoc_vote_and_warns_when_its_map_is_unknown() {
    let mut save = wisdom_save();
    save.options = vec![json!(0.0); 480];
    save.options[453] = json!(7.0);
    save.options[472] = json!(1.0);
    save.options[464] = json!(6.0);
    if let Value::Array(row) = &mut save.holes[11] {
        row[13] = json!(100_000.0);
    }
    let unknown = reward_charge(&save, 2, 0, 0.0).expect("the charge is valid");
    assert_eq!(unknown.meritoc, 0.0);
    let notes = wisdom_snapshot(&save, 0).notes;
    assert!(
        notes
            .iter()
            .any(|note| note.contains("Summoning access is missing")),
        "{notes:?}"
    );

    // A cleared map 250 activates the vote: base 200, +5% ClamWork, can vote.
    save.kla = vec![json!({ "250": [0.0, 0.0] })];
    let cleared = reward_charge(&save, 2, 0, 0.0).expect("the charge is valid");
    close(cleared.meritoc, 200.0 * 1.05);
    assert!(cleared.multi > unknown.multi);
    let notes = wisdom_snapshot(&save, 0).notes;
    assert!(!notes.iter().any(|note| note.contains("Meritoc")));

    // One kill left locks the vote until the map is cleared again.
    save.kla = vec![json!({ "250": [3.0, 0.0] })];
    assert_eq!(reward_charge(&save, 2, 0, 0.0).map(|c| c.meritoc), Ok(0.0));
    assert_eq!(meritoc(&save, 7, 0), 0.0);
}

#[test]
fn invalid_charge_inputs_are_rejected() {
    let save = wisdom_save();
    assert_eq!(reward_charge(&save, 3, 0, 0.0), Err(Error::InvalidCharge));
    assert_eq!(reward_charge(&save, 2, 0, -1.0), Err(Error::InvalidCharge));
    assert_eq!(
        reward_charge(&save, 2, 0, f64::NAN),
        Err(Error::InvalidCharge)
    );
    assert_eq!(
        reward_charge(&save, 2, 0, f64::INFINITY),
        Err(Error::InvalidCharge)
    );
}
