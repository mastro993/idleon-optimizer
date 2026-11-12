//! The jar formulas on purpose-built saves: the readers, the tier scaling and
//! the terms each total is made of.

use super::*;
use crate::rupies::{INCOME_TIERS, VALUE_TIERS};
use serde_json::json;

/// A save carrying only `holes`.
fn jar(holes: Value) -> Jar {
    jar_with(holes, &[])
}

/// A save with `holes` and the given top-level fields.
fn jar_with(holes: Value, fields: &[(&str, Value)]) -> Jar {
    let mut save = Map::new();
    save.insert("Holes".into(), json!(holes.to_string()));
    for (key, value) in fields {
        save.insert((*key).into(), json!(value.to_string()));
    }
    Jar::from_save(&save).expect("the save carries Holes")
}

/// `Spelunk` with the given `(row, cell, value)` cells set.
fn spelunk(cells: &[(usize, usize, f64)]) -> Value {
    let mut rows = vec![vec![0.0; 50]; 55];
    for (row, cell, value) in cells {
        rows[*row][*cell] = *value;
    }
    rows[6] = vec![1.0; 3];
    rows.into()
}

fn blank_rows() -> Vec<Vec<f64>> {
    vec![vec![0.0; 150]; 33]
}

#[test]
fn a_save_without_holes_has_no_jar_state() {
    assert!(Jar::from_save(&Map::new()).is_none());
    let mut save = Map::new();
    save.insert("Holes".into(), json!("not an array"));
    assert!(Jar::from_save(&save).is_none());
}

#[test]
fn missing_rows_read_as_zero_instead_of_panicking() {
    let empty = jar(json!([]));
    assert_eq!(empty.types_available(), 1);
    assert_eq!(empty.production_slots(), 1);
    assert_eq!(empty.rupies(0), 0.0);
    assert_eq!(empty.collectibles_found(), 0);
    assert_eq!(empty.production_per_hour(), BASE_PRODUCTION_PER_HOUR);
    assert_eq!(empty.rupie_value(), 1.0);
    assert_eq!(empty.extra_rupie_chance(), 0.0);
    assert_eq!(empty.avg_rupies_per_break(), 1.0);
    // Nothing found: the collectible chance falls back to the upstream quarter.
    assert_eq!(empty.collectible_chance(), 0.25);
    assert_eq!(empty.opal_chance(), 0.25);
}

#[test]
fn a_slot_needs_one_schematic_per_extra_slot() {
    let mut rows = blank_rows();
    assert_eq!(jar(json!(rows)).production_slots(), 1);
    rows[13][63] = 1.0;
    assert_eq!(jar(json!(rows)).production_slots(), 2);
    rows[13][66] = 1.0;
    assert_eq!(jar(json!(rows)).production_slots(), 3);
}

#[test]
fn a_jar_type_costs_more_in_the_rupies_the_one_below_it_stores() {
    let empty = jar(json!([]));
    assert_eq!(empty.next_type_cost(), 50.0);
    assert!(!empty.can_afford_next_type());

    let mut rows = blank_rows();
    rows[9][20] = 50.0;
    assert!(jar(json!(rows)).can_afford_next_type());

    // All nine bought: there is no next type to pay for.
    let mut rows = blank_rows();
    rows[11][37] = 9.0;
    rows[9][29] = 1e30;
    assert!(!jar(json!(rows)).can_afford_next_type());
}

#[test]
fn the_production_measurement_scales_with_the_account_levels() {
    let mut rows = blank_rows();
    rows[22][12] = 100.0;
    let jar = jar_with(json!(rows), &[("Lv0_0", json!([3000]))]);
    // A TOT base at level 100 is half of 30, and six Tome quantities add 116%.
    assert!((jar.measurement_bonus(12) - 15.0 * 2.16).abs() < 1e-9);
    assert_eq!(jar.production_terms().len(), 1);
}

#[test]
fn the_production_line_divides_the_requirement_by_its_log() {
    let mut rows = blank_rows();
    assert_eq!(jar(json!(rows.clone())).production_requirement(3), 7000.0);
    rows[13][67] = 1.0;
    assert_eq!(
        jar(json!(rows.clone())).production_requirement(3),
        7000.0 / 1.3
    );
    rows[13][74] = 1.0;
    rows[11][42] = 1000.0;
    // `5 × log10(1000)` is 15 percent off the requirement.
    let expected = 7000.0 / (1.3 * (1.0 + 5.0 * log(1000.0) / 100.0));
    assert!((jar(json!(rows)).production_requirement(3) - expected).abs() < 1e-9);
}

#[test]
fn the_targeted_enchant_is_only_read_when_it_names_a_collectible() {
    let mut rows = blank_rows();
    // The game writes the target into the cell; zero names the first collectible.
    assert_eq!(jar(json!(rows.clone())).targeted_enchant(), Some(0));
    rows[11][62] = 30.0;
    assert_eq!(jar(json!(rows.clone())).targeted_enchant(), Some(30));
    rows[11][62] = 40.0;
    assert_eq!(jar(json!(rows)).targeted_enchant(), None);
}

#[test]
fn page_two_decides_how_many_collectibles_the_account_can_find() {
    let mut rows = blank_rows();
    let first_page = jar(json!(rows.clone()));
    assert_eq!(first_page.collectible_slots(), 16);
    rows[13][75] = 1.0;
    assert_eq!(jar(json!(rows)).collectible_slots(), 40);
}

#[test]
fn the_base_rupies_multiply_the_rupies_a_jar_drops() {
    let mut rows = blank_rows();
    assert_eq!(jar(json!(rows.clone())).rupie_value(), 1.0);
    rows[13][62] = 1.0;
    rows[13][65] = 1.0;
    rows[13][68] = 1.0;
    // One, two and four base rupies: `1 + 1 + 2 + 4`.
    assert_eq!(jar(json!(rows)).rupie_value(), 8.0);
}

#[test]
fn the_rupies_per_break_count_the_extra_chance_and_the_tier() {
    let mut rows = blank_rows();
    rows[13][64] = 1.0;
    rows[17][4] = 10.0;
    let jar = jar(json!(rows));
    // Mach IV is 50% and ten bell levels are 5%: 55% on top of the one rupie.
    assert!((jar.extra_rupie_chance() - 0.55).abs() < 1e-9);
    assert!((jar.avg_rupies_per_break() - 1.55).abs() < 1e-9);
    assert!((jar.rupies_per_break(0) - 1.55).abs() < 1e-9);
    // Every tier multiplies the payout by ten.
    assert!(
        (jar.rupies_per_break(INCOME_TIERS - 1) / jar.rupies_per_break(0)
            - 10_f64.powi(INCOME_TIERS as i32 - 1))
        .abs()
            < 1e-9
    );
    assert!(
        (jar.rupie_value_at_tier(VALUE_TIERS - 1) / jar.rupie_value()
            - 10_f64.powi(VALUE_TIERS as i32 - 1))
        .abs()
            < 1e-9
    );
}

#[test]
fn a_rupies_colors_escalate_at_the_upstream_thresholds() {
    let mut rows = blank_rows();
    assert_eq!(jar(json!(rows.clone())).rupie_colors(0), vec![0]);
    rows[9][20] = 100.0;
    assert_eq!(jar(json!(rows.clone())).rupie_colors(0), vec![0, 1]);
    rows[9][21] = 1000.0;
    assert_eq!(jar(json!(rows)).rupie_colors(0), vec![0, 1, 2]);

    assert_eq!(jar(json!([[]])).rupie_colors(5), vec![10]);
    assert_eq!(jar(json!([[]])).rupie_colors(8), vec![11]);
    assert_eq!(jar(json!([[]])).rupie_colors(9), vec![9]);
    // A chance jar pays no rupies, so its table row is never built.
    assert!(
        !jar(json!([[]]))
            .rupie_jars()
            .iter()
            .any(|(jar_type, _, _)| *jar_type == 1)
    );
}

#[test]
fn the_rupies_tree_branches_multiply_to_the_root() {
    let mut rows = blank_rows();
    rows[11][60] = 25.0;
    rows[13][80] = 1.0;
    rows[11][5] = 10.0;
    rows[21][7] = 40.0;
    rows[24][3] = 4.0;
    let jar = jar(json!(rows));
    let tree = jar.rupie_value_tree();
    assert_eq!(tree.format, idleon_common::formula::ValueFormat::Multiplier);
    assert!((tree.value - jar.rupie_value()).abs() < 1e-9);
    let product: f64 = tree.children.iter().map(|child| child.value).product();
    assert!((product - tree.value).abs() <= tree.value * 1e-12);
    assert!(tree.match_count("Gilded Jar") >= 1);
    assert!(tree.match_count("Collectible") >= 1);
}

#[test]
fn the_extra_rupies_list_only_the_sources_that_grant_them() {
    let empty = jar(json!([[]]));
    assert!(empty.extra_rupie_terms().is_empty());

    let mut rows = blank_rows();
    rows[4][3] = 4.0;
    rows[13][64] = 1.0;
    let terms = jar(json!(rows)).extra_rupie_terms();
    let labels: Vec<&str> = terms.iter().map(|term| term.label).collect();
    // Four Cosmo levels are `floor(35 × 4) = 140`, and Mach IV adds 50.
    assert_eq!(labels, vec!["Rupies Everywhere (Cosmo)", "Big Jar Mach IV"]);
    assert_eq!(terms[0].percent, 140.0);
}

#[test]
fn a_chance_grows_tenfold_per_tier_and_names_the_certain_tier() {
    assert_eq!(Jar::chance_at_tier(0.25, 0), 0.25);
    assert_eq!(Jar::chance_at_tier(0.25, 2), 25.0);
    // Half a percent is certain at tier four, where it exceeds one.
    assert_eq!(Jar::tier_for_certainty(0.005), Some(4));
    assert_eq!(Jar::tier_for_certainty(0.0), None);
    assert!(Jar::expected_breaks(0.0).is_infinite());
    assert_eq!(Jar::expected_breaks(2.0), 1.0);
    assert_eq!(Jar::expected_breaks(0.25), 4.0);
}

#[test]
fn the_collectible_chance_gets_harder_with_every_find() {
    let mut rows = blank_rows();
    let first = jar(json!(rows.clone()));
    rows[24][0] = 1.0;
    let second = jar(json!(rows.clone()));
    rows[24][1] = 1.0;
    let third = jar(json!(rows));
    assert_eq!(first.collectible_chance(), 0.25);
    assert!(second.collectible_chance() < first.collectible_chance());
    assert!(third.collectible_chance() < second.collectible_chance());
    // Collect 'Em All counts the digits of the ten standard balances.
    rows = vec![vec![0.0; 150]; 33];
    rows[24][0] = 1.0;
    rows[13][76] = 1.0;
    rows[9][20] = 1000.0;
    assert!(jar(json!(rows)).collectible_chance() > second.collectible_chance());
}

#[test]
fn the_enchant_curve_falls_with_every_level_and_the_multipliers_raise_it() {
    let base = Jar::enchant_chance_at(0, 1.0);
    assert!((base - 0.35 / (1.0 + 1.0)).abs() < 1e-12);
    assert!(Jar::enchant_chance_at(10, 1.0) < base);
    assert!(
        (Jar::enchant_chance_at(10, 3.0) - 3.0 * Jar::enchant_chance_at(10, 1.0)).abs() < 1e-12
    );

    let mut rows = blank_rows();
    rows[26][10] = 4.0;
    assert!((jar(json!(rows)).enchant_multi() - 2.0).abs() < 1e-9);
}

#[test]
fn the_bonuses_other_systems_grant_are_read_from_the_save() {
    let mut rows = blank_rows();
    rows[4][3] = 5.0;
    rows[17][4] = 10.0;
    rows[26][10] = 4.0;
    rows[15][21] = 100.0;
    let jar = jar_with(
        json!(rows),
        &[
            (
                "Spelunk",
                spelunk(&[(0, 8, 1.0), (9, 2, 25.0), (18, 10, 5.0), (18, 29, 5.0)]),
            ),
            (
                "Gaming",
                json!(["", "", "", "", "", "", "", "", "", "", "", "", "F"]),
            ),
        ],
    );
    assert_eq!(jar.ext.cosmo, 175.0);
    assert_eq!(jar.ext.bell, 5.0);
    assert_eq!(jar.ext.bolaia, 100.0);
    assert_eq!(jar.ext.legend_pts, 100.0);
    // Palette 2 is linear at 25 a level, raised by its legend talent and lore.
    let palette = 25.0 * 25.0 * 2.25 * 1.5;
    assert!((jar.ext.palette - palette).abs() < 1e-9);
    assert_eq!(jar.ext.lore_episodes, 3.0);
    assert_eq!(jar.ext.super_bit, 1.0);
    assert_eq!(jar.ext.monument, 100.0 * 3.0);
    // The two systems this app has not ported stay at the upstream default.
    assert_eq!(jar.ext.stamp_cavern_res, 0.0);
    assert_eq!(jar.ext.exotic, 0.0);
}

#[test]
fn the_chance_events_name_the_three_chance_jars() {
    let events = jar(json!([[]])).chance_events();
    let jar_types: Vec<usize> = events.iter().map(|event| event.jar_type).collect();
    assert_eq!(jar_types, vec![1, 2, 4]);
    assert_eq!(events[0].name, "Opal");
    assert_eq!(events[2].name, "Enchant");
}

#[test]
fn the_time_to_an_event_counts_the_jar_type_rate() {
    let mut rows = blank_rows();
    rows[13][69] = 1.0;
    let jar = jar(json!(rows));
    // One break at the shared rate, one chance in four: four jars' hours.
    let hours = jar.expected_hours(1, 0.25);
    let per_jar = jar.production_requirement(1) / jar.production_per_hour();
    assert!((hours - 4.0 * per_jar).abs() < 1e-9);
    assert!(jar.expected_hours(1, 0.0).is_infinite());
}

#[test]
fn the_upgrade_plan_only_buys_what_the_balance_covers() {
    let mut rows = blank_rows();
    rows[11][37] = 3.0;
    let cost = Jar::jar_type_cost(4);
    // Type four is paid with Purple, `Holes[9][23]`, the third standard rupie.
    rows[9][23] = cost - 1.0;
    let plan = jar(json!(rows.clone())).upgrade_plan();
    assert_eq!(plan.steps[0].status, super::UpgradeStatus::Owned);
    assert_eq!(plan.steps[2].status, super::UpgradeStatus::Owned);
    assert_eq!(plan.steps[3].status, super::UpgradeStatus::Shortfall);
    assert_eq!(plan.steps[4].status, super::UpgradeStatus::Locked);
    let next = plan.next().expect("the fourth type is next");
    assert_eq!(next.jar_type, 4);
    assert_eq!(next.name, "Enchant");
    assert_eq!(next.shortfall(), 1.0);
    // A shortfall spends nothing from the budget.
    assert_eq!(plan.spent(), 0.0);
    assert_eq!(plan.budget(), cost - 1.0);
    assert_eq!(plan.purchases().count(), 0);

    rows[9][23] = cost;
    let plan = jar(json!(rows)).upgrade_plan();
    assert_eq!(plan.steps[3].status, super::UpgradeStatus::Affordable);
    assert_eq!(plan.spent(), cost);
    assert!(plan.spent() <= plan.budget());
    assert_eq!(plan.purchases().count(), 1);
    // With every type bought the plan has nothing left to buy.
    let mut rows = blank_rows();
    rows[11][37] = 9.0;
    let plan = jar(json!(rows)).upgrade_plan();
    assert!(plan.next().is_none());
    assert_eq!(plan.spent(), 0.0);
}

#[test]
fn the_highest_reachable_tier_is_the_accumulation_peak() {
    let mut rows = blank_rows();
    // A hundred thousand Sapphire Droplet levels: a rate high enough to bank
    // progress past the first requirement on every tick.
    rows[24][1] = 100_000.0;
    rows[13][74] = 1.0;
    let jar = jar(json!(rows));
    let projection = jar.tier_projection(0);
    let sustained = projection.sustained.expect("the rate sustains a tier");
    assert!(projection.max_tier >= sustained);
    assert!(projection.seconds_between.is_some());
    assert!(jar.production_line_unlocked());

    let ranking = jar.milestone_ranking();
    assert_eq!(ranking.len(), jar.types_available());
    assert!(
        ranking
            .windows(2)
            .all(|pair| pair[0].hours <= pair[1].hours)
    );
    assert_eq!(ranking[0].benefits.is_some(), jar.types_available() > 1);
}

#[test]
fn a_collectible_effect_fills_its_placeholder() {
    assert_eq!(collectible_effect(0, 360.0), "+360.0% rupie value");
    assert_eq!(collectible_effect(3, 225.0), "3.25x rupie value");
    assert_eq!(collectible_effect(31, 9.0), "Future cavern");
    assert!(collectible_effect(99, 0.0).is_empty());
}
