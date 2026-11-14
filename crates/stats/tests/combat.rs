//! The save-derived progress config against upstream's own inputs.
//!
//! `fixtures/combat-progress.json`'s `account` block is
//! `combatProgressInputs` on the committed synthetic save. The decoder here
//! reads the same save through the account stats crate, so every term that
//! feeds the panel is compared with the upstream module's own value.

use idleon_save::{Save, json};
use idleon_stats::combat;
use serde_json::Value;

fn save() -> Save {
    let path = format!("{}/../../fixtures/data.json", env!("CARGO_MANIFEST_DIR"));
    let text = std::fs::read_to_string(path).expect("the synthetic save is committed");
    Save::new(serde_json::from_str(&text).expect("save JSON"))
}

fn fixture() -> Value {
    let path = format!(
        "{}/../../fixtures/combat-progress.json",
        env!("CARGO_MANIFEST_DIR")
    );
    serde_json::from_str(&std::fs::read_to_string(path).expect("the progress fixture is committed"))
        .expect("fixture JSON")
}

fn number(value: &Value) -> f64 {
    value.as_f64().unwrap_or(0.0)
}

fn close(got: f64, want: f64, what: &str) {
    assert!(
        (got - want).abs() <= 1e-9 * want.abs().max(1.0),
        "{what}: {got} != {want}"
    );
}

#[test]
fn the_forge_config_matches_upstream_inputs() {
    let save = save();
    let fixture = fixture();
    let account = &fixture["account"]["forge"];
    let (config, _) = combat::progress(&save, 0);
    let forge = &config.forge;
    assert_eq!(
        account["available"].as_bool().unwrap_or(false),
        forge.available
    );
    assert_eq!(
        account["storageAvailable"].as_bool().unwrap_or(false),
        forge.storage_available
    );
    assert_eq!(account["refill"].as_bool().unwrap_or(false), forge.refill);
    close(number(&account["slots"]), forge.slots, "slots");
    close(number(&account["gemSlots"]), forge.gem_slots, "gem slots");
    close(number(&account["cap"]), forge.cap, "cap");
    close(number(&account["speed"]), forge.speed, "speed");
    close(number(&account["bars"]), forge.bars, "bars");
    close(number(&account["oilUse"]), forge.oil_use, "oil use");
    let order: Vec<String> = account["order"]
        .as_array()
        .expect("order")
        .iter()
        .map(|item| item.as_str().expect("an item").to_string())
        .collect();
    assert_eq!(order, forge.order);
    let qty: Vec<f64> = account["qty"]
        .as_array()
        .expect("qty")
        .iter()
        .map(number)
        .collect();
    assert_eq!(qty, forge.qty);
    let progress: Vec<f64> = account["progress"]
        .as_array()
        .expect("progress")
        .iter()
        .map(number)
        .collect();
    assert_eq!(progress, forge.progress);
    let storage: std::collections::BTreeMap<String, f64> = account["storage"]
        .as_object()
        .expect("storage")
        .iter()
        .map(|(key, value)| (key.clone(), number(value)))
        .filter(|(_, qty)| *qty != 0.0)
        .collect();
    let ours: std::collections::BTreeMap<String, f64> = forge
        .storage
        .iter()
        .filter(|(_, qty)| **qty != 0.0)
        .map(|(key, qty)| (key.clone(), *qty))
        .collect();
    assert_eq!(storage, ours);
}

#[test]
fn the_pet_scurge_coral_and_omega_config_matches_upstream_inputs() {
    let save = save();
    let fixture = fixture();
    let account = &fixture["account"];
    let (config, warnings) = combat::progress(&save, 0);
    assert_eq!(
        account["pets"]["available"].as_bool().unwrap_or(false),
        config.pets.available
    );
    close(
        number(&account["pets"]["shinySpeed"]),
        config.pets.shiny_speed,
        "shiny speed",
    );
    assert_eq!(
        account["scurge"]["unlocked"].as_bool().unwrap_or(false),
        config.scurge.unlocked
    );
    let menagerie: Vec<Vec<f64>> = account["scurge"]["menagerie"]
        .as_array()
        .expect("menagerie")
        .iter()
        .map(|row| row.as_array().expect("a row").iter().map(number).collect())
        .collect();
    assert_eq!(menagerie.len(), config.scurge.menagerie.len());
    for (want, got) in menagerie.iter().zip(&config.scurge.menagerie) {
        assert_eq!(want.len(), got.len());
        for (want, got) in want.iter().zip(got) {
            close(*want, *got, "menagerie cell");
        }
    }
    assert_eq!(
        account["coral"]["unlocked"].as_bool().unwrap_or(false),
        config.coral.unlocked
    );
    close(
        number(&account["coral"]["kills"]),
        config.coral.kills,
        "coral kills",
    );
    assert_eq!(
        account["omega"]["unlocked"].as_bool().unwrap_or(false),
        config.omega.unlocked
    );
    assert_eq!(
        account["omega"]["available"].as_bool().unwrap_or(false),
        config.omega.available
    );
    close(
        number(&account["omega"]["balls"]),
        config.omega.balls,
        "balls",
    );
    close(
        number(&account["omega"]["drops"]),
        config.omega.drops,
        "drops",
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("Star sign 57b")),
        "the missing star-sign term is named: {warnings:?}"
    );
    assert!(
        warnings
            .iter()
            .any(|warning| warning.starts_with("Per-kill")),
        "the missing talent grants are named: {warnings:?}"
    );
}

#[test]
fn the_den_config_reads_the_saved_holes_rows() {
    let save = save();
    let config = combat::den(&save, 0).expect("the fixture save has a cavern pick");
    let holes = save.get("Holes").expect("Holes");
    let at = |path: &[usize]| {
        json::index(&holes, path)
            .and_then(json::number)
            .unwrap_or(0.0)
    };
    for (index, level) in config.amps.iter().enumerate() {
        assert_eq!(at(&[12, index]), *level, "amplifier {index}");
    }
    assert_eq!(1.0, config.b_upg23);
    assert_eq!(at(&[26, 2]), config.bolaia_level);
    assert_eq!(at(&[11, 8]), config.best_score);
    assert_eq!(at(&[7, 2]), config.opal_level);
    assert_eq!(idleon_combat::den::hound_hp(), config.hound_hp);
}
