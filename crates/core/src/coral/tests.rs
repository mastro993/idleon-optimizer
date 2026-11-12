//! The coral formulas and planner inputs against the upstream JS values.

use super::*;

fn fixture_inputs() -> Inputs {
    Inputs {
        budget: 2.909_587_276_919_98e18,
        god_rank: 31,
        reef_upgrade_count: 53.0,
        mine_floor: 23.0,
        vault76_bonus: 40.0,
        levels: [154, 64, 124, 98, 100, 90],
        singularity_levels: [49, 31, 0, 0, 0, 0],
        ..Inputs::default()
    }
}

#[test]
fn upgrade_values_match_upstream_at_the_fixture_levels() {
    let expected = [
        (1540.0, 23685006833599.492),
        (128.0, 95585514667947.14),
        (16.644_295_302_013_422, 48864725156227.086),
        (98.0, 24600101366955.652),
        (200.0, 178596735924098.06),
        (69.230_769_230_769_23, 413140638297324.75),
    ];
    for (index, (effect, cost)) in expected.into_iter().enumerate() {
        let level = f64::from(fixture_inputs().levels[index]);
        assert!((upgrade_effect(index, level) - effect).abs() < 1e-9);
        assert!((upgrade_cost(index, level) - cost).abs() < 1.0);
    }
}

#[test]
fn metrics_match_upstream_for_the_fixture_levels() {
    let metrics = fixture_inputs().metrics();
    assert_eq!(metrics.divinity_exp, 16.4);
    assert_eq!(metrics.divinity_points, 5350.0);
    assert_eq!(metrics.blessing_cap, 348.0);
    assert_eq!(metrics.minor_links, 1.98);
    assert!((metrics.class_exp - 25.359_084_901_759_285).abs() < 1e-12);
    assert!((metrics.daily_reef_coral - 1.692_307_692_307_692).abs() < 1e-12);
}

#[test]
fn the_singularity_upgrades_scale_points_and_exp() {
    let inputs = Inputs {
        singularity_levels: [0, 0, 0, 0, 0, 0],
        ..fixture_inputs()
    };
    let metrics = inputs.metrics();
    assert_eq!(metrics.divinity_points, 107.0);
    assert_eq!(metrics.divinity_exp, 16.4);
}

#[test]
fn slots_follow_the_upgrade_page() {
    assert_eq!(upgrade_slot(0), 427);
    assert_eq!(upgrade_slot(5), 432);
    assert_eq!(upgrade_slot(6), 616);
    assert_eq!(upgrade_slot(11), 621);
}

#[test]
fn unlock_levels_decide_when_the_save_knows_them() {
    let inputs = Inputs {
        assume_unlocked: false,
        total_divinity_levels: Some(1600),
        ..Inputs::default()
    };
    assert_eq!(inputs.unlocked_count(), 5);
    assert!(!inputs.upgrade_unlocked(5));
    assert!(inputs.upgrade_unlocked(4));

    let all = Inputs {
        total_divinity_levels: Some(4727),
        ..inputs
    };
    assert_eq!(all.unlocked_count(), 6);
}

#[test]
fn assume_unlocked_wins_over_the_save() {
    let inputs = Inputs {
        assume_unlocked: true,
        total_divinity_levels: Some(0),
        ..Inputs::default()
    };
    assert_eq!(inputs.unlocked_count(), 6);
}

#[test]
fn validation_rejects_impossible_inputs() {
    let inputs = Inputs {
        budget: -1.0,
        beam_size: 0,
        assume_unlocked: false,
        total_divinity_levels: None,
        ..Inputs::default()
    };
    let validation = inputs.validation();
    assert!(!validation.ok());
    assert!(
        validation
            .errors
            .iter()
            .any(|error| error.contains("Budget"))
    );
    assert!(
        validation
            .errors
            .iter()
            .any(|error| error.contains("Beam size"))
    );
    assert!(
        validation
            .errors
            .iter()
            .any(|error| error.contains("unlock state"))
    );
}

#[test]
fn validation_accepts_the_fixture_defaults() {
    let validation = fixture_inputs().validation();
    assert!(validation.ok(), "{validation:?}");
}

#[test]
fn a_budget_below_the_cheapest_upgrade_is_a_warning() {
    let inputs = Inputs {
        budget: 1.0,
        ..fixture_inputs()
    };
    let validation = inputs.validation();
    assert!(validation.ok());
    assert!(
        validation
            .warnings
            .iter()
            .any(|warning| warning.contains("Budget buys no"))
    );
}
