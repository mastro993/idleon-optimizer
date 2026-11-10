//! Bravery formula parity against values evaluated from the upstream JS.

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
    Save::from(&imported.data)
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn training_matches_the_upstream_sources_and_rate() {
    let training = training(&fixture(), 0);
    let expected = [
        ("Bravery: All Monument AFK Gain", 105.9),
        ("Justice: All Monument AFK Gain", 98.1),
        ("Wisdom: All Monument AFK Gain", 98.0),
        ("Summoning victories", 438.633),
        ("Jar collectible: Monument AFK gain", 360.0),
        ("Rock Smart", 20.0),
        ("Monument AFK measurement", 167.214_711_111_111_1),
        ("Arcade: Monument AFK gain", 50.0),
        ("Hug from Timmy", 10.0),
        ("Compass: Monument AFK gain", 200.0),
    ];
    assert_eq!(training.sources.len(), expected.len());
    for (source, (name, value)) in training.sources.iter().zip(expected) {
        assert_eq!(source.name, name);
        close(source.value, value);
    }
    close(training.pct, 1_547.847_711_111_111);
    close(training.multi, 1.25);
    close(training.rate, 20.598_096_388_888_89);
    assert_eq!(training.per_day, 494.0);
}

#[test]
fn bravery_stats_match_the_upstream_cards() {
    let stats = bravery_stats(&fixture(), Runtime::default()).expect("the fixture is valid");
    assert_eq!(stats.hours, 121945.0);
    assert_eq!(stats.milestones, 8);
    close(stats.opal_chance, 0.018_089_404_296_875);
    close(stats.min_dmg, 73_598_959.228_639_99);
    close(stats.max_dmg, 229_933_225.714_499_95);
    assert_eq!(stats.max_rerolls, 15);
    assert_eq!(stats.rerolls, 15);
    assert_eq!(stats.swords, 8);
    assert_eq!(stats.retellings, 1);
    assert!(stats.rightmost);
    assert!(stats.auto_kill);
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
fn sword_odds_match_the_upstream_values() {
    let save = fixture();
    let base = bravery_odds(&save, 0, Runtime::default(), None).expect("the run is valid");
    assert_eq!(base.hp, 10.0);
    assert!(base.automatic);
    close(base.kill, 1.0);
    close(base.boost_chance, 1.0);
    assert_eq!(base.last_sword_chance, None);

    let wave_100 = bravery_odds(
        &save,
        100,
        Runtime {
            boosts: 10,
            ..Runtime::default()
        },
        None,
    )
    .expect("the run is valid");
    close(wave_100.hp, 374_379_601_755_863.1);
    assert!(!wave_100.automatic);
    close(wave_100.kill, 0.0);
    close(wave_100.boost_chance, 0.0);

    let wave_1000 = bravery_odds(
        &save,
        1000,
        Runtime {
            rerolls: Some(0),
            ..Runtime::default()
        },
        None,
    )
    .expect("the run is valid");
    close(wave_1000.hp, 1.317_446_533_493_369e118);
    close(wave_1000.kill, 0.0);

    let with_hp = bravery_odds(
        &save,
        50,
        Runtime {
            boosts: 3,
            ..Runtime::default()
        },
        Some(100_000.0),
    )
    .expect("the run is valid");
    close(with_hp.hp, 378_426_209.464_137_26);
    close(with_hp.kill, 1.0);
    close(with_hp.last_sword_chance.expect("HP was entered"), 1.0);
    close(with_hp.next_damage_factor, 1.076_923_076_923_076_9);

    // A tail case where the CDF is fractional: the Irwin-Hall sum must not
    // cancel near the upper end of the damage range.
    let tail =
        bravery_odds(&save, 52, Runtime::default(), Some(150_000_000.0)).expect("the run is valid");
    close(tail.hp, 664_785_305.599_433_7);
    close(tail.kill, 0.999_999_922_685_022_2);
    close(tail.boost_chance, 0.000_145_656_653_803_010_65);
    close(
        tail.last_sword_chance.expect("HP was entered"),
        0.511_296_899_337_994_5,
    );
}

#[test]
fn milestones_and_bonus_rows_match_the_upstream_tables() {
    let save = fixture();
    let training = training(&save, 0);
    let milestones = milestones(&save, training.rate);
    assert_eq!(milestones.len(), 8);
    assert_eq!(milestones[0].hours, 1.0);
    assert_eq!(milestones[0].text, "Swords_deal_{~}_damage");
    assert!(milestones.iter().all(|row| row.unlocked && row.ready));
    assert_eq!(milestones[7].hours, 24000.0);

    let bonuses = bonuses(&save);
    assert_eq!(bonuses.len(), 10);
    assert_eq!(bonuses[0].level, 14686.0);
    close(bonuses[0].value, 63_443.520_000_000_004);
    close(bonuses[0].next, 63_447.840_000_000_004);
    assert_eq!(bonuses[8].level, 12122.0);
    close(bonuses[8].value, 105.9);
    close(bonuses[8].next, 105.9);
    assert_eq!(bonuses[9].level, 0.0);
    close(bonuses[9].next, 1.1);
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
}

#[test]
fn a_save_without_the_monument_rows_is_unavailable() {
    let save = Save::default();
    let snapshot = snapshot(&save, 0);
    assert!(!snapshot.available);
    assert!(snapshot.milestones.is_empty());
    assert!(snapshot.bonuses.is_empty());
}
