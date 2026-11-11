//! The coral god rank planner's save inputs, pinned to the committed fixture.

use idleon_core::coral::{Goal, Inputs, upgrade_slot};
use idleon_core::save::import;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

#[test]
fn the_fixture_fills_every_save_derived_planner_input() {
    let imported = import(&fixture()).expect("the fixture imports");
    let coral = &imported.coral;

    assert_eq!(coral.budget, Some(2.909_587_276_919_98e18));
    assert_eq!(coral.god_rank, Some(31.0));
    assert_eq!(coral.tier, Some(1.0));
    assert_eq!(coral.levels, [154, 64, 124, 98, 100, 90].map(Some));
    assert_eq!(coral.singularity_levels, [49, 31, 0, 0, 0, 0].map(Some));
    assert_eq!(coral.reef_upgrade_count, Some(53.0));
    assert_eq!(coral.mine_floor, Some(23.0));
    assert_eq!(coral.vault76_bonus, Some(40.0));
    assert_eq!(coral.y4, None, "the fixture's Y4 is Prisma-boosted");
    assert_eq!(coral.div_cost_after3, None, "the fixture has no serverVars");
    assert_eq!(coral.total_divinity_levels, Some(4727));
    assert!(
        coral
            .missing
            .iter()
            .any(|field| field.contains("Y4 is Prisma-boosted")),
        "{:?}",
        coral.missing
    );
    assert!(
        coral
            .missing
            .iter()
            .any(|field| field.contains("DivCostAfter3")),
        "{:?}",
        coral.missing
    );
    assert!(
        !coral
            .missing
            .iter()
            .any(|field| field.contains("Coral Kid")),
        "the fixture carries Coral Kid levels: {:?}",
        coral.missing
    );
}

#[test]
fn the_summary_matches_the_fixture_save() {
    let imported = import(&fixture()).expect("the fixture imports");
    let inputs = Inputs::from_save(&imported.coral);
    let metrics = inputs.metrics();

    assert_eq!(inputs.god_rank, 31);
    assert_eq!(inputs.levels, [154, 64, 124, 98, 100, 90]);
    assert_eq!(metrics.divinity_points, 5350.0);
    assert_eq!(metrics.divinity_exp, 16.4);
    assert_eq!(metrics.blessing_cap, 348.0);
    assert!((metrics.class_exp - 25.359_084_901_759_285).abs() < 1e-12);
    assert!((metrics.daily_reef_coral - 1.692_307_692_307_692).abs() < 1e-12);
    assert!(inputs.validation().ok(), "{:?}", inputs.validation());

    let rows = inputs.level_rows();
    assert_eq!(rows[0].level, 154);
    assert_eq!(rows[0].effect, 1540.0);
    assert!((rows[0].next_cost - 23_685_006_833_599.492).abs() < 1.0);
}

#[test]
fn every_planner_input_has_a_default() {
    let inputs = Inputs::from_save(&Default::default());
    assert_eq!(inputs.goal, Goal::Balanced);
    assert_eq!(inputs.max_levels, 25);
    assert_eq!(inputs.beam_size, 100);
    assert_eq!(inputs.planning_confidence, 0.9);
    assert!(inputs.assume_unlocked);
    assert_eq!(inputs.blessing_base, 100.0);
}

#[test]
fn the_save_slots_are_the_upstream_ones() {
    assert_eq!(
        (0..12).map(upgrade_slot).collect::<Vec<_>>(),
        [427, 428, 429, 430, 431, 432, 616, 617, 618, 619, 620, 621]
    );
}
