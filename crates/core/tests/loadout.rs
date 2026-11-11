//! The loadout model against small purpose-built saves.
//!
//! The readers, edits, totals, flags and presets are exercised here; the
//! committed synthetic account is covered by `tests/combat.rs`. Expected values
//! come from the upstream readers in `combat-sim-setup.js`, not from a fixture.

use idleon_core::combat::{IssueKind, Loadout, Preset, PresetError};
use idleon_core::save::Save;
use serde_json::json;

fn save() -> Save {
    Save::new(json!({
        "charNames": ["Ana", "Bo"],
        "Lv0_0": "[120, 1, 2]",
        "CharacterClass_0": 7,
        "EquipOrder_0": [
            ["EquipmentHats1", "EquipmentSword1", "", "", "", "", "", "", "", "", "", "", "", "", "", ""],
            ["", "", "", "", "", "", ""],
            []
        ],
        "EMm0_0": { "0": { "STR": 2, "Defence": 4 }, "1": { "Weapon_Power": 3 } },
        "EMm1_0": {},
        "SL_0": { "0": 5, "3": 2 },
        "SM_0": { "0": 20, "3": 10, "7": 0 },
    }))
}

#[test]
fn reads_saved_gear_talents_and_the_character() {
    let loadout = Loadout::from_save(&save(), 0);
    assert_eq!(loadout.character.name, "Ana");
    assert_eq!(loadout.character.level, 120);
    assert_eq!(loadout.character.class, "Warrior");
    assert_eq!(loadout.slots.len(), 23);
    let helmet = loadout.slot(0, 0).expect("helmet slot");
    assert_eq!(helmet.item, "EquipmentHats1");
    assert_eq!(helmet.total("STR"), 4.0);
    assert_eq!(helmet.total("Defence"), 7.0);
    let weapon = loadout.slot(0, 1).expect("weapon slot");
    assert_eq!(weapon.total("Weapon_Power"), 29.0);
    assert_eq!(
        loadout
            .talents
            .iter()
            .map(|talent| talent.id)
            .collect::<Vec<_>>(),
        [0, 3, 7]
    );
    assert_eq!(loadout.talents[0].name, "Health Booster");
    assert_eq!(loadout.talents[0].max, 20);
    assert!(loadout.missing.is_empty());
}

#[test]
fn a_save_without_gear_or_talents_names_the_missing_fields() {
    let loadout = Loadout::from_save(&Save::new(json!({ "charNames": ["Ana"] })), 0);
    assert!(loadout.missing.contains(&"EquipOrder_0".to_string()));
    assert!(loadout.missing.contains(&"SL_0".to_string()));
    assert!(loadout.slots.iter().all(|slot| slot.item == "Blank"));
    assert!(loadout.talents.is_empty());
}

#[test]
fn edits_change_the_totals_and_reset_restores_the_save() {
    let mut loadout = Loadout::from_save(&save(), 0);
    assert_eq!(
        loadout.totals().weapon_power,
        29.0,
        "the weapon's base power plus the saved +3"
    );
    loadout.set_item(0, 1, "Blank");
    assert_eq!(loadout.totals().weapon_power, 0.0);
    loadout.set_item(0, 1, "EquipmentSword1");
    assert_eq!(
        loadout.totals().weapon_power,
        26.0,
        "re-equipping drops the previous item's deltas"
    );
    loadout.set_talent(0, 1);
    assert_eq!(loadout.totals().talents, 1 + 2);
    assert_eq!(loadout.changed(), 2);
    loadout.reset();
    assert_eq!(loadout.totals().weapon_power, 29.0);
    assert_eq!(loadout.changed(), 0);
}

#[test]
fn invalid_and_missing_items_are_flagged() {
    let mut loadout = Loadout::from_save(&save(), 0);
    loadout.set_item(0, 2, "NotAnItem");
    loadout.set_item(0, 3, "EquipmentHats1");
    loadout.set_item(0, 4, "EquipmentSword1");
    loadout.set_item(0, 5, "EquipmentHats61");
    loadout.set_item(0, 6, "EquipmentHats71");
    loadout.talents[0].level = 25;
    let issues = loadout.issues();
    let kinds: Vec<IssueKind> = issues.iter().map(|issue| issue.kind).collect();
    assert!(kinds.contains(&IssueKind::UnknownItem), "{issues:?}");
    assert!(kinds.contains(&IssueKind::WrongSlot), "{issues:?}");
    assert!(kinds.contains(&IssueKind::ClassRequirement), "{issues:?}");
    assert!(kinds.contains(&IssueKind::LevelRequirement), "{issues:?}");
    assert!(kinds.contains(&IssueKind::TalentOverMax), "{issues:?}");
    // The saved helmet and sword fit a level 120 warrior, so they stay quiet.
    assert!(loadout.slot_issues(0, 0).is_empty(), "{issues:?}");
    assert!(loadout.slot_issues(0, 1).is_empty(), "{issues:?}");
}

#[test]
fn a_preset_round_trips_and_refuses_another_character() {
    let mut loadout = Loadout::from_save(&save(), 0);
    loadout.set_item(0, 1, "EquipmentSword1");
    loadout.set_talent(3, 4);
    let preset = loadout.preset("Bossing");
    let json = serde_json::to_string(&preset).expect("the preset serializes");
    let back: Preset = serde_json::from_str(&json).expect("the preset reads back");
    assert_eq!(back, preset);

    let mut second = Loadout::from_save(&save(), 0);
    second.set_item(0, 1, "Blank");
    back.apply_to(&mut second)
        .expect("the same character applies");
    assert_eq!(second.slot(0, 1).expect("weapon").item, "EquipmentSword1");
    assert_eq!(second.talents[1].level, 4);

    let mut other = Loadout::from_save(&save(), 1);
    assert_eq!(
        back.apply_to(&mut other),
        Err(PresetError::OtherCharacter {
            preset: 0,
            current: 1
        })
    );
}
