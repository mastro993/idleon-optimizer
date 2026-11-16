//! The committed synthetic save is the reference input for save-import work.
//!
//! It is test data only. The assertions below pin what the importer decodes
//! from it, so a change to either side is noticed deliberately.

use idleon_import::import;
use idleon_research::sim;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn imported() -> idleon_import::Imported {
    import(&fixture()).expect("the fixture imports")
}

#[test]
fn the_synthetic_account_imports_into_a_usable_run() {
    let imported = imported();
    let simulation = &imported.simulation;

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

    // The dashboard, the observation board and the grid-point breakdown read
    // the same imported run as the simulator, so their figures agree.
    let context = &imported.context;
    let rate = sim::exp_rate(simulation, context);
    assert!(rate.total > 0.0);
    let summary = idleon_research::dashboard::summary(simulation, context);
    assert!((summary.active_rate - rate.total).abs() < 1e-6);
    assert!((summary.afk_rate - sim::afk_rate(simulation, context).rate).abs() < 1e-12);
    assert_eq!(
        summary.magnifiers,
        sim::magnifiers_owned(simulation, context)
    );
    let cards = idleon_research::dashboard::observations(simulation, context);
    let board_exp: f64 = cards.iter().map(|card| card.exp).sum();
    assert!((board_exp / rate.total - 1.0).abs() < 1e-9);
    assert_eq!(
        idleon_research::dashboard::grid_points(simulation, context)
            .points
            .earned,
        sim::grid_points(simulation, context).earned
    );

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

    // The terms the save cannot supply are named instead of guessed. The star
    // talent tome slot now resolves, so its `SL_<char>` warning is gone.
    assert!(
        !imported
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

    // The Clamworks fields the save carries.
    let clamworks = &imported.clamworks;
    assert_eq!(clamworks.pearl_balance, 2_960_058_875.172_09);
    assert_eq!(clamworks.promotion_level, 6);
    assert_eq!(clamworks.upgrades, [123, 12, 30, 43, 79, 33, 9, 0, 0]);
    assert_eq!(clamworks.unlocked, 6);
    assert_eq!(clamworks.scurge_level, 122);
    assert_eq!(clamworks.scurge_upgrades, [18, 2, 3, 18, 27, 54, 0, 0, 0]);

    // The Cglunko overview from the same `OptLacc` row.
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
        "the fixture names the multisystems this build does not read: {:?}",
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
    use idleon_project::{self as project, Document, Project};

    let imported = imported();
    let document = Project {
        version: project::VERSION,
        section: Some("research".into()),
        save: Some(serde_json::from_str(&fixture()).expect("the fixture is JSON")),
        simulation: imported.simulation.clone(),
        context: imported.context.clone(),
        sections: Default::default(),
        warnings: Vec::new(),
        has_research: true,
        has_supplements: true,
    };
    let text = project::write(&document);
    let Document::Project(back) = project::read(&text).expect("the export reads back") else {
        panic!("an export is a project document");
    };
    assert_eq!(back.version, project::VERSION);
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
    // chance and the counts.
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
