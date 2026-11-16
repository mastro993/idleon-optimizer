//! The Sushi Station formulas: the Ring of Glory table, the Menagerie doublers
//! and an empty station.

use idleon_save::Save;
use idleon_sushi::data::game;
use idleon_sushi::math::{self, External, Station};
use idleon_sushi::{Inputs, inputs};
use serde_json::Value;

fn fixture_text() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    std::fs::read_to_string(path).expect("fixtures/data.json is committed")
}

fn decoded() -> Inputs {
    let imported = idleon_import::import(&fixture_text()).expect("the fixture imports");
    let parsed: Value = serde_json::from_str(&fixture_text()).expect("JSON");
    let save = Save::new(parsed);
    inputs(&save, &imported.simulation).0
}

#[test]
fn the_ring_of_glory_table_follows_the_unique_tiers() {
    let unique = decoded().station.unique;
    let station = Station {
        unique,
        ..Station::default()
    };
    let data = game();
    let rog = math::build_rog_array(data, unique);
    assert_eq!(rog.len(), data.rog_qty.len());
    assert!(rog[0] > 0.0, "the first tier is unlocked");
    assert_eq!(rog[unique as usize], 0.0, "the next tier is locked");
    assert_eq!(station.rog(53), rog[53]);
    assert_eq!(station.rog(unique as usize), 0.0);
    // A save without the tier unlocks nothing.
    assert_eq!(
        math::build_rog_array(data, 0),
        vec![0.0; data.rog_qty.len()]
    );
}

#[test]
fn the_menagerie_doubles_upgrades_but_not_slots_or_cook_tiers() {
    let mut station = decoded().station;
    station.doublers.clear();
    let plain = station.clone();
    assert!(
        (0..plain.levels.len()).any(|upg| plain.qty(upg) > 0.0),
        "the fixture holds upgrade levels"
    );
    station.doublers = (0..station.levels.len()).collect();
    for upg in 0..station.levels.len() {
        assert_eq!(station.qty(upg), 2.0 * plain.qty(upg), "upgrade {upg}");
    }
    assert_eq!(station.slots_owned(), plain.slots_owned());
    assert_eq!(station.max_cook_tier(), plain.max_cook_tier());
}

#[test]
fn a_station_without_rows_reads_as_empty() {
    let save = Save::new(serde_json::json!({}));
    let (inputs, warnings) = inputs(&save, &idleon_research::model::Simulation::default());
    assert_eq!(inputs.station, Station::default());
    assert_eq!(inputs.external, External::default());
    assert!(warnings.iter().any(|warning| warning.contains("Sushi")));
    assert_eq!(inputs.station.bucks_per_hour(&[], &inputs.external), 0.0);
}
