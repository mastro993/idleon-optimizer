//! Bravery formula edges: run-state effects, partial saves and rejected inputs.
//!
//! The saved cards and the odds for every run state are compared against the
//! upstream calculator in `crates/core/tests/monument.rs`.

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
}
