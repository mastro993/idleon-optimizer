//! The committed synthetic save is the reference input for save-import work.
//!
//! It is test data only: the decoding rules come from the upstream
//! `idleon-research-optimizer` repo, not from this file. The assertions below
//! pin the fixture so a change to either side is noticed deliberately.
//!
//! Expected values come from upstream by running its loader and save context
//! against this save (`js/save/context.js`, `js/stats/defs/research-exp.js`,
//! `js/stats/systems/w7/research-math.js`). The only term left out is the Tome
//! star talent slot, which needs the talent system; it is warned about and
//! shifts the rate by 2e-5 relative, inside the documented tolerance.

use idleon_core::save::import;
use idleon_core::sim;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn imported() -> idleon_core::save::Imported {
    import(&fixture()).expect("the fixture imports")
}

/// The upstream term value, or the documented tolerance for the one omitted
/// tome slot.
fn assert_term(imported: &idleon_core::save::Imported, name: &str, expected: f64) {
    let term = imported
        .tree
        .term(name)
        .unwrap_or_else(|| panic!("the tree carries {name}"));
    let tolerance = if name == "Tome Bonus" { 0.1 } else { 1e-9 };
    assert!(
        (term.value - expected).abs() <= tolerance,
        "{name}: got {}, upstream {expected}",
        term.value
    );
    assert!(!term.is_omitted(), "{name} is not omitted");
}

#[test]
fn the_synthetic_account_imports_into_a_usable_run() {
    let imported = imported();
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

    // The save context terms upstream derives from this save.
    assert_eq!(context.server_var_res_xp, 1.01);
    assert!((context.all_bonus_multi - 1.01).abs() < 1e-9);
    assert!((context.companion_52 - 1.0).abs() < 1e-9);
    assert!((context.sticker_fixed - 75.0).abs() < 1e-9);
    assert!((context.boony_count - 17.0).abs() < 1e-9);
    assert!((context.event_shop_37 - 1.0).abs() < 1e-9);
    assert!((context.button0 - 59.46875).abs() < 1e-9);
    assert!((context.button_base_no_grid - 50.0).abs() < 1e-9);
    assert!((context.killroy5 - 1.208_866_995_073_891_7).abs() < 1e-9);
    assert!((context.dream14 - 39.0).abs() < 1e-9);
    assert!((context.cglunko11 - 0.0).abs() < 1e-9);
    assert!((context.fountain_2_16 - 0.0).abs() < 1e-9);
    assert!((context.royal_research - 2.090_909_090_909_091).abs() < 1e-9);
    assert!(!context.sticker_multiplicative);
    assert!((context.arcade63 - 10.0).abs() < 1e-9);
    assert!((context.clam_work9 - 0.0).abs() < 1e-9);
    assert!((context.menagerie_b3 - 220.0).abs() < 1e-9);
    assert!((context.research_equipment_multi - 4.941_465_831_401_585).abs() < 1e-9);
    assert!((context.jelly_research_multi - 2.1).abs() < 1e-9);
    assert!((context.jelly_insight_pct - 40.0).abs() < 1e-9);
    assert!((context.jelly_kalei_pct - 1.0).abs() < 1e-9);
    assert_eq!(context.rog(0), 100.0);
    assert_eq!(context.rog(53), 1.0);
    assert_eq!(context.rog(54), 1.0);
    assert!((context.mastery_additive_pct - 25.0).abs() < 1e-9);
    assert!((context.mastery_exp_multi - 1.1).abs() < 1e-9);
    assert_eq!(context.bonus_grid_points, 18);
    assert_eq!(context.sailing_art37, 5.0);
    assert_eq!(context.task_grid_points, 1.0);
    assert!(context.bundle_ban_k);
    assert!(context.bundle_ban_m);
    assert_eq!(sim::shapes_owned(simulation.research_level, context), 8);
    assert_eq!(sim::magnifiers_owned(simulation, context), 17);
    assert_eq!(sim::grid_points(simulation, context).earned, 176);

    // The additive tree, in upstream order.
    for (name, value) in [
        ("Sticker Bonus", 113.596_875_000_000_01),
        ("Dancing Coral", 360.0),
        ("Zenith Market", 30.0),
        ("Trench Fish Card", 3.0),
        ("Eggroll Card", 4.0),
        ("Coralcave Crab Card", 1.0),
        ("Seaweed Serpent Card", 3.0),
        ("Paper Hardhat", 20.0),
        ("Prehistoric Set", 50.0),
        ("Slab Bonus", 28.137_226_000_000_002),
        ("Meal (Giga Chip)", 33.189_826_214_4),
        ("Crop Scientist", 12.995_463_569_682_151),
        ("MSA Bonus", 26.099_999_999_999_998),
        ("Tome Bonus", 29.683_700_308_098_57),
        ("Grid: Pts Every Ten", 6.3125),
        ("Grid: Sharp Eye", 25.25),
        ("Grid: Observationalistic", 5.454_000_000_000_001),
        ("Grid: All Night Studying", 136.35),
        ("Grid: Smart Eye", 18.9375),
        ("Grid: See 'Em All", 211.342_5),
        ("Grid: Game Design 102", 1405.92),
        ("Research Skill Mastery", 25.0),
    ] {
        assert_term(&imported, name, value);
    }
    // The pet arena crucibles are not reached on this save.
    assert_term(&imported, "Pet Arena Crucible", 0.0);

    // The true multipliers, in upstream order.
    for (name, value) in [
        ("Grid: Takin' Notes", 1.654_48),
        ("Equipment Research EXP", 4.941_465_831_401_585),
        ("Research Skill Mastery multiplier", 1.1),
        ("ClamWork Promotion 9", 1.0),
        ("Nonstop Studies", 2.17),
        ("Button: Monster EXP", 1.594_687_5),
        ("Arcade: x Research XP", 1.1),
        ("Killroy: Research EXP", 1.208_866_995_073_891_7),
        ("Companion: Jellofish (1.5x)", 1.0),
        ("Companion: Rift Stalker (2x / 3x enhanced)", 1.0),
        ("Companion: Neonscale (enhanced)", 1.0),
        ("Ring of Glory: multiplier to Research EXP gain", 2.0),
        ("Menagerie Boost 3", 3.2),
        ("Royal Guardian: Research Focus", 2.090_909_090_909_091),
    ] {
        assert_term(&imported, name, value);
    }
    let mut jelly: Vec<f64> = imported
        .tree
        .multipliers
        .iter()
        .filter(|term| term.name == "Jelly Reward")
        .map(|term| term.value)
        .collect();
    jelly.sort_by(f64::total_cmp);
    assert_eq!(jelly, vec![1.0, 1.0, 1.4, 1.5]);
    assert!((imported.tree.additive_pct - 2_549.269_591_092_180_7).abs() < 0.1);
    assert!((imported.tree.multiplier - 1_162.922_980_120_347_6).abs() < 1e-6);
    // Every grid/sticker/mastery term is taken back out for the simulator.
    assert!((context.extra_pct - 601.106_216_092_180_4).abs() < 0.1);

    let rate = sim::exp_rate(simulation, context);
    assert!((rate.obs_base - 464_661.605_531_104_6).abs() < 1e-6);
    assert!((rate.total / 14_315_743_085.962_536 - 1.0).abs() < 1e-4);

    // The terms the save cannot supply are named instead of guessed.
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("SL_<char>")),
        "{:?}",
        imported.warnings
    );
    assert!(
        imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("PVStatList_<char>")),
        "{:?}",
        imported.warnings
    );
    assert!(
        imported
            .warnings
            .iter()
            .all(|warning| warning.contains(':')),
        "every warning names its field: {:?}",
        imported.warnings
    );
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("gridLevels") || warning.contains("observations")),
        "the fixture carries the research fields: {:?}",
        imported.warnings
    );
}

#[test]
fn importing_the_fixture_twice_gives_the_same_rate() {
    let once = imported();
    let twice = imported();
    let first = sim::exp_rate(&once.simulation, &once.context);
    let second = sim::exp_rate(&twice.simulation, &twice.context);
    assert_eq!(first.total, second.total);
    assert_eq!(first.multi, second.multi);
    assert_eq!(once.context, twice.context);
    assert_eq!(once.tree, twice.tree);
}
