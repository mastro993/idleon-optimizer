//! The loadout editor's model: a character's equipment and talents, read from a
//! decoded save and edited in memory.
//!
//! Ported from the pure half of `js/stats/systems/common/combat-sim-setup.js`
//! (`readEquipment`, `readTalents`, `readCharacter`, `itemName`) and the item
//! slot rules in `js/ui/combat-sim-setup-ui.js`. Upstream edits a setup of typed
//! patches over a cloned save; this model edits the resolved loadout directly,
//! because the editor is the only consumer and the save stays read-only.
//!
//! A newly equipped item drops its saved `EMm` upgrade deltas, exactly as the
//! upstream `equipment` patch does unless `keepStats` is set. Every edit
//! recomputes the totals from scratch, so a view that reads them is never stale.

use super::data;
use super::preset::Preset;
use crate::save::{Save, at, number, row_values};
use serde_json::Value;
use std::collections::BTreeMap;

/// Who the loadout belongs to.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Character {
    pub index: usize,
    pub name: String,
    pub level: u32,
    pub class_id: u32,
    pub class: String,
}

/// One equipment slot: the item and the save's upgrade deltas on it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Slot {
    pub row: usize,
    pub slot: usize,
    /// The equipped item key; `Blank` is an empty slot.
    pub item: String,
    /// The `EMm{row}` deltas on top of the item's base stats.
    pub added: BTreeMap<String, f64>,
    saved_item: String,
    saved_added: BTreeMap<String, f64>,
}

impl Slot {
    /// The slot's label, such as `Helmet` or `Pickaxe`.
    pub fn name(&self) -> String {
        data::slot_name(self.row, self.slot)
    }

    /// The equipped item's readable name.
    pub fn item_name(&self) -> String {
        data::item_name(&self.item)
    }

    /// The equipped item's catalog entry, when the catalog knows it.
    pub fn def(&self) -> Option<&'static data::ItemDef> {
        data::item(&self.item)
    }

    /// The item's base value of one stat.
    pub fn base_stat(&self, stat: &str) -> f64 {
        self.def().map_or(0.0, |def| def.stat(stat))
    }

    /// Base plus saved deltas for one stat.
    pub fn total(&self, stat: &str) -> f64 {
        self.base_stat(stat) + self.added.get(stat).copied().unwrap_or(0.0)
    }

    /// Whether the slot differs from the save.
    pub fn changed(&self) -> bool {
        self.item != self.saved_item || self.added != self.saved_added
    }

    /// Put the slot back to its saved item and deltas.
    pub fn reset(&mut self) {
        self.item = self.saved_item.clone();
        self.added = self.saved_added.clone();
    }
}

/// One talent row: its saved level and the saved maximum.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Talent {
    pub id: u32,
    pub name: String,
    pub level: u32,
    pub max: u32,
    saved_level: u32,
}

impl Talent {
    /// Whether the level differs from the save.
    pub fn changed(&self) -> bool {
        self.level != self.saved_level
    }

    /// Put the level back to the save.
    pub fn reset(&mut self) {
        self.level = self.saved_level;
    }
}

/// The whole loadout: the character, its equipment slots and its talents.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Loadout {
    pub character: Character,
    /// Armor slots then tool slots, in save order.
    pub slots: Vec<Slot>,
    /// Every talent the save stores a level or a maximum for, by id.
    pub talents: Vec<Talent>,
    /// Save fields the character does not carry, named for the UI.
    pub missing: Vec<String>,
}

impl Loadout {
    /// A loadout with every slot empty and no talents.
    pub fn empty() -> Self {
        let mut slots = Vec::with_capacity(data::SLOT_COUNT);
        for (row, count) in [(0, data::ARMOR_SLOTS.len()), (1, data::TOOL_SLOTS.len())] {
            for slot in 0..count {
                slots.push(Slot {
                    row,
                    slot,
                    item: "Blank".into(),
                    ..Slot::default()
                });
            }
        }
        Self {
            character: Character::default(),
            slots,
            talents: Vec::new(),
            missing: Vec::new(),
        }
    }

    /// Read one character's saved equipment and talents.
    ///
    /// A save without `EquipOrder_<ci>` or `SL_<ci>`/`SM_<ci>` still produces a
    /// loadout: the slots stay empty and the field is named in [`Self::missing`],
    /// so the editor never has to guess what a save did not carry.
    pub fn from_save(save: &Save, index: usize) -> Self {
        let mut loadout = Self::empty();
        loadout.character = Character {
            index,
            name: save.character_name(index),
            level: read_number(save, &format!("Lv0_{index}"), 0).unwrap_or(0.0) as u32,
            class_id: save
                .get(&format!("CharacterClass_{index}"))
                .as_ref()
                .and_then(number)
                .unwrap_or(0.0) as u32,
            class: String::new(),
        };
        loadout.character.class = data::class_name(loadout.character.class_id);

        let order_key = format!("EquipOrder_{index}");
        let order = save.get(&order_key);
        if order.is_none() {
            loadout.missing.push(order_key.clone());
        }
        for slot in &mut loadout.slots {
            let emm_key = format!("EMm{}_{index}", slot.row);
            let emm = save.get(&emm_key);
            slot.item = order
                .as_ref()
                .and_then(|rows| at(rows, slot.row))
                .map(row_values)
                .and_then(|row| row.get(slot.slot).cloned())
                .and_then(|key| key.as_str().map(str::to_string))
                .filter(|key| !key.is_empty())
                .unwrap_or_else(|| "Blank".into());
            slot.added = emm
                .as_ref()
                .and_then(|map| at(map, slot.slot))
                .map(read_deltas)
                .unwrap_or_default();
            slot.saved_item = slot.item.clone();
            slot.saved_added = slot.added.clone();
        }

        let levels_key = format!("SL_{index}");
        let maxes_key = format!("SM_{index}");
        let levels = save.get(&levels_key);
        let maxes = save.get(&maxes_key);
        if levels.is_none() && maxes.is_none() {
            loadout.missing.push(levels_key);
        }
        for id in talent_ids(levels.as_ref(), maxes.as_ref()) {
            let level = levels
                .as_ref()
                .and_then(|map| map.get(id.to_string()))
                .and_then(number)
                .unwrap_or(0.0) as u32;
            let max = maxes
                .as_ref()
                .and_then(|map| map.get(id.to_string()))
                .and_then(number)
                .unwrap_or(0.0) as u32;
            loadout.talents.push(Talent {
                id,
                name: data::talent_name(id),
                level,
                max,
                saved_level: level,
            });
        }
        loadout
    }

    /// One slot by its save position.
    pub fn slot(&self, row: usize, slot: usize) -> Option<&Slot> {
        self.slots
            .iter()
            .find(|entry| entry.row == row && entry.slot == slot)
    }

    /// One slot by its position in the flat slot list the editor shows.
    pub fn slot_at(&self, index: usize) -> Option<&Slot> {
        self.slots.get(index)
    }

    /// The flat slot position of a save slot.
    pub fn slot_index(&self, row: usize, slot: usize) -> Option<usize> {
        self.slots
            .iter()
            .position(|entry| entry.row == row && entry.slot == slot)
    }

    /// Equip an item, dropping the previous item's saved deltas. An empty key
    /// unequips the slot.
    pub fn set_item(&mut self, row: usize, slot: usize, key: &str) {
        let normalized = if data::is_empty(key) { "Blank" } else { key };
        if let Some(entry) = self
            .slots
            .iter_mut()
            .find(|entry| entry.row == row && entry.slot == slot)
            && entry.item != normalized
        {
            entry.item = normalized.to_string();
            entry.added.clear();
        }
    }

    /// Set one talent, clamped to the saved maximum.
    pub fn set_talent(&mut self, id: u32, level: u32) {
        if let Some(talent) = self.talents.iter_mut().find(|talent| talent.id == id) {
            talent.level = level.min(talent.max);
        }
    }

    /// Put every slot and talent back to the save.
    pub fn reset(&mut self) {
        for slot in &mut self.slots {
            slot.reset();
        }
        for talent in &mut self.talents {
            talent.reset();
        }
    }

    /// How many slots and talents differ from the save.
    pub fn changed(&self) -> usize {
        self.slots.iter().filter(|slot| slot.changed()).count()
            + self
                .talents
                .iter()
                .filter(|talent| talent.changed())
                .count()
    }

    /// The class's main stat, as upstream `primaryStatForClass` names it.
    pub fn main_stat(&self) -> &'static str {
        data::primary_stat(self.character.class_id)
    }

    /// Base plus saved deltas, with the class's main stat picked out.
    pub fn totals(&self) -> Totals {
        let mut stats: BTreeMap<String, f64> = BTreeMap::new();
        for stat in data::GEAR_STATS {
            let total: f64 = self
                .slots
                .iter()
                .map(|slot| slot.total(stat))
                .filter(|value| *value != 0.0)
                .sum();
            if total != 0.0 {
                stats.insert(stat.to_string(), total);
            }
        }
        let main_stat = self.main_stat();
        Totals {
            weapon_power: stats.get("Weapon_Power").copied().unwrap_or(0.0),
            defence: stats.get("Defence").copied().unwrap_or(0.0),
            main_stat,
            main_value: stats.get(main_stat).copied().unwrap_or(0.0),
            equipped: self
                .slots
                .iter()
                .filter(|slot| !data::is_empty(&slot.item))
                .count(),
            slots: self.slots.len(),
            talents: self.talents.iter().map(|talent| talent.level).sum(),
            stats,
        }
    }

    /// Every item and talent the catalog rejects, with the reason.
    pub fn issues(&self) -> Vec<Issue> {
        let mut issues = Vec::new();
        for slot in &self.slots {
            if data::is_empty(&slot.item) {
                continue;
            }
            let target = IssueTarget::Equipment {
                row: slot.row,
                slot: slot.slot,
            };
            let name = slot.name();
            let Some(def) = slot.def() else {
                issues.push(Issue {
                    target,
                    kind: IssueKind::UnknownItem,
                    detail: format!("{name}: “{}” is not in the item catalog", slot.item),
                });
                continue;
            };
            let item = data::clean_name(&def.name);
            if !data::fits(slot.row, slot.slot, def) {
                issues.push(Issue {
                    target,
                    kind: IssueKind::WrongSlot,
                    detail: format!("{name}: {item} does not fit this slot"),
                });
            }
            if def.level > self.character.level {
                issues.push(Issue {
                    target,
                    kind: IssueKind::LevelRequirement,
                    detail: format!(
                        "{name}: {item} needs level {}, {} is {}",
                        def.level, self.character.name, self.character.level
                    ),
                });
            }
            if def.class() != "ALL"
                && data::class_family(self.character.class_id) != Some(def.class())
            {
                issues.push(Issue {
                    target,
                    kind: IssueKind::ClassRequirement,
                    detail: format!(
                        "{name}: {item} needs the {} class, {} is {}",
                        data::clean(def.class()),
                        self.character.name,
                        self.character.class
                    ),
                });
            }
        }
        for talent in &self.talents {
            if talent.level > talent.max {
                issues.push(Issue {
                    target: IssueTarget::Talent(talent.id),
                    kind: IssueKind::TalentOverMax,
                    detail: format!(
                        "{}: level {} is above the saved maximum {}",
                        talent.name, talent.level, talent.max
                    ),
                });
            }
        }
        issues
    }

    /// The issues that belong to one equipment slot.
    pub fn slot_issues(&self, row: usize, slot: usize) -> Vec<Issue> {
        self.issues()
            .into_iter()
            .filter(|issue| issue.target == (IssueTarget::Equipment { row, slot }))
            .collect()
    }

    /// A preset of this loadout's items and talents.
    pub fn preset(&self, name: &str) -> Preset {
        Preset {
            name: name.trim().to_string(),
            character: self.character.index,
            items: self.slots.iter().map(|slot| slot.item.clone()).collect(),
            talents: self
                .talents
                .iter()
                .map(|talent| (talent.id.to_string(), talent.level))
                .collect(),
        }
    }
}

/// Summed gear stats for the live summary.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    /// Weapon power across the equipped weapons.
    pub weapon_power: f64,
    /// Defence across every slot.
    pub defence: f64,
    /// The class's main stat.
    pub main_stat: &'static str,
    /// The main stat's total.
    pub main_value: f64,
    /// Slots holding an item.
    pub equipped: usize,
    /// Slots the loadout has.
    pub slots: usize,
    /// Summed talent levels.
    pub talents: u32,
    /// Every gear stat with a nonzero total.
    pub stats: BTreeMap<String, f64>,
}

impl Totals {
    /// The one-line summary the section header and the harness read.
    pub fn line(&self) -> String {
        format!(
            "Weapon Power {:.0} · Defence {:.0} · {} {:.0} · {} talent points",
            self.weapon_power, self.defence, self.main_stat, self.main_value, self.talents
        )
    }
}

/// Where an issue points.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueTarget {
    Equipment { row: usize, slot: usize },
    Talent(u32),
}

/// Why an issue was raised.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueKind {
    /// The save holds an item key the catalog does not know.
    UnknownItem,
    /// The item's family does not belong in that slot.
    WrongSlot,
    /// The character's level is below the item's requirement.
    LevelRequirement,
    /// The character's class does not match the item's class.
    ClassRequirement,
    /// The saved talent level is above the saved maximum.
    TalentOverMax,
}

/// One rejected item or talent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Issue {
    pub target: IssueTarget,
    pub kind: IssueKind,
    pub detail: String,
}

/// A saved stat value at one row index, decoded through `Save`.
fn read_number(save: &Save, key: &str, index: usize) -> Option<f64> {
    save.get(key)
        .as_ref()
        .and_then(|value| at(value, index))
        .and_then(number)
}

/// The `EMm` entry's numeric deltas.
fn read_deltas(entry: &Value) -> BTreeMap<String, f64> {
    let Some(map) = entry.as_object() else {
        return BTreeMap::new();
    };
    data::DELTA_STATS
        .into_iter()
        .filter_map(|stat| {
            let value = map.get(stat).and_then(number)?;
            (value != 0.0).then(|| (stat.to_string(), value))
        })
        .collect()
}

/// Every talent id the level or max table stores, ascending.
fn talent_ids(levels: Option<&Value>, maxes: Option<&Value>) -> Vec<u32> {
    let mut ids: Vec<u32> = [levels, maxes]
        .into_iter()
        .flatten()
        .filter_map(Value::as_object)
        .flat_map(|map| map.keys())
        .filter_map(|key| key.parse::<u32>().ok())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}
