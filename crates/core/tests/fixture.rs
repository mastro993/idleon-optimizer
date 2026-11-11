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
    // The fixture's sailing fleet: 20 active boat and captain slots, Sailing
    // level 175 and the save's minimum chest tier.
    assert_eq!(imported.fleet.boats.len(), 20);
    assert_eq!(imported.fleet.captains.len(), 20);
    assert_eq!(imported.fleet.sailing_level, 175);
    assert_eq!(imported.fleet.artifact_tiers.len(), 60);
    assert_eq!(imported.fleet.available_islands(), 17);
    assert_eq!(imported.sailing.minimum_tier, 2);
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
