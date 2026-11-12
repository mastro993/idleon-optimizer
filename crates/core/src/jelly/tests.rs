//! The Jelly Operator formulas that need no save.

use super::board;
use super::estimate::{mean, quantile, standard_error, wilson};
use super::metrics::{MetricOptions, attack_cadence, best_dps_multiplier_from_value};
use super::{Jelly, boss_attack_cooldown, boss_hp, boss_time, roid_active_frames};

#[test]
fn a_cell_footprint_wraps_with_the_column() {
    // A Plasmid is two slots wide, so column 17 cannot host it.
    assert_eq!(board::footprint(17, 1), None);
    assert_eq!(board::footprint(16, 1), Some(vec![16, 17]));
    // An Organelle reaches a slot either side of its anchor.
    assert_eq!(board::footprint(19, 3), Some(vec![19, 20, 18, 37, 1]));
    assert_eq!(
        board::footprint(18, 3),
        None,
        "the left offset leaves the row"
    );
    assert_eq!(
        board::footprint(2, 3),
        None,
        "the 18 offset leaves the board"
    );
}

#[test]
fn a_footprint_that_leaves_the_board_is_refused() {
    // The Gigacyst covers 3×3 slots, so the last row cannot carry it.
    assert!(board::footprint(170, 7).is_none());
    assert!(board::footprint(180, 0).is_none());
    assert_eq!(board::footprint(179, 0), Some(vec![179]));
}

#[test]
fn the_boss_curves_match_the_game() {
    assert_eq!(boss_hp(0), 100.0);
    assert_eq!(boss_hp(11), 100_000.0);
    assert!(boss_hp(12) > boss_hp(11));
    assert_eq!(boss_time(11), 30.0);
    assert_eq!(boss_time(12), 35.0);
    assert_eq!(boss_time(32), 40.0);
    assert_eq!(boss_attack_cooldown(0), 120.0);
    assert_eq!(boss_attack_cooldown(72), 10.0);
    assert_eq!(boss_attack_cooldown(32), 120.0 - 32.0 / 6.0 * 20.0);
}

#[test]
fn stronkroid_drains_for_491_frames() {
    assert_eq!(roid_active_frames(), 491);
    assert!((super::roid_duration_seconds() - 491.0 / 60.0).abs() < 1e-12);
}

#[test]
fn a_cadence_breaks_on_whole_frames() {
    let cadence = attack_cadence(100.0, Some(100.0), Some(1.0));
    assert_eq!(cadence.frames_per_attack, 100.0);
    assert_eq!(cadence.attacks_per_second, 0.6);
    // A frame is never skipped, so a partial frame rounds up.
    let partial = attack_cadence(100.0, Some(100.5), Some(1.0));
    assert_eq!(partial.frames_per_attack, 101.0);
    let default = attack_cadence(65.0, None, None);
    assert_eq!(default.frames_per_attack, 100.0, "0.65 progress a frame");
}

#[test]
fn the_best_dps_curve_grows_with_the_best_run() {
    let low = best_dps_multiplier_from_value(0.0);
    let mid = best_dps_multiplier_from_value(1_000_000.0);
    let high = best_dps_multiplier_from_value(1e15);
    assert!((low - 1.0).abs() < 1e-12);
    assert!(mid > low && high > mid);
}

#[test]
fn the_wilson_interval_stays_inside_zero_and_one() {
    assert_eq!(wilson(0.0, 0.0), (0.0, 0.0));
    let (low, high) = wilson(0.0, 8.0);
    assert_eq!(low, 0.0);
    assert!(high > 0.0 && high < 1.0);
    let (low, high) = wilson(8.0, 8.0);
    assert!(low > 0.0 && low < 1.0);
    assert_eq!(high, 1.0);
}

#[test]
fn the_statistics_helpers_match_their_definitions() {
    assert_eq!(mean(&[]), 0.0);
    assert_eq!(mean(&[1.0, 2.0, 3.0]), 2.0);
    assert_eq!(quantile(&[], 0.5), None);
    assert_eq!(quantile(&[3.0, 1.0, 2.0], 0.5), Some(2.0));
    assert_eq!(quantile(&[1.0, 2.0], 0.5), Some(1.5));
    assert_eq!(standard_error(&[1.0]), 0.0);
    assert!(standard_error(&[1.0, 2.0, 3.0]) > 0.0);
}

#[test]
fn the_menagerie_doubler_doubles_an_upgrade() {
    let mut jelly = Jelly {
        upgrades: vec![0; 40],
        ..Jelly::default()
    };
    jelly.upgrades[36] = 1;
    assert_eq!(
        jelly.upgrade_quantity(36),
        1.0,
        "Critical Status is 1 a level"
    );
    jelly.account.doubled = vec![36];
    assert_eq!(jelly.upgrade_quantity(36), 2.0);
    assert_eq!(jelly.upgrade_quantity(99), 0.0, "no such upgrade");
}

#[test]
fn the_count_bonus_needs_its_upgrade() {
    let text = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/data.json"
    ))
    .expect("fixtures/data.json is committed");
    let imported = crate::save::import(&text).expect("the fixture imports");
    let mut jelly = imported.jelly.expect("the fixture carries a board");
    let with_bonus = jelly.layout_metrics(&jelly.layout, MetricOptions::default());
    assert!(with_bonus.valid);
    jelly.upgrades[14] = 0;
    let without = jelly.layout_metrics(&jelly.layout, MetricOptions::default());
    assert_eq!(without.counts, without.raw_counts);
    assert_eq!(
        with_bonus.counts[0],
        with_bonus.raw_counts[0] + (with_bonus.raw_counts[0] / 3.0).floor()
    );
    assert_eq!(
        with_bonus.counts[5], with_bonus.raw_counts[5],
        "viruses keep their count"
    );
}
