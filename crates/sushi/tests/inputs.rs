//! The Sushi rows of the committed fixture save, decoded the way the tab reads
//! them: `Sushi` rows 0-7 plus the account terms and the research level.

use idleon_save::Save;
use idleon_sushi::inputs;

fn fixture() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn decoded() -> (idleon_sushi::Inputs, Vec<String>) {
    let imported = idleon_import::import(&fixture()).expect("the fixture imports");
    let parsed: serde_json::Value = serde_json::from_str(&fixture()).expect("JSON");
    let save = Save::new(parsed);
    inputs(&save, &imported.simulation)
}

#[test]
fn the_fixture_decodes_its_sushi_rows() {
    let (inputs, warnings) = decoded();
    let station = &inputs.station;
    assert_eq!(inputs.research_level, 122);
    assert_eq!(station.unique, 58);
    assert_eq!(station.tiers.len(), 120);
    assert_eq!(station.types.len(), 120);
    assert!(station.levels.len() >= 46);
    assert_eq!(station.fires.len(), 15);
    assert!(station.misc.len() > 8);
    assert_eq!(station.found.len(), 100);
    assert_eq!(station.knowledge.len(), 100);
    assert_eq!(station.active_slots(), 7);
    assert_eq!(station.slots_owned(), 60.0);
    assert!(warnings.iter().all(|warning| !warning.contains("Sushi")));
}

#[test]
fn the_station_renders_its_rates_from_the_save() {
    let (inputs, _) = decoded();
    let station = &inputs.station;
    let levels = &station.levels;
    assert!(station.bucks() > 0.0);
    assert!(station.bucks_per_hour(levels, &inputs.external) > 0.0);
    assert!(station.fuel_gen(levels, &inputs.external) > 0.0);
    assert!(station.fuel_capacity(levels, &inputs.external) > 200.0);
    assert!(station.currency_multi(levels, &inputs.external) > 1.0);
    assert_eq!(station.sparks(), 4_281_325.134_503_62);
    assert_eq!(station.combo_score(), 6_753.0);
}

#[test]
fn a_save_without_sushi_warns_and_renders_empty() {
    let save = Save::new(serde_json::json!({ "Research": "[]" }));
    let (inputs, warnings) = inputs(&save, &idleon_research::model::Simulation::default());
    assert_eq!(inputs.station.tiers, Vec::<f64>::new());
    assert_eq!(inputs.station.unique, 0);
    assert!(
        warnings.iter().any(|warning| warning.starts_with("Sushi:")),
        "{warnings:?}"
    );
}
