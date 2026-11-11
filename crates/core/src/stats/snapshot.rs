//! The save-derived inputs the calculation context reads.
//!
//! Ported from the per-character reads in the upstream `js/save/loader.js`:
//! `KLA_<n>` map counters drive which maps a character has visited and which
//! portals it has cleared, `CurrentMap_<n>` is where it stands, and the
//! account fields the map-access rules test are `Dream`, `OptionsListAccount`,
//! `QuestComplete_<n>`, `Holes`, `Lv0_0`, `MapBon` and `CharacterClass_<n>`.
//!
//! Upstream pads the character count to at least ten even when the save names
//! fewer, because the selectors are built before the save is read; the same
//! padding keeps the character selector identical here.

use crate::save::{Save, number, row_values};
use serde_json::Value;

/// One save's slice of the fields the context resolves against.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub(crate) names: Vec<String>,
    pub(crate) classes: Vec<usize>,
    pub(crate) characters: usize,
    /// `CurrentMap_<n>` per character, `None` when the save has no such key.
    pub(crate) current_maps: Vec<Option<usize>>,
    /// `KLA_<char>` rows, one counter row per map; a missing cell stays `None`.
    pub(crate) kla: Vec<Vec<Vec<Option<f64>>>>,
    pub(crate) dreams: Vec<f64>,
    pub(crate) options: Vec<f64>,
    pub(crate) pet_park_level: f64,
    pub(crate) rift_quest: bool,
    pub(crate) hole_unlocked: bool,
    pub(crate) map_bonuses: Vec<Vec<f64>>,
}

impl Snapshot {
    /// Read a decoded save's context fields.
    pub fn from_save(save: &Save) -> Self {
        let names = names(save);
        let characters = character_count(save, names.len());
        let mut classes = vec![0; characters];
        let mut current_maps = vec![None; characters];
        let mut kla = Vec::with_capacity(characters);
        for index in 0..characters {
            classes[index] = save
                .get(&format!("CharacterClass_{index}"))
                .as_ref()
                .and_then(number)
                .map_or(0, |id| id.max(0.0) as usize);
            current_maps[index] = save
                .get(&format!("CurrentMap_{index}"))
                .as_ref()
                .and_then(number)
                .map(|map| map.max(0.0) as usize);
            kla.push(
                rows(save, &format!("KLA_{index}"))
                    .iter()
                    .map(|row| row_values(row).iter().map(number).collect())
                    .collect(),
            );
        }
        let options = save
            .get("OptionsListAccount")
            .or_else(|| save.get("OptLacc"))
            .map(|value| numbers(&value))
            .unwrap_or_default();
        Self {
            names,
            characters,
            classes,
            current_maps,
            kla,
            dreams: save
                .get("Dream")
                .map(|value| numbers(&value))
                .unwrap_or_default(),
            options,
            pet_park_level: rows(save, "Lv0_0").first().and_then(number).unwrap_or(0.0),
            rift_quest: (0..characters).any(|index| {
                save.get(&format!("QuestComplete_{index}"))
                    .and_then(|quests| quests.get("Rift_Ripper1").and_then(number))
                    .is_some_and(|done| done == 1.0)
            }),
            hole_unlocked: rows(save, "Holes")
                .get(1)
                .and_then(|row| row_values(row).first().and_then(number))
                .is_some_and(|level| level >= 1.0),
            map_bonuses: rows(save, "MapBon").iter().map(numbers).collect(),
        }
    }

    /// How many characters the selectors offer.
    pub fn characters(&self) -> usize {
        self.characters
    }

    /// One character's name, as the roster carries it.
    pub fn name(&self, character: usize) -> String {
        self.names
            .get(character)
            .filter(|name| !name.is_empty())
            .cloned()
            .unwrap_or_else(|| format!("Character {}", character + 1))
    }

    /// One character's class id (`CharacterClass_<n>`).
    pub fn class_id(&self, character: usize) -> usize {
        self.classes.get(character).copied().unwrap_or(0)
    }

    /// The map a character is standing on, when its row is in the save.
    pub fn current_map(&self, character: usize) -> Option<usize> {
        self.current_maps.get(character).copied().flatten()
    }

    /// The account's arcane kills for one map, when `MapBon` carries the row.
    pub fn map_bonus(&self, map: usize) -> Option<&[f64]> {
        self.map_bonuses.get(map).map(Vec::as_slice)
    }
}

/// The `charNames` roster beside the save, when it carries one.
fn names(save: &Save) -> Vec<String> {
    save.get("charNames")
        .and_then(|names| names.as_array().cloned())
        .unwrap_or_default()
        .iter()
        .map(|name| name.as_str().unwrap_or_default().to_string())
        .collect()
}

/// How many characters a save carries, padded to the upstream minimum of ten.
fn character_count(save: &Save, named: usize) -> usize {
    const ROWS: [&str; 17] = [
        "Lv0",
        "Exp0",
        "CharacterClass",
        "SL",
        "SM",
        "PlayerStuff",
        "PVtStarSign",
        "PVFishingToolkit",
        "PVStatList",
        "FoodSlO",
        "AttackLoadout",
        "MaxCarryCap",
        "InventoryOrder",
        "ItemQTY",
        "AnvilPAstats",
        "AnvilPA",
        "AnvilPAselect",
    ];
    let inferred = save
        .as_object()
        .map(|fields| {
            fields
                .keys()
                .filter_map(|key| {
                    let (prefix, index) = key.rsplit_once('_')?;
                    ROWS.contains(&prefix)
                        .then(|| index.parse::<usize>().ok())
                        .flatten()
                })
                .map(|index| index + 1)
                .max()
                .unwrap_or(0)
        })
        .unwrap_or(0);
    inferred.max(named).max(10)
}

/// One save key's rows, or none when the key is missing.
fn rows(save: &Save, key: &str) -> Vec<Value> {
    save.get(key)
        .map(|value| row_values(&value))
        .unwrap_or_default()
}

/// Every readable number in a row, in order.
fn numbers(value: &Value) -> Vec<f64> {
    row_values(value).iter().filter_map(number).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_save_with_fewer_than_ten_characters_still_offers_ten() {
        let save = Save::new(json!({
            "charNames": ["Ana"],
            "Lv0_0": "[1]",
            "CharacterClass_0": 7,
            "CurrentMap_0": 50,
        }));
        let snapshot = Snapshot::from_save(&save);
        assert_eq!(snapshot.characters(), 10);
        assert_eq!(snapshot.name(0), "Ana");
        assert_eq!(snapshot.name(1), "Character 2");
        assert_eq!(snapshot.class_id(0), 7);
        assert_eq!(snapshot.current_map(0), Some(50));
        assert_eq!(snapshot.current_map(1), None);
    }

    #[test]
    fn missing_map_counters_stay_missing_instead_of_zero() {
        let save = Save::new(json!({
            "KLA_0": "[[0,0],[]]",
        }));
        let snapshot = Snapshot::from_save(&save);
        assert_eq!(snapshot.kla[0][0], vec![Some(0.0), Some(0.0)]);
        assert_eq!(snapshot.kla[0][1], Vec::<Option<f64>>::new());
    }
}
