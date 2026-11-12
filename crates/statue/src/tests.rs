//! Statue unit tests: the requirement table, the deposit multipliers and the
//! batch plan.

use super::*;
use batch::parse_level;
use idleon_common::formula::ValueFormat;

fn row(level: u32, exp: f64) -> Option<Progress> {
    Some(Progress { level, exp })
}

fn sample(rows: Vec<Option<Progress>>) -> StatueSave {
    StatueSave::new(vec![rows], vec!["main".into()])
}

#[test]
fn the_bundled_table_covers_every_statue() {
    assert_eq!(count(), 32);
    assert_eq!(info(0).unwrap().name, "POWER");
    assert_eq!(info(0).unwrap().base_per_level, 3.0);
    assert_eq!(info(1).unwrap().effect, "MOVE SPEED");
    assert!(info(1).unwrap().percent);
    assert_eq!(info(31).unwrap().name, "CORAL");
    assert!(info(32).is_none());
}

#[test]
fn single_digit_statues_round_on_their_own_target_level() {
    let save = sample(vec![row(10, 0.0); count()]);
    assert_eq!(exp_requirement(&save, 0, 0, 1).unwrap(), 2.0);
    assert_eq!(exp_requirement(&save, 0, 5, 141).unwrap(), 22_507.0);
    assert_eq!(exp_requirement(&save, 0, 9, 249).unwrap(), 1_119_089.0);
}

#[test]
fn higher_statues_round_on_their_gate_statue() {
    let mut rows = vec![row(0, 0.0); count()];
    rows[1] = row(141, 0.0);
    let save = sample(rows.clone());
    // Statue 10 rounds on statue 1's saved level (141), not its own level.
    assert_eq!(exp_requirement(&save, 0, 10, 432).unwrap(), 517_529_111.0);
    // Past gate level 300 the raw value is kept.
    rows[1] = row(301, 0.0);
    let save = sample(rows);
    let raw = exp_requirement(&save, 0, 10, 432).unwrap();
    assert_ne!(raw, raw.round());
}

#[test]
fn a_missing_gate_row_is_unavailable() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[1] = None;
    let save = sample(rows);
    assert_eq!(
        exp_requirement(&save, 0, 10, 432),
        Err(Unavailable::NoRow(1))
    );
    assert_eq!(next_level(&save, 0, 10), Err(Unavailable::NoRow(1)));
}

#[test]
fn next_level_reports_the_full_bar_and_the_remainder() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[3] = row(141, 100.0);
    let save = sample(rows);
    let next = next_level(&save, 0, 3).unwrap();
    assert_eq!(next.level, 141);
    assert_eq!(next.exp, 100.0);
    assert_eq!(next.required, 22_507.0);
    assert_eq!(next.remaining, 22_407.0);
}

#[test]
fn an_unknown_character_or_statue_is_unavailable() {
    let save = sample(vec![row(10, 0.0); count()]);
    assert_eq!(next_level(&save, 1, 0), Err(Unavailable::NoRow(0)));
    assert_eq!(
        next_level(&save, 0, count()),
        Err(Unavailable::NoRow(count()))
    );
    assert_eq!(
        exp_requirement(&save, 0, count(), 1),
        Err(Unavailable::NoStatue(count()))
    );
}

#[test]
fn the_startue_bubble_caps_retention_at_half() {
    let none = Leveling::new(0.0, 0.0, 0, 0, Vec::new(), false);
    assert_eq!(none.cost_factor, 1.0);
    assert_eq!(none.retention_pct, 0.0);
    assert_eq!(none.tier_multiplier, 1.0);

    let part = Leveling::new(20.0, 0.0, 0, 0, Vec::new(), false);
    assert!((part.cost_factor - 0.8).abs() < 1e-12);
    assert!((part.retention_pct - 20.0).abs() < 1e-12);

    let full = Leveling::new(68.4, 0.0, 0, 0, Vec::new(), false);
    assert_eq!(full.cost_factor, 0.5, "retention is capped at 50%");
    assert_eq!(full.retention_pct, 50.0);
    let negative = Leveling::new(-30.0, 0.0, 0, 0, Vec::new(), false);
    assert_eq!(negative.bubble_pct, 0.0, "a negative bubble reads as none");
}

#[test]
fn the_tier_multiplier_adds_onyx_and_zenith() {
    let leveling = Leveling::new(0.0, 0.0, 10, 4, Vec::new(), false);
    assert!((leveling.tier_multiplier - (1.0 + 0.1 * 10.0 + 0.15 * 4.0)).abs() < 1e-12);
    assert_eq!(leveling.double_chance, 0.0);
    let capped = Leveling::new(0.0, 3.0, 0, 0, Vec::new(), false);
    assert_eq!(
        capped.double_chance, 1.0,
        "the vial chance is clamped to 100%"
    );
}

#[test]
fn flair_multipliers_are_the_saved_tiers() {
    assert_eq!(flair_multiplier(0), 1.0);
    assert_eq!(flair_multiplier(1), 7.0);
    assert_eq!(flair_multiplier(2), 15.0);
    assert_eq!(flair_multiplier(3), 25.0);
    assert_eq!(flair_multiplier(4), 33.0);
}

#[test]
fn the_bubble_changes_the_statues_a_target_needs() {
    let save = sample(vec![row(100, 0.0); count()]);
    let none = Leveling::default();
    let bubbled = Leveling::new(50.0, 0.0, 0, 0, Vec::new(), true);
    let plain = target_cost(&save, &none, 0, 0, 105).unwrap();
    let cheap = target_cost(&save, &bubbled, 0, 0, 105).unwrap();
    assert!(
        cheap.exp_needed < plain.exp_needed,
        "retention lowers the EXP a target needs: {} vs {}",
        cheap.exp_needed,
        plain.exp_needed
    );
    assert!(cheap.statues_guaranteed < plain.statues_guaranteed);
}

#[test]
fn retention_lowers_consumed_exp_but_keeps_the_triggering_bar() {
    let save = sample(vec![row(10, 0.0); count()]);
    let leveling = Leveling::new(50.0, 0.0, 0, 0, Vec::new(), true);
    let cost = target_cost(&save, &leveling, 0, 0, 13).unwrap();
    let bar = |level| exp_requirement(&save, 0, 0, level).unwrap();
    // Levels 10 and 11 consume half; level 12 is the bar that triggers the
    // last level-up, so it is required in full.
    let expected = 0.5 * (bar(10) + bar(11)) + bar(12);
    assert!((cost.exp_needed - expected).abs() <= expected * 1e-12);
}

#[test]
fn a_target_at_or_below_the_saved_level_needs_nothing() {
    let save = sample(vec![row(42, 7.0); count()]);
    let leveling = Leveling::default();
    for target in [0, 41, 42] {
        let cost = target_cost(&save, &leveling, 0, 0, target).unwrap();
        assert_eq!(cost.exp_needed, 0.0, "target {target}");
        assert_eq!(cost.statues_guaranteed, 0);
    }
}

#[test]
fn the_estimate_switches_between_guaranteed_and_average_deposits() {
    let save = sample(vec![row(10, 0.0); count()]);
    let leveling = Leveling::new(0.0, 1.0, 0, 0, Vec::new(), true);
    let cost = target_cost(&save, &leveling, 0, 0, 20).unwrap();
    assert_eq!(cost.flair_level, 0, "a save without a Flair row assumes 1x");
    assert_eq!(cost.flair_multiplier, 1.0);
    assert_eq!(
        cost.guaranteed_exp, 2.0,
        "a full vial chance doubles deposits"
    );
    assert_eq!(cost.average_exp, 2.0);
    assert_eq!(
        Estimate::Guaranteed.statues(&cost),
        Estimate::Average.statues(&cost)
    );
    assert_eq!(Estimate::Guaranteed.deposit(&cost), 2.0);

    let partial = Leveling::new(0.0, 0.25, 0, 0, Vec::new(), true);
    let cost = target_cost(&save, &partial, 0, 1, 20).unwrap();
    assert_eq!(cost.guaranteed_exp, 1.0);
    assert_eq!(cost.average_exp, 1.25);
    assert!(cost.statues_average < cost.statues_guaranteed);
}

#[test]
fn a_flair_row_multiplies_that_statue_only() {
    let save = sample(vec![row(10, 0.0); count()]);
    let leveling = Leveling::new(0.0, 0.0, 0, 0, vec![0, 2], true);
    let plain = target_cost(&save, &leveling, 0, 0, 20).unwrap();
    let flaired = target_cost(&save, &leveling, 0, 1, 20).unwrap();
    assert_eq!(plain.flair_multiplier, 1.0);
    assert_eq!(flaired.flair_level, 2);
    assert_eq!(flaired.flair_multiplier, 15.0);
    assert!(flaired.guaranteed_exp > plain.guaranteed_exp);
    assert!(flaired.statues_guaranteed < plain.statues_guaranteed);
}

#[test]
fn a_missing_row_cannot_be_costed() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[5] = None;
    let save = sample(rows);
    assert_eq!(
        target_cost(&save, &Leveling::default(), 0, 5, 12),
        Err(Unavailable::NoRow(5))
    );
    assert_eq!(
        target_cost(&save, &Leveling::default(), 0, count(), 12),
        Err(Unavailable::NoRow(count()))
    );
}

#[test]
fn a_rate_reads_suffixes_exponents_and_empty_entries() {
    assert_eq!(parse_rate(""), Ok(0.0));
    assert_eq!(parse_rate("   "), Ok(0.0));
    assert_eq!(parse_rate("500"), Ok(500.0));
    assert_eq!(parse_rate("500K"), Ok(500_000.0));
    assert_eq!(parse_rate("1.2m"), Ok(1_200_000.0));
    assert_eq!(parse_rate(" 2 B "), Ok(2_000_000_000.0));
    assert_eq!(parse_rate(".5"), Ok(0.5));
    assert_eq!(parse_rate("1e9"), Ok(1e9));
    assert_eq!(parse_rate("1.5e-3"), Ok(0.0015));
    assert_eq!(parse_rate("2.5 T"), Ok(2.5e12));
}

#[test]
fn an_invalid_rate_names_the_entry() {
    for text in ["abc", "-1", "1.2.3", "1e", "K", "1 K M", "NaN", "inf"] {
        assert_eq!(
            parse_rate(text),
            Err(RateError::Invalid),
            "the rate {text:?} is rejected"
        );
    }
    assert_eq!(parse_rate("1e999"), Err(RateError::TooLarge));
    assert_eq!(
        RateError::Invalid.message(),
        "Enter a nonnegative rate, such as 500K, 1.2M or 1e9."
    );
}

#[test]
fn farm_time_waits_on_a_rate_and_reads_ready_at_zero() {
    assert_eq!(hours(0, 500.0), 0.0);
    assert_eq!(hours(10, 2.0), 5.0);
    assert!(hours(10, 0.0).is_infinite());
    assert!(hours(10, -1.0).is_infinite());
    assert!(hours(10, f64::INFINITY).is_infinite());
}

#[test]
fn characters_without_a_readable_row_are_not_offered() {
    let save = StatueSave::new(
        vec![vec![None; count()], vec![row(1, 0.0); count()]],
        vec![],
    );
    assert!(!save.is_empty());
    assert_eq!(save.first_character(), Some(1));
    assert_eq!(save.name(1), "Character 2");
    assert_eq!(save.missing_rows(0), count());
    assert_eq!(save.missing_rows(1), 0);
}

#[test]
fn a_batch_entry_reads_whole_nonnegative_levels() {
    assert_eq!(parse_level("0"), Ok(0));
    assert_eq!(parse_level(" 150 "), Ok(150));
    assert_eq!(parse_level("1e3"), Ok(1000));
    assert_eq!(parse_level("150.0"), Ok(150));
    for text in ["", "  ", "-1", "12.5", "abc", "NaN", "inf", "1e20", "0x10"] {
        assert_eq!(
            parse_level(text),
            Err(BatchError::InvalidLevel),
            "the entry {text:?} is rejected"
        );
    }
    assert_eq!(
        BatchError::InvalidLevel.message(),
        "Enter a nonnegative whole target level."
    );
}

#[test]
fn set_all_costs_every_statue_before_it_writes_the_plan() {
    let save = sample(vec![row(10, 0.0); count()]);
    let mut plan = Plan::new();
    plan.set_all(&save, 0, "20").expect("a valid level applies");
    assert!(
        plan.targets().iter().all(|target| *target == Some(20)),
        "every statue takes the entry"
    );

    let mut plan = Plan::new();
    plan.set_target(0, 3);
    let error = plan
        .set_all(&save, 0, "1000000")
        .expect_err("past the finite range");
    assert!(matches!(
        error,
        BatchError::Unavailable(_, Unavailable::TargetOutOfRange)
    ));
    assert_eq!(
        error.message(),
        "Statue target EXP exceeds the finite calculation range."
    );
    assert_eq!(
        plan.target(0),
        Some(3),
        "a batch that cannot be costed changes nothing"
    );

    let out_of_range = sample(vec![row(30_000, 0.0); count()]);
    let error = plan
        .set_all(&out_of_range, 0, "31000")
        .expect_err("the saved level's own bar overflows");
    assert!(matches!(
        error,
        BatchError::Unavailable(_, Unavailable::OutOfRange)
    ));
    assert_eq!(
        error.message(),
        "Statue level exceeds the finite EXP calculation range."
    );
}

#[test]
fn a_batch_needing_a_missing_gate_names_it() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[1] = None;
    let save = sample(rows);
    let mut plan = Plan::new();
    plan.set_target(0, 3);
    let error = plan
        .set_all(&save, 0, "20")
        .expect_err("the gate row is missing");
    assert_eq!(error, BatchError::Unavailable(10, Unavailable::NoRow(1)));
    assert_eq!(
        error.message(),
        format!(
            "{} rounds its EXP on the saved level of {}, which this save lacks.",
            info(10).expect("statue 10").name,
            info(1).expect("MOVE SPEED").name
        )
    );
    assert_eq!(plan.target(0), Some(3));
}

#[test]
fn a_statue_without_a_row_is_skipped_not_rejected() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[5] = None;
    let save = sample(rows);
    let mut plan = Plan::new();
    plan.set_all(&save, 0, "20")
        .expect("a missing own row leaves the batch available");
    assert_eq!(plan.target(5), Some(20), "the plan still carries the level");
    assert_eq!(
        target_cost(&save, save.leveling(), 0, 5, 20),
        Err(Unavailable::NoRow(5))
    );
}

#[test]
fn a_batch_action_is_undoable() {
    let save = sample(vec![row(10, 0.0); count()]);
    let mut plan = Plan::new();
    plan.set_target(2, 7);
    let before = plan.targets().to_vec();
    plan.set_all(&save, 0, "40").expect("a valid level applies");
    assert_eq!(plan.undo_label(), Some("set all targets"));
    assert_eq!(plan.undo(), Some("set all targets"));
    assert_eq!(plan.targets(), before.as_slice(), "the plan is restored");
    assert_eq!(plan.undo(), None, "one undo step");

    plan.next_level();
    assert!(plan.targets().iter().all(Option::is_none));
    assert_eq!(plan.undo(), Some("next level for all"));
    assert_eq!(
        plan.target(2),
        Some(7),
        "next level clears the entries only"
    );
    assert_eq!(plan.undo_label(), None);
}

#[test]
fn a_hand_edit_ends_the_undo_step() {
    let save = sample(vec![row(10, 0.0); count()]);
    let mut plan = Plan::new();
    plan.set_all(&save, 0, "40").expect("a valid level applies");
    plan.set_target(0, 41);
    assert_eq!(plan.undo_label(), None);
    assert_eq!(plan.undo(), None);
}

#[test]
fn a_project_reset_sizes_the_plan_and_drops_the_undo_step() {
    let save = sample(vec![row(10, 0.0); count()]);
    let mut plan = Plan::new();
    plan.set_all(&save, 0, "40").expect("a valid level applies");
    plan.reset(vec![Some(12)], vec![" 1.2M ".to_string()]);
    assert_eq!(plan.targets().len(), count());
    assert_eq!(plan.target(0), Some(12));
    assert_eq!(plan.target(count() - 1), None);
    assert_eq!(plan.rate(0), "1.2M");
    assert_eq!(plan.rate(count() - 1), "");
    assert_eq!(plan.undo_label(), None);

    plan.clear();
    assert!(plan.targets().iter().all(Option::is_none));
    assert_eq!(plan.rates(), &vec![String::new(); count()][..]);
}

fn with_leveling(leveling: Leveling) -> StatueSave {
    let mut save = sample(vec![row(100, 0.0); count()]);
    save.set_leveling(leveling);
    save
}

#[test]
fn the_breakdown_terms_account_for_the_row_it_describes() {
    let save = sample(vec![row(100, 0.0); count()]);
    let cost = target_cost(&save, save.leveling(), 0, 0, 105).expect("a target");
    let tree = breakdown_tree(&save, 0, 0, Some(105), Estimate::Guaranteed, 1_000_000.0);
    assert_eq!(tree.value, cost.statues_guaranteed as f64);
    assert_eq!(tree.format, ValueFormat::Count);
    assert_eq!(tree.children.len(), 4);

    let needed = &tree.children[0];
    assert_eq!(needed.label, "EXP needed");
    assert_eq!(needed.value, cost.exp_needed);
    assert_eq!(needed.children.len(), 2, "the bubble and its retention");

    let deposit = &tree.children[1];
    assert_eq!(deposit.label, "EXP per deposited statue");
    assert_eq!(deposit.value, cost.guaranteed_exp);
    let product: f64 = deposit.children.iter().map(|child| child.value).product();
    assert!(
        (product - cost.guaranteed_exp).abs() <= cost.guaranteed_exp * 1e-12,
        "the tier, Flair and vial terms multiply to the deposit: {product} vs {}",
        cost.guaranteed_exp
    );

    let rate = &tree.children[2];
    assert_eq!(rate.value, 1_000_000.0);
    let time = &tree.children[3];
    assert_eq!(
        time.value,
        cost.statues_guaranteed as f64 / 1_000_000.0,
        "the time is the statues the rate covers"
    );
}

#[test]
fn the_breakdown_reads_the_bubble_the_tier_and_the_flair() {
    let save = with_leveling(Leveling::new(50.0, 0.25, 2, 1, vec![3], true));
    let guaranteed = breakdown_tree(&save, 0, 0, Some(101), Estimate::Guaranteed, 0.0);
    let deposit = &guaranteed.children[1];
    assert_eq!(deposit.value, 1.35 * 25.0);
    let terms: Vec<(&str, f64)> = deposit
        .children
        .iter()
        .map(|child| (child.label.as_str(), child.value))
        .collect();
    assert_eq!(
        terms,
        [
            ("Onyx / Zenith tier", 1.35),
            ("Royal Guardian Flair", 25.0),
            ("Skinny 0 Cal vial", 1.0),
        ]
    );
    let needed = &guaranteed.children[0];
    assert_eq!(needed.children[0].value, 50.0, "the STARTUE bubble");
    assert_eq!(needed.children[1].value, 50.0, "retention, capped at 50%");
    assert!(
        guaranteed.children[3].value.is_infinite(),
        "no rate means no time estimate"
    );

    let average = breakdown_tree(&save, 0, 0, Some(101), Estimate::Average, 0.0);
    assert_eq!(average.children[1].children[2].value, 1.25);
    assert!(
        average.value < guaranteed.value,
        "doubles need fewer statues"
    );
}

#[test]
fn an_unavailable_row_names_its_reason_in_the_breakdown() {
    let mut rows = vec![row(10, 0.0); count()];
    rows[5] = None;
    let save = sample(rows.clone());
    let tree = breakdown_tree(&save, 0, 5, Some(20), Estimate::Guaranteed, 0.0);
    assert_eq!(tree.value, 0.0);
    assert_eq!(tree.note.as_deref(), Some("Missing row"));
    assert!(tree.children.is_empty());

    rows[1] = None;
    let save = sample(rows);
    let tree = breakdown_tree(&save, 0, 10, Some(20), Estimate::Guaranteed, 0.0);
    assert_eq!(
        tree.note.as_deref(),
        Some(format!("Needs {}", info(1).expect("SPEED").name).as_str()),
        "a missing gate names the gate statue"
    );
    let tree = breakdown_tree(&save, 0, 2, None, Estimate::Guaranteed, 0.0);
    assert!(tree.note.is_some());
}
