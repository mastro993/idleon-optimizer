//! The combat progress and smelting rules against the upstream module's output.
//!
//! `fixtures/combat-progress.json` is written by
//! `script/combat-progress-oracle.mjs`: it drives the upstream
//! `initCombatProgress` state, its grants, the 250 ms forge tick and the
//! Scurge, Coral and Omega kill credits on a seeded stream, and records
//! `combatProgressSummary`. Every case here replays one of those runs, so a
//! rule cannot drift from its source.

use idleon_combat::progress::{Config, Grant, State};
use idleon_common::Rng;
use serde_json::Value;
use std::collections::BTreeMap;

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

fn map(value: &Value) -> BTreeMap<String, f64> {
    value
        .as_object()
        .map(|rows| {
            rows.iter()
                .map(|(key, value)| (key.clone(), number(value)))
                .collect()
        })
        .unwrap_or_default()
}

fn config(case: &Value) -> Config {
    serde_json::from_value(case["config"].clone()).expect("the case carries a progress config")
}

/// One case's script, as the port runs it.
fn run(case: &Value) -> State {
    let seed = case["seed"].as_u64().expect("a seed");
    let mut state = State::new(config(case));
    let mut rng = Rng::new(seed);
    for step in case["steps"].as_array().expect("steps") {
        if let Some(kind) = step.get("grant").and_then(Value::as_str) {
            let seconds = number(&step["seconds"]);
            let kind = match kind {
                "forge" => Grant::Forge,
                "pets" => Grant::Pets,
                _ => Grant::Alchemy,
            };
            state.grant(&mut rng, kind, seconds);
        } else if step.get("scurgeKill").is_some() {
            state.scurge_kill();
        } else if let Some(credit) = step.get("coralKill") {
            state.coral_kill(number(credit));
        } else if step.get("omegaKill").is_some() {
            let grants = idleon_combat::progress::Grants {
                map: 1,
                drop_rate: 1.0,
                ..idleon_combat::progress::Grants::default()
            };
            state.omega_kill(&mut rng, &grants);
        } else if let Some(ticks) = step.get("ticks").and_then(Value::as_u64) {
            // The panel's 250 ms task fires on the run's own clock; the case
            // names the 10 ms passes it covers.
            for pass in 0..ticks {
                if (pass + 1) % 25 == 0 {
                    state.tick(&mut rng);
                }
            }
        } else if step.get("petBatch").is_some() {
            state.pet_batch();
        }
    }
    state
}

fn check(case: &Value, summary: &idleon_combat::progress::Summary) {
    let want = &case["expected"];
    let gains = &want["gains"];
    let note = case["note"].as_str().unwrap_or("");
    assert_eq!(
        number(&gains["forgeSeconds"]),
        summary.forge_seconds,
        "{note}"
    );
    assert_eq!(number(&gains["petSeconds"]), summary.pet_seconds, "{note}");
    assert_eq!(
        number(&gains["alchemySeconds"]),
        summary.alchemy_seconds,
        "{note}"
    );
    assert_eq!(number(&gains["scurgeExp"]), summary.scurge_exp, "{note}");
    assert_eq!(number(&gains["coralKills"]), summary.coral_kills, "{note}");
    assert_eq!(number(&gains["omegaBalls"]), summary.omega_balls, "{note}");
    assert_eq!(map(&want["bars"]), summary.bars, "{note}");
    assert_eq!(map(&want["oreUsed"]), summary.ore_used, "{note}");
    assert_eq!(map(&want["oilUsed"]), summary.oil_used, "{note}");
    assert_eq!(map(&want["storageUsed"]), summary.storage_used, "{note}");
    let notices: Vec<String> = summary.notices.iter().map(|n| n.text.clone()).collect();
    let wanted: Vec<String> = want["notices"]
        .as_array()
        .expect("notices")
        .iter()
        .map(|text| text.as_str().expect("a notice").to_string())
        .collect();
    assert_eq!(wanted, notices, "{note}");
    assert_eq!(
        number(&want["state"]["petPendingSeconds"]),
        summary.pet_pending_seconds,
        "{note}"
    );
    assert_eq!(
        want["state"]["forgeAvailable"].as_bool().unwrap_or(false),
        summary.forge_available,
        "{note}"
    );
    match (&want["state"]["forge"], &summary.forge) {
        (Value::Null, None) => {}
        (want, Some(forge)) => {
            let wanted_order: Vec<String> = want["order"]
                .as_array()
                .expect("order")
                .iter()
                .map(|item| item.as_str().expect("an item key").to_string())
                .collect();
            assert_eq!(wanted_order, forge.order, "{note}");
            let wanted_qty: Vec<f64> = want["qty"]
                .as_array()
                .expect("qty")
                .iter()
                .map(number)
                .collect();
            assert_eq!(wanted_qty, forge.qty, "{note}");
            let wanted_progress: Vec<f64> = want["progress"]
                .as_array()
                .expect("progress")
                .iter()
                .map(number)
                .collect();
            assert_eq!(wanted_progress, forge.progress, "{note}");
            assert_eq!(map(&want["storage"]), forge.storage, "{note}");
        }
        (want, None) => panic!("{note}: the fixture carries {want} but the port has no furnace"),
    }
    match (&want["state"]["scurge"], &summary.scurge) {
        (Value::Null, None) => {}
        (want, Some(scurge)) => {
            assert_eq!(number(&want["level"]), scurge.level, "{note}");
            assert_eq!(number(&want["exp"]), scurge.exp, "{note}");
            assert_eq!(number(&want["requirement"]), scurge.requirement, "{note}");
        }
        (want, None) => panic!("{note}: the fixture carries {want} but the port has no Scurge"),
    }
    match (&want["state"]["coral"], &summary.coral) {
        (Value::Null, None) => {}
        (want, Some(coral)) => assert_eq!(number(&want["kills"]), coral.kills, "{note}"),
        (want, None) => panic!("{note}: the fixture carries {want} but the port has no Coral"),
    }
    match (&want["state"]["omega"], &summary.omega) {
        (Value::Null, None) => {}
        (want, Some(omega)) => {
            assert_eq!(number(&want["balls"]), omega.balls, "{note}");
            assert_eq!(number(&want["drops"]), omega.drops, "{note}");
        }
        (want, None) => panic!("{note}: the fixture carries {want} but the port has no Omega"),
    }
    let wanted_pets = want["pets"].as_array().expect("pets");
    assert_eq!(wanted_pets.len(), summary.pets.len(), "{note}");
    for (want, pet) in wanted_pets.iter().zip(&summary.pets) {
        assert_eq!(number(&want["world"]) as usize, pet.world, "{note}");
        assert_eq!(number(&want["index"]) as usize, pet.index, "{note}");
        assert_eq!(
            want["name"].as_str().unwrap_or_default(),
            pet.name,
            "{note}"
        );
        assert_eq!(number(&want["start"]), pet.start, "{note}");
        assert_eq!(number(&want["gained"]), pet.gained, "{note}");
        assert_eq!(number(&want["end"]), pet.end, "{note}");
        assert_eq!(number(&want["startLevel"]), pet.start_level, "{note}");
        assert_eq!(number(&want["level"]), pet.level, "{note}");
    }
}

#[test]
fn the_forge_matches_upstream_grants_and_ticks() {
    let fixture = fixture();
    for case in fixture["forgeCases"].as_array().expect("forge cases") {
        let state = run(case);
        check(case, &state.summary());
    }
}

#[test]
fn the_pet_batch_matches_upstream() {
    let fixture = fixture();
    for case in fixture["petCases"].as_array().expect("pet cases") {
        let state = run(case);
        check(case, &state.summary());
    }
}

#[test]
fn the_scurge_coral_and_omega_credits_match_upstream() {
    let fixture = fixture();
    for key in ["scurgeCases", "coralCases", "omegaCases"] {
        for case in fixture[key].as_array().expect("cases") {
            let state = run(case);
            check(case, &state.summary());
        }
    }
}

#[test]
fn a_run_with_no_forge_inputs_reports_the_upstream_notice() {
    let mut state = State::new(Config::default());
    let mut rng = Rng::new(1);
    state.grant(&mut rng, Grant::Forge, 30.0);
    state.grant(&mut rng, Grant::Pets, 12.0);
    let summary = state.summary();
    assert_eq!(summary.forge_seconds, 30.0);
    assert_eq!(summary.pet_pending_seconds, 12.0);
    assert!(!summary.forge_available);
    assert!(summary.forge.is_none());
    let keys: Vec<&str> = summary.notices.iter().map(|notice| notice.key).collect();
    assert_eq!(keys, ["forgeState", "petState", "petNonShiny"]);
}
