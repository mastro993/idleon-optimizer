//! The combat loadout editor's model against real save data.
//!
//! The small hand-built saves live beside the module; this file checks the
//! committed synthetic account, whose equipment and talent tables are the
//! realistic case. Expected values come from the upstream readers
//! (`readEquipment`, `readTalents` in `combat-sim-setup.js`), not from the
//! fixture itself.

use idleon_core::combat::{IssueKind, Loadout, data};

fn fixture() -> idleon_core::save::Imported {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/data.json");
    let text = std::fs::read_to_string(path).expect("fixtures/data.json is committed");
    idleon_core::save::import(&text).expect("the fixture imports")
}

#[test]
fn the_fixture_loads_a_full_character_loadout() {
    let imported = fixture();
    assert!(imported.characters > 0);
    let loadout = Loadout::from_save(&imported.save, 0);
    assert_eq!(loadout.slots.len(), data::SLOT_COUNT);
    assert_eq!(loadout.character.index, 0);
    assert!(
        loadout.character.level > 0,
        "the fixture has a levelled first character"
    );
    assert!(
        loadout.slots.iter().any(|slot| !data::is_empty(&slot.item)),
        "the fixture's first character wears gear"
    );
    assert!(
        !loadout.talents.is_empty(),
        "the fixture's first character has talent levels"
    );
    assert!(loadout.missing.is_empty(), "{}", loadout.missing.join(", "));
}

#[test]
fn every_equipped_fixture_item_resolves_to_the_catalog() {
    let imported = fixture();
    let loadout = Loadout::from_save(&imported.save, 0);
    let unknown: Vec<&str> = loadout
        .slots
        .iter()
        .filter(|slot| slot.def().is_none() && !data::is_empty(&slot.item))
        .map(|slot| slot.item.as_str())
        .collect();
    assert!(
        unknown.is_empty(),
        "the catalog covers every equipped item: {unknown:?}"
    );
    let issues = loadout.issues();
    assert!(
        !issues
            .iter()
            .any(|issue| issue.kind == IssueKind::UnknownItem),
        "{issues:?}"
    );
}

#[test]
fn the_fixture_character_totals_recompute_from_its_equipment() {
    let imported = fixture();
    let loadout = Loadout::from_save(&imported.save, 0);
    let totals = loadout.totals();
    let expected: f64 = loadout
        .slots
        .iter()
        .map(|slot| slot.base_stat("Defence") + slot.added.get("Defence").copied().unwrap_or(0.0))
        .sum();
    assert!((totals.defence - expected).abs() < 1e-9);
    assert_eq!(totals.slots, data::SLOT_COUNT);
    assert_eq!(
        totals.equipped,
        loadout
            .slots
            .iter()
            .filter(|slot| !data::is_empty(&slot.item))
            .count()
    );
    assert!(!totals.line().is_empty());
}
