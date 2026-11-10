//! The committed synthetic save is the reference input for save-import work.
//!
//! It is test data only: the decoding rules come from the upstream
//! `idleon-research-optimizer` repo, not from this file. The assertions below
//! pin the fixture so a change to either side is noticed deliberately.

use idleon_core::save::{ShadowVerdict, import};
use idleon_core::sim;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

#[test]
fn the_synthetic_account_imports_into_a_usable_run() {
    let imported = import(&fixture()).expect("the fixture imports");
    let simulation = &imported.simulation;
    let context = &imported.context;

    assert_eq!(simulation.research_level, 122);
    assert!(simulation.research_exp > 0.0);
    assert_eq!(simulation.grid.live_cells().count(), 62);
    assert_eq!(simulation.mags.len(), 80);
    assert_eq!(simulation.placements.len(), 10);
    assert_eq!(
        simulation
            .observations
            .found
            .iter()
            .filter(|found| **found)
            .count(),
        31
    );

    // The account fields the save does carry are read; the ones it cannot carry
    // are warned about rather than guessed.
    assert!((context.extra_multiplier - 5.771_131_034_482_759).abs() < 1e-9);
    assert_eq!(context.bonus_grid_points, 16);
    // `OptLacc[26]` is zero in the fixture, so the upstream page answers NO.
    assert_eq!(imported.shadow.verdict(), ShadowVerdict::No);
    for field in ["serverVarResXP", "allBonusMulti", "extra_pct"] {
        assert!(
            imported
                .warnings
                .iter()
                .any(|warning| warning.contains(field)),
            "warnings name {field}: {:?}",
            imported.warnings
        );
    }
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("gridLevels")),
        "the fixture carries grid levels: {:?}",
        imported.warnings
    );
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("observations")),
        "the fixture carries observations: {:?}",
        imported.warnings
    );

    let rate = sim::exp_rate(simulation, context);
    assert!(rate.total.is_finite() && rate.total > 0.0);
    assert!(rate.multi > 1.0);

    // The Clamworks fields the save carries decode as the upstream module reads them.
    let clamworks = &imported.clamworks;
    assert_eq!(clamworks.pearl_balance, 2_960_058_875.172_09);
    assert_eq!(clamworks.promotion_level, 6);
    assert_eq!(clamworks.upgrades, [123, 12, 30, 43, 79, 33, 9, 0, 0]);
    assert_eq!(clamworks.scurge_level, 122);
    assert_eq!(clamworks.scurge_upgrades, [18, 2, 3, 18, 27, 54, 0, 0, 0]);
    let pearl_value = clamworks.pearl_value(1234.5);
    assert!((pearl_value - 379.647_08).abs() < 1e-3);
}

#[test]
fn importing_the_fixture_twice_gives_the_same_rate() {
    let once = import(&fixture()).expect("the fixture imports");
    let twice = import(&fixture()).expect("the fixture imports again");
    let first = sim::exp_rate(&once.simulation, &once.context);
    let second = sim::exp_rate(&twice.simulation, &twice.context);
    assert_eq!(first.total, second.total);
    assert_eq!(first.multi, second.multi);
}

#[test]
fn the_fixture_farming_state_imports_into_the_field() {
    let imported = import(&fixture()).expect("the fixture imports");
    let farm = &imported.farming;

    assert_eq!(farm.plots.len(), 36);
    assert_eq!(farm.farm_level, 486);
    assert_eq!(farm.crop_count, 238);
    assert_eq!(farm.crop_quantities.len(), 238);
    assert_eq!(farm.market_levels.len(), 100);
    assert_eq!(farm.rank_ranks[0], 300);
    assert_eq!(farm.rank_levels[0], 250);
    assert_eq!(farm.total_land_rank(), 10_812);
    assert!(farm.organic_exotics.is_empty());

    for plot in &farm.plots {
        assert_eq!(plot.seed_family, 6);
    }
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.contains("farming multipliers")),
        "the fixture names the multisystems this port does not read: {:?}",
        imported.warnings
    );
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("Farm")),
        "the fixture carries every farm field: {:?}",
        imported.warnings
    );

    let snapshot = farm.snapshot();
    let first = farm.plot_metrics_with(0, &snapshot);
    assert_eq!(first.crop_id, 231);
    assert_eq!(first.evolution, 1);
    assert_eq!(first.rank, 300);
    assert_eq!(
        first.rank_requirement,
        idleon_core::farming::land_rank_requirement(300)
    );
    assert_eq!(first.growth_requirement, 25_200.0 * snapshot.growth_rate);
    assert_eq!(first.growth_seconds, 0.0);
    assert!(first.crop_value.capped);
    assert!(first.next_crop_chance > 0.0);
    assert!(first.next_overgrowth_chance > 0.0);
    assert!(first.farming_exp > 0.0);
}
