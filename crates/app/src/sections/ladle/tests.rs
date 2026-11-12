//! The ladle section's own rules: who is counted, whose breakdown is drawn, what
//! the Cook EXP tab plans for and how the reset tab reads its controls.

use super::exp::{exp_to_level, target_choice};
use super::forecast::toggle_day;
use super::kitchens::{inputs, parse_policy, policy_id};
use super::summary::{all, line, total};
use super::{adopt, toggle};
use idleon_ladle::kitchen::Policy;
use idleon_ladle::{Character, EXP_BALLOONS, Ladle, Panel};

fn character(index: usize, cooking: bool) -> Character {
    Character {
        index,
        name: format!("Character {}", index + 1),
        afk_target: match cooking {
            true => "Cooking".into(),
            false => "Fighting".into(),
        },
        level: 10.0 + index as f64,
        exp: 0.0,
        pending_hours: None,
        double_chance: 0.0,
        claim_capped: false,
    }
}

fn ladle(cooking: &[usize]) -> Ladle {
    Ladle {
        characters: (0..3)
            .map(|index| character(index, cooking.contains(&index)))
            .collect(),
        ladles: Default::default(),
        balloons: vec![Default::default(); EXP_BALLOONS.len()],
        reset_hours: Some(7.5),
        kitchen: None,
    }
}

#[test]
fn toggling_counts_a_character_once_in_save_order() {
    let mut assigned = Vec::new();
    toggle(&mut assigned, 2);
    toggle(&mut assigned, 0);
    assert_eq!(assigned, vec![0, 2]);
    toggle(&mut assigned, 2);
    assert_eq!(assigned, vec![0]);
}

#[test]
fn a_new_roster_counts_the_cooking_characters_until_something_is_checked() {
    let mut assigned = Vec::new();
    adopt(&mut assigned, &ladle(&[0, 2]));
    assert_eq!(assigned, vec![0, 2]);

    // A manual choice survives an import that still carries the characters.
    let mut manual = vec![1];
    adopt(&mut manual, &ladle(&[0, 2]));
    assert_eq!(manual, vec![1]);

    // Characters the new save does not carry are dropped.
    let mut shorter = ladle(&[]);
    shorter.characters.truncate(2);
    let mut stale = vec![1, 2];
    adopt(&mut stale, &shorter);
    assert_eq!(stale, vec![1]);

    // A save with no roster leaves nothing checked.
    let mut checked = vec![0];
    adopt(
        &mut checked,
        &Ladle {
            characters: Vec::new(),
            ..Ladle::default()
        },
    );
    assert!(checked.is_empty());
}

#[test]
fn the_header_line_names_the_totals_and_the_reset() {
    let ladle = ladle(&[0, 2]);
    let panel = Panel {
        efficiency: 1000.0,
        afk: 1.0,
        ..Panel::default()
    };
    let text = line(&ladle, panel, 24.0, &[0, 2]);
    assert!(text.contains("0 ladles owned"), "{text}");
    assert!(text.contains("guaranteed / day"), "{text}");
    assert!(text.contains("7.5h to reset"), "{text}");
    let unknown_reset = line(
        &Ladle {
            reset_hours: None,
            ..ladle.clone()
        },
        panel,
        24.0,
        &[0],
    );
    assert!(
        unknown_reset.contains("reset countdown missing"),
        "{unknown_reset}"
    );
}

#[test]
fn the_totals_count_only_the_checked_characters() {
    let ladle = ladle(&[0, 2]);
    let stats = all(
        &ladle,
        Panel {
            efficiency: 1000.0,
            afk: 1.0,
            ..Panel::default()
        },
        24.0,
    );
    // 15 ladles a day each, all in one claim, floored by the 24h window.
    assert_eq!(total(&stats, &[0], |row| row.panel_per_day), 15.0);
    assert_eq!(total(&stats, &[0, 1, 2], |row| row.panel_per_day), 45.0);
    assert_eq!(total(&stats, &[], |row| row.panel_per_day), 0.0);
    assert_eq!(total(&stats, &[1], |row| row.per_day), 15.0);
}

#[test]
fn the_exp_target_is_the_entered_level_or_the_next_one() {
    assert_eq!(target_choice(0, 42.0), None);
    assert_eq!(target_choice(42, 42.0), None);
    assert_eq!(target_choice(45, 42.0), Some(45.0));
}

#[test]
fn afk_time_costs_the_exp_between_the_two_levels() {
    let next = exp_to_level(42.0, 0.0, 43.0);
    close(next, idleon_ladle::cooking_exp_req(42.0));
    // Banked EXP already paid for part of it.
    let banked = exp_to_level(42.0, 1_000_000.0, 43.0);
    close(banked, idleon_ladle::cooking_exp_req(42.0) - 1_000_000.0);
    // Never negative, and nothing to do at the current level.
    close(exp_to_level(42.0, 1e30, 43.0), 0.0);
    close(exp_to_level(42.0, 0.0, 42.0), 0.0);
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{actual} != {expected}"
    );
}

#[test]
fn the_donation_policy_round_trips_through_its_select_value() {
    for policy in [Policy::None, Policy::All, Policy::Share, Policy::Fixed] {
        assert_eq!(parse_policy(policy_id(policy)), policy);
    }
    // An unknown value falls back to no donations rather than panicking.
    assert_eq!(parse_policy("auto"), Policy::None);
}

#[test]
fn expanding_a_reset_keeps_the_list_in_day_order() {
    let mut open = vec![1];
    toggle_day(&mut open, 3);
    toggle_day(&mut open, 2);
    assert_eq!(open, vec![1, 2, 3]);
    toggle_day(&mut open, 2);
    assert_eq!(open, vec![1, 3]);
}

#[test]
fn the_plan_inputs_come_from_the_checked_rates_and_the_save() {
    let ladle = ladle(&[0, 2]);
    let panel = Panel {
        efficiency: 1000.0,
        afk: 1.0,
        talent_148: 50.0,
        ..Panel::default()
    };
    let stats = all(&ladle, panel, 24.0);
    let settings = idleon_ladle::kitchen::Inputs::default();
    let plan = inputs(&settings, &[0, 2], panel, true, &ladle, &stats);
    // 15 ladles a day each, both checked.
    assert_eq!(plan.ladles_per_day, 30.0);
    assert_eq!(plan.hours_to_reset, 7.5);
    assert_eq!(plan.start_ladles, ladle.ladles.value);
    assert_eq!(plan.ladle_multi, 1.5);
    // The save's ladles are only spent when the switch is on.
    assert_eq!(
        inputs(&settings, &[0, 2], panel, false, &ladle, &stats).start_ladles,
        0.0
    );
}
