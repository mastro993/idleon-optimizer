//! Named loadouts: the item keys and talent levels the project JSON keeps.
//!
//! Upstream stores named setups per character in browser storage
//! (`js/ui/combat-sim-loadouts.js`). A preset here is data only; applying one
//! edits the loadout the same way equipping each slot by hand does, and the app
//! keeps the list in the project's section config.

use super::data;
use super::loadout::Loadout;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A named loadout: the item keys and talent levels, saved in the project JSON.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Preset {
    pub name: String,
    /// The character index the preset was captured from.
    pub character: usize,
    /// Item keys in [`Loadout::slots`] order.
    pub items: Vec<String>,
    /// Talent levels by id, as a JSON object.
    pub talents: BTreeMap<String, u32>,
}

/// Why a preset could not be applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PresetError {
    /// The preset was captured from another character.
    OtherCharacter { preset: usize, current: usize },
    /// The preset's slot count does not fit this build.
    Slots { preset: usize, expected: usize },
}

impl std::fmt::Display for PresetError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OtherCharacter { preset, current } => write!(
                out,
                "the preset belongs to character {}; character {} is selected",
                preset + 1,
                current + 1
            ),
            Self::Slots { preset, expected } => write!(
                out,
                "the preset has {preset} slots; this build edits {expected}"
            ),
        }
    }
}

impl Preset {
    /// Apply the preset to a loadout of the same character.
    pub fn apply_to(&self, loadout: &mut Loadout) -> Result<(), PresetError> {
        if self.character != loadout.character.index {
            return Err(PresetError::OtherCharacter {
                preset: self.character,
                current: loadout.character.index,
            });
        }
        if self.items.len() != loadout.slots.len() {
            return Err(PresetError::Slots {
                preset: self.items.len(),
                expected: loadout.slots.len(),
            });
        }
        let items = self.items.clone();
        for (slot, key) in loadout.slots.iter_mut().zip(items) {
            slot.item = if data::is_empty(&key) {
                "Blank".into()
            } else {
                key
            };
            slot.added.clear();
        }
        for talent in &mut loadout.talents {
            if let Some(level) = self.talents.get(&talent.id.to_string()) {
                talent.level = (*level).min(talent.max);
            }
        }
        Ok(())
    }
}
