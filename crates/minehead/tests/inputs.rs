//! The Minehead rows of the committed fixture save, decoded the way the tab
//! reads them. The values come from `Research[7]` (floor, currency, best hit)
//! and `Research[8]` (upgrade levels); the account terms come from the shared
//! decoder.

use idleon_minehead::inputs;
use idleon_save::Save;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn decoded() -> (idleon_minehead::Inputs, Vec<String>) {
    let imported = idleon_import::import(&fixture()).expect("the fixture imports");
    let parsed: serde_json::Value = serde_json::from_str(&fixture()).expect("JSON");
    let save = Save::new(parsed);
    inputs(&save, &imported.simulation)
}

#[test]
fn the_fixture_decodes_its_minehead_rows() {
    let (inputs, warnings) = decoded();
    assert_eq!(inputs.floor, 23);
    assert!((inputs.currency - 45_496_327.476_888_8).abs() < 1e-6);
    assert!((inputs.highest_dmg - 17_691_570_892_187.5).abs() < 1e-6);
    assert_eq!(inputs.levels[0], 123.0);
    assert_eq!(inputs.levels[2], 7.0);
    assert_eq!(inputs.levels[8], 4.0);
    assert!(inputs.levels.len() >= 30);
    assert_eq!(inputs.grid_dims(), (7, 4));
    assert!(warnings.iter().all(|warning| !warning.contains("Research")));
}

#[test]
fn the_dashboard_values_follow_the_save() {
    let (inputs, _) = decoded();
    assert!(inputs.boss_hp() > 0.0);
    assert!(inputs.lives() >= 3.0);
    assert!((1.0..=40.0).contains(&inputs.mines()));
    assert!(inputs.daily_tries() >= 3.0);
    assert!(inputs.currency_per_hour() > 0.0);
    assert!(inputs.base_dmg() > 0.0);
    assert!(inputs.per_tile_pct() >= 0.0);
}

#[test]
fn an_empty_save_warns_instead_of_guessing() {
    let save = Save::new(serde_json::json!({}));
    let (inputs, warnings) = inputs(&save, &idleon_research::model::Simulation::default());
    assert_eq!(inputs.floor, 0);
    assert_eq!(inputs.levels.len(), 0);
    assert!(warnings.iter().any(|warning| warning.contains("Research")));
}
