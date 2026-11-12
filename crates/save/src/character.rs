//! One character's progress, from the `Lv0_<n>` and `Exp0_<n>` rows.

use crate::json;
use serde_json::{Map, Value};

/// `Lv0_<n>` row indices: the class level at 0, then one per skill.
const CONSTRUCTION: usize = 8;
const SAILING: usize = 13;
const DIVINITY: usize = 14;
const FARMING: usize = 16;
const RESEARCH: usize = 20;

#[derive(Clone, Debug, Default)]
pub struct Character {
    levels: Vec<Option<f64>>,
    research_exp: Option<f64>,
}

impl Character {
    /// The class level (`Lv0_<n>[0]`), or 0 when missing.
    pub fn class_level(&self) -> f64 {
        self.raw(0).unwrap_or(0.0)
    }

    /// The level of `skill` (0 = the first skill row), or 0 when missing.
    pub fn skill(&self, skill: usize) -> f64 {
        self.raw(skill + 1).unwrap_or(0.0)
    }

    pub fn construction_level(&self) -> Option<f64> {
        self.raw(CONSTRUCTION)
    }

    pub fn sailing_level(&self) -> Option<u32> {
        self.whole(SAILING)
    }

    pub fn divinity(&self) -> Option<u32> {
        self.whole(DIVINITY)
    }

    pub fn farming_level(&self) -> Option<u32> {
        self.whole(FARMING)
    }

    pub fn research_level(&self) -> Option<u32> {
        self.whole(RESEARCH)
    }

    /// The accumulated research EXP (`Exp0_<n>[20]`).
    pub fn research_exp(&self) -> Option<f64> {
        self.research_exp
    }

    fn raw(&self, row: usize) -> Option<f64> {
        self.levels.get(row).copied().flatten()
    }

    fn whole(&self, row: usize) -> Option<u32> {
        self.raw(row).map(|level| level.max(0.0).round() as u32)
    }
}

/// Every character the save carries, indexed by its `Lv0_<n>` suffix; a gap in
/// the numbering is a default character.
pub fn characters(save: &Map<String, Value>) -> Vec<Character> {
    let mut characters: Vec<Character> = Vec::new();
    for key in save.keys() {
        let Some(index) = character_index(key) else {
            continue;
        };
        if characters.len() <= index {
            characters.resize(index + 1, Character::default());
        }
        characters[index].levels = json::parsed_field(save, key)
            .as_ref()
            .and_then(Value::as_array)
            .map(|cells| cells.iter().map(json::number).collect())
            .unwrap_or_default();
        characters[index].research_exp = json::parsed_field(save, &format!("Exp0_{index}"))
            .as_ref()
            .and_then(Value::as_array)
            .and_then(|cells| cells.get(RESEARCH))
            .and_then(json::number);
    }
    characters
}

/// How many characters the save carries: one per `Lv0_<n>` row.
pub fn character_count(save: &Map<String, Value>) -> usize {
    save.keys().filter_map(|key| character_index(key)).count()
}

fn character_index(key: &str) -> Option<usize> {
    key.strip_prefix("Lv0_")?.parse().ok()
}
