//! The fishing slice of a decoded save: characters, their rod and their saved
//! fishing spot and toolkit.
//!
//! The field names follow the upstream loader's `js/save/loader.js`:
//! `Lv0_<n>[4]` is the Fishing level, `EquipOrder_<n>[1][2]` the equipped rod,
//! `PVFishingToolkit_<n>` the saved `[lure, line]` (the key's presence is what
//! the upstream calls "available"), `PVFishingSpotIndex_<n>` the saved spot and
//! `CurrentMap_<n>` / `AFKtarget_<n>` where the character was left.

use super::data::{Location, data};
use crate::save::{Save, at, number};

/// One character's fishing context, as the save carries it.
#[derive(Clone, Debug, PartialEq)]
pub struct Character {
    pub index: usize,
    pub name: String,
    /// `Lv0_<n>[4]`, the Fishing level.
    pub level: u32,
    /// The equipped rod's item key, or `Blank`.
    pub rod_key: String,
    pub rod_name: String,
    pub rod_speed: f64,
    /// The saved lure index; `0` when the save carries no toolkit key.
    pub saved_lure: usize,
    /// The saved line index; `0` when the save carries no toolkit key.
    pub saved_line: usize,
    /// Whether the save carries `PVFishingToolkit_<n>` at all.
    pub toolkit_available: bool,
    /// `CurrentMap_<n>` when the save carries one.
    pub saved_map: Option<usize>,
    /// `PVFishingSpotIndex_<n>` when the save carries one.
    pub saved_spot: Option<usize>,
    /// Whether the save carries `PVFishingSpotIndex_<n>` at all.
    pub spot_available: bool,
    /// `AFKtarget_<n>`, e.g. `FishSmall` or `MotherlodeFISH`.
    pub afk_target: String,
}

impl Default for Character {
    fn default() -> Self {
        Self {
            index: 0,
            name: "Character 1".into(),
            level: 0,
            rod_key: "Blank".into(),
            rod_name: "No rod".into(),
            rod_speed: 0.0,
            saved_lure: 0,
            saved_line: 0,
            toolkit_available: false,
            saved_map: None,
            saved_spot: None,
            spot_available: false,
            afk_target: String::new(),
        }
    }
}

impl Character {
    /// The location the character was fishing, matching the upstream
    /// `savedFishingLocationId`: an exact map/spot/pool row, then the pool's
    /// map-wide row, then the pool outside the normal maps.
    pub fn saved_location(&self) -> Option<&'static Location> {
        let map = self.saved_map;
        let spot = self.saved_spot.unwrap_or(0);
        let target = self.afk_target.as_str();
        let locations = &data().locations;
        locations
            .iter()
            .find(|location| {
                location.map_idx == map
                    && location.spot_idx == Some(spot)
                    && location.pool_key == target
            })
            .or_else(|| {
                locations.iter().find(|location| {
                    location.map_idx == map
                        && location.spot_idx.is_none()
                        && location.pool_key == target
                })
            })
            .or_else(|| {
                locations
                    .iter()
                    .find(|location| location.map_idx.is_none() && location.pool_key == target)
            })
    }
}

/// Every character's fishing context, plus the import warnings it produced.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FishingSave {
    pub characters: Vec<Character>,
}

impl FishingSave {
    /// Read the fishing fields of a decoded save.
    pub fn from_save(save: &Save) -> Self {
        let count = save.character_count();
        Self {
            characters: (0..count).map(|index| read(save, index)).collect(),
        }
    }

    /// A character by index.
    pub fn character(&self, index: usize) -> Option<&Character> {
        self.characters.get(index)
    }

    /// The character a run starts on: the first one left on a fishing pool,
    /// then the highest Fishing level, matching the upstream `defaultCharacter`.
    pub fn default_character(&self) -> usize {
        let pools: Vec<&str> = data().pools.iter().map(|pool| pool.key.as_str()).collect();
        let mut best = 0;
        for (index, character) in self.characters.iter().enumerate() {
            let candidate = (
                pools.contains(&character.afk_target.as_str()),
                character.level,
            );
            let current = self.characters.get(best);
            let current = current.map_or((false, 0), |best| {
                (pools.contains(&best.afk_target.as_str()), best.level)
            });
            if candidate > current {
                best = index;
            }
        }
        best
    }

    /// Whether any character carries a saved toolkit.
    pub fn has_toolkit(&self) -> bool {
        self.characters
            .iter()
            .any(|character| character.toolkit_available)
    }
}

fn read(save: &Save, index: usize) -> Character {
    let name = save.character_name(index);
    let level = save
        .get(&format!("Lv0_{index}"))
        .and_then(|row| at(&row, 4).and_then(number))
        .map(|level| level.max(0.0).round() as u32)
        .unwrap_or(0);
    let rod_key = rod_key(save, index);
    let rod = data().rod(&rod_key);
    let toolkit = save.get(&format!("PVFishingToolkit_{index}"));
    let toolkit_available = toolkit.is_some();
    let saved_lure = toolkit
        .as_ref()
        .and_then(|row| at(row, 0))
        .and_then(number)
        .filter(|value| *value >= 0.0)
        .map_or(0, |value| value.round() as usize);
    let saved_line = toolkit
        .as_ref()
        .and_then(|row| at(row, 1))
        .and_then(number)
        .filter(|value| *value >= 0.0)
        .map_or(0, |value| value.round() as usize);
    let spot = save.get(&format!("PVFishingSpotIndex_{index}"));
    let saved_spot = spot
        .as_ref()
        .and_then(number)
        .map(|value| value.max(0.0).round() as usize);
    Character {
        index,
        name,
        level,
        rod_name: rod
            .map(|rod| rod.name.clone())
            .unwrap_or_else(|| "No rod".into()),
        rod_speed: rod.map(|rod| rod.speed).unwrap_or(0.0),
        rod_key,
        saved_lure,
        saved_line,
        toolkit_available,
        saved_map: save
            .get(&format!("CurrentMap_{index}"))
            .and_then(|value| number(&value))
            .map(|value| value.max(0.0).round() as usize),
        saved_spot,
        spot_available: spot.is_some(),
        afk_target: save
            .get(&format!("AFKtarget_{index}"))
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default(),
    }
}

/// The equipped rod's item key: `EquipOrder_<n>[1][2]`, the tools row's
/// fishing slot.
fn rod_key(save: &Save, index: usize) -> String {
    save.get(&format!("EquipOrder_{index}"))
        .and_then(|order| at(&order, 1).and_then(|row| at(row, 2).cloned()))
        .and_then(|value| value.as_str().map(str::to_owned))
        .filter(|key| !key.is_empty())
        .unwrap_or_else(|| "Blank".into())
}
