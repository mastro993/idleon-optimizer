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

use idleon_import::ShadowVerdict;
use idleon_import::import;
use idleon_research::sim;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn imported() -> idleon_import::Imported {
    import(&fixture()).expect("the fixture imports")
}

/// The upstream term value, or the documented tolerance for the one omitted
/// tome slot.
fn assert_term(imported: &idleon_import::Imported, name: &str, expected: f64) {
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
    // `gridPointSources` adds Ring of Glory 3 and 13 to the flat pool.
    assert_eq!(context.rog(3), 2.0);
    assert_eq!(context.rog(13), 1.0);
    assert_eq!(context.bonus_grid_points, 18);
    assert_eq!(context.sailing_art37, 5.0);
    assert_eq!(context.task_grid_points, 1.0);
    assert!(context.bundle_ban_k);
    assert!(context.bundle_ban_m);
    assert_eq!(sim::shapes_owned(simulation.research_level, context), 8);
    assert_eq!(sim::magnifiers_owned(simulation, context), 17);
    assert_eq!(sim::grid_points(simulation, context).earned, 176);
    // `OptLacc[26]` is zero in the fixture, so the upstream page answers NO.
    assert_eq!(imported.shadow.verdict(), ShadowVerdict::No);

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

    // The dashboard reads the same rate, and its AFK terms come from this save.
    let afk = sim::afk_rate(simulation, context);
    assert!((context.companion_28 - 0.0).abs() < 1e-9);
    assert!((context.companion_153_level2 - 0.0).abs() < 1e-9);
    assert!((context.sailing_art36 - 5.0).abs() < 1e-9);
    assert!((context.card_w7b11 - 1.0).abs() < 1e-9);
    assert!((context.gem_shop_45 - 10.0).abs() < 1e-9);
    // Upstream reports 0.55362 with the Gambit Milestone's 3%; this build lists
    // that term as not modelled and leaves it out.
    assert!((afk.rate - 0.523_62).abs() < 1e-9, "{}", afk.rate);
    let summary = idleon_research::dashboard::summary(simulation, context);
    assert!((summary.active_rate - rate.total).abs() < 1e-6);
    assert!((summary.offline_rate / 7_496_009_394.671_702 - 1.0).abs() < 1e-4);
    assert!((summary.exp_required / 573_434_105_600_703_300.0 - 1.0).abs() < 1e-9);
    assert!((summary.hours_active / 21_867_825.609_115_824 - 1.0).abs() < 1e-4);
    assert!((summary.afk_rate - 0.523_62).abs() < 1e-9);
    assert_eq!(summary.mag_cap, 4);
    assert_eq!(summary.magnifiers, 17);

    let insight = idleon_research::dashboard::insight_multiplier(simulation, context);
    assert!((insight.grid_92 - 68.175).abs() < 1e-9);
    assert!((insight.grid_91 - 5.454).abs() < 1e-9);
    assert!((insight.emperor_pct - 35.0).abs() < 1e-9);
    assert!((insight.jelly_pct - 40.0).abs() < 1e-9);
    assert!((insight.multi - 3.281_588_1).abs() < 1e-9);
    assert!((insight.per_monocle - 9.844_764_3).abs() < 1e-9);

    let cards = idleon_research::dashboard::observations(simulation, context);
    assert_eq!(cards.len(), 43);
    let board_exp: f64 = cards.iter().map(|card| card.exp).sum();
    assert!((board_exp / rate.total - 1.0).abs() < 1e-9);
    let card = |obs: usize| cards.iter().find(|card| card.obs == obs).expect("a card");
    assert!((card(40).exp / 3_189_669_459.531_201 - 1.0).abs() < 1e-4);
    assert!((card(42).exp / 11_126_073_626.431_335 - 1.0).abs() < 1e-4);
    assert!((card(37).insight - 39.379_057_2).abs() < 1e-7);
    assert_eq!(card(37).monocles, 4);
    assert_eq!(card(34).kaleidoscopes, 4);

    let points = idleon_research::dashboard::grid_points(simulation, context);
    assert_eq!(points.every_ten, 36);
    assert_eq!(points.account_sources, 18);
    assert_eq!(points.points.earned, 176);
    assert_eq!(points.points.free, 6);

    // The Spelunking rows the section reads: saved stamina, unlocks and records.
    let spelunking = imported
        .spelunking
        .as_ref()
        .expect("the fixture carries the Spelunk rows");
    let spelunk = spelunking.inputs(&idleon_spelunking::Account::neutral());
    assert_eq!(spelunk.characters.len(), 11);
    assert_eq!(spelunk.characters[0].stamina, 3522.0);
    assert_eq!(spelunk.caves.len(), 10);
    assert_eq!(spelunk.tools.len(), 5);
    assert_eq!(spelunk.elixir_owned, 12);
    assert_eq!(spelunk.slots, 5);
    assert_eq!(spelunk.dupes, 5);
    assert_eq!(spelunk.memory_slots, 2);
    assert_eq!(spelunk.saved_elixirs, vec![6, 2, 0, 5, 3]);
    assert_eq!(spelunk.memorized, vec![7]);
    assert!(spelunk.manic && spelunk.saved_manic && spelunk.auto);
    assert!(spelunk.full_clear);
    assert_eq!(spelunk.speed_max, 4);
    assert_eq!(spelunk.depths[0], 338.0);
    assert_eq!(spelunk.manic_depths[0], 101.0);
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.starts_with("Spelunk")),
        "the fixture carries the Spelunk rows: {:?}",
        imported.warnings
    );

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

    // The Clamworks fields the save carries decode as the upstream module reads them.
    let clamworks = &imported.clamworks;
    assert_eq!(clamworks.pearl_balance, 2_960_058_875.172_09);
    assert_eq!(clamworks.promotion_level, 6);
    assert_eq!(clamworks.upgrades, [123, 12, 30, 43, 79, 33, 9, 0, 0]);
    assert_eq!(clamworks.unlocked, 6);
    assert_eq!(clamworks.scurge_level, 122);
    assert_eq!(clamworks.scurge_upgrades, [18, 2, 3, 18, 27, 54, 0, 0, 0]);
    let pearl_value = clamworks.pearl_value(1234.5);
    assert!((pearl_value - 379.647_08).abs() < 1e-3);

    // The Cglunko overview from the same `OptLacc` row, against the upstream
    // `cglunko.js` values for this save.
    let cglunko = &imported.cglunko;
    assert!(cglunko.saved);
    assert_eq!(
        cglunko.upgrades,
        [
            5, 26, 10, 17, 13, 3, 6, 5, 4, 2, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0
        ]
    );
    assert_eq!(cglunko.blue_stones, [3400.1, 959.1, 50.1, 11.0, 5.1, 1.1]);
    assert_eq!(cglunko.purple_stones, [0.0; 6]);
    assert_eq!(cglunko.blue_digits(), 13);
    assert_eq!(cglunko.purple_digits(), 0);
    assert_eq!(cglunko.kill_digits(), 9);
    assert_eq!(cglunko.normal_dr_digits(), 4);
    assert!((cglunko.dr() - 58.933_613_2).abs() < 1e-7);
    assert!((cglunko.afk_gains() - 0.14).abs() < 1e-12);
    assert_eq!(cglunko.mk_tier(), 6.0);
    assert_eq!(cglunko.mk_base(), 0.0);
    assert_eq!(cglunko.respawn(), 20.0);
    assert_eq!(cglunko.cavern_currency(), 0.0);
    assert_eq!(cglunko.research_exp(), 0.0);
    assert_eq!(cglunko.total_dmg(), 0.0);
    assert_eq!(cglunko.grand_discovery(), 0.0);
    assert!(
        !imported
            .warnings
            .iter()
            .any(|warning| warning.contains("Cglunko")),
        "the fixture carries the Cglunko rows: {:?}",
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
        idleon_farming::land_rank_requirement(300)
    );
    assert_eq!(first.growth_requirement, 25_200.0 * snapshot.growth_rate);
    assert_eq!(first.growth_seconds, 0.0);
    assert!(first.crop_value.capped);
    assert!(first.next_crop_chance > 0.0);
    assert!(first.next_overgrowth_chance > 0.0);
    assert!(first.farming_exp > 0.0);
}

#[test]
fn the_fixture_survives_a_project_export_and_import() {
    use idleon_project::{self as project, Document, Origin, Project};

    let imported = imported();
    let document = Project {
        version: project::VERSION,
        section: Some("research".into()),
        save: Some(serde_json::from_str(&fixture()).expect("the fixture is JSON")),
        simulation: imported.simulation.clone(),
        context: imported.context.clone(),
        sections: Default::default(),
        warnings: Vec::new(),
        origin: Origin::Project,
    };
    let text = project::write(&document);
    let Document::Project(back) = project::read(&text).expect("the export reads back") else {
        panic!("an export is a project document");
    };
    assert_eq!(back.version, project::VERSION);
    assert_eq!(back.origin, Origin::Project);
    assert_eq!(back.simulation, imported.simulation);
    assert_eq!(back.context, imported.context);
    assert!(back.warnings.is_empty(), "{:?}", back.warnings);

    // The embedded save still imports into the same run every section reads.
    let save = back.save.as_ref().expect("the project embeds the save");
    let reread = import(&save.to_string()).expect("the embedded save imports");
    assert_eq!(reread.simulation, imported.simulation);
    assert_eq!(reread.context, imported.context);
    let before = sim::exp_rate(&imported.simulation, &imported.context);
    let after = sim::exp_rate(&reread.simulation, &reread.context);
    assert_eq!(before.total, after.total);
}

#[test]
fn the_fixture_carries_the_ladle_roster() {
    // The ladle section reads the roster, the Cooking rows, the Post Office 19c
    // chance and the counts; upstream's `characterCount` and `itemCount` answer
    // the same from this save.
    let ladle = imported()
        .ladle
        .expect("the fixture carries a character roster");
    assert_eq!(ladle.characters.len(), 11);
    assert_eq!(ladle.characters[0].name, "Character 1");
    assert_eq!(ladle.characters[0].level, 142.0);
    assert_eq!(ladle.characters[0].exp, 369943639032.185);
    // No character is AFK-ing Cooking in the fixture, so no ladles are counted.
    assert!(ladle.cooking_characters().is_empty());
    assert!(ladle.characters[0].pending_hours.is_none());
    // `POu_<n>[19]` is 400 everywhere: decay(88, 200, 400 - 99.9) = 52.8.
    assert!(ladle.characters.iter().all(|row| row.double_chance == 52.8));
    // No prayer 2 is equipped, and its passive is not paid on the cost the cap
    // reads, so the ten-hour cap never applies here.
    assert!(!ladle.characters.iter().any(|row| row.claim_capped));
    assert_eq!(ladle.ladles.value, 0.0);
    assert!(!ladle.ladles.partial);
    assert_eq!(
        ladle
            .balloons
            .iter()
            .map(|count| count.value)
            .collect::<Vec<f64>>(),
        vec![585599.0, 50.0, 96187.0]
    );
    let reset = ladle
        .reset_hours
        .expect("TimeAway.ShopRestock is in the save");
    assert!((reset - 17.227732320030555).abs() < 1e-9, "{reset}");
}

#[test]
fn the_fixture_carries_the_kitchen_reset_state() {
    // The reset reads `Cooking` (unlocked kitchens, their saved meal and speed
    // level), `Meals` (levels and stocks), `Menagerie` (the Blue Jellyfish grant)
    // and the bundle that gives No Meal Left Behind its levels.
    let ladle = imported()
        .ladle
        .expect("the fixture carries a character roster");
    let kitchen = ladle.kitchen.expect("the fixture carries Cooking rows");
    let save = imported().save;
    let meals = save.get("Meals").expect("Meals is in the save");
    let levels = idleon_save::row_values(&idleon_save::at(&meals, 0).expect("Meals[0]").clone());
    assert_eq!(kitchen.meals.levels.len(), levels.len());
    assert_eq!(kitchen.meals.levels[0], 130.0);
    assert!(kitchen.meals.stocks[0] > 1e159);
    assert_eq!(kitchen.kitchens.len(), 10);
    assert!(kitchen.kitchens.iter().all(|row| row.unlocked));
    // The first kitchen cooks meal 68 with 628 speed levels.
    assert_eq!(kitchen.kitchens[0].saved_meal, 68);
    assert_eq!(kitchen.kitchens[0].speed_level, 628.0);
    // `Menagerie[0][5]` is the saved grant and rank 5 is owned.
    assert_eq!(kitchen.jelly.daily_hours, 1039999928.0);
    assert!(kitchen.jelly.available);
    assert!(kitchen.jelly.unlocked);
    // The bundle awards two No Meal Left Behind levels and emporium 16 (`N2L[16]`
    // is in the save's `Ninja[102][9]`) the third.
    assert_eq!(kitchen.nmlb, 3.0);
    assert!(kitchen.speed(1.0) > 0.0);
    assert_eq!(kitchen.unlocked().count(), 10);
}
