//! Reading the sneaking loadout out of an imported save.
//!
//! Ported from the save reads in `sneaking-calc.html`: `Ninja[0..12]` is each
//! twin's placement row, `Ninja[12..60]` the four equipment rows per twin,
//! `Ninja[60..100]` the inventory, `Ninja[100]` the door progress, `Ninja[102]`
//! the emporium row and `Ninja[103]` the Ninja Knowledge levels. `Spelunk[14]`
//! holds the slot symbols, `[15]` the floor flowers and `[18]` the legend talent
//! levels; `Menagerie[7]` the Omni-Jade levels; `Lv0_<n>[17]` each twin's
//! Sneaking level; `OptLacc[231..242]` the mastery pair and the gemstone levels.
//!
//! Rows are read positionally: a cell the save leaves blank becomes zero, it
//! never shifts the cells after it. The account-wide terms the breakdown trees
//! multiply in are resolved by [`crate::save::account`] after the rows load.

use super::{
    EMPORIUM_ROW, EQUIP_START, FLOOR_COUNT, INVENTORY_SCAN, INVENTORY_START, Item, KNOWLEDGE_ROW,
    Loadout, SlotKind, TWIN_COUNT, Twin, charmed_perk,
};
use crate::save::{Save, json, number, row_values};
use serde_json::Value;

/// The first gemstone level in `OptLacc`.
const GEMSTONE_START: usize = 233;
/// The saved mastery and its maximum in `OptLacc`.
const MASTERY: usize = 231;
/// The slot symbol row in `Spelunk`.
const SYMBOLS_ROW: usize = 14;
/// The floor flowers row in `Spelunk`.
const FLOWERS_ROW: usize = 15;
/// The legend talent levels row in `Spelunk`.
const LEGEND_ROW: usize = 18;

impl Loadout {
    /// Read the loadout a save carries, or `None` when it has no `Ninja` rows.
    pub fn from_save(save: &Save) -> Option<Self> {
        let ninja = save.get("Ninja")?;
        let rows: Vec<Vec<Value>> = row_values(&ninja).iter().map(row_values).collect();
        if rows.len() <= KNOWLEDGE_ROW {
            return None;
        }
        let mut missing = Vec::new();
        let cell = |row: usize, index: usize| -> Option<f64> {
            rows.get(row)
                .and_then(|cells| cells.get(index))
                .and_then(number)
        };

        let spelunk: Vec<Vec<Value>> = save
            .get("Spelunk")
            .map(|value| row_values(&value).iter().map(row_values).collect())
            .unwrap_or_default();
        let spelunk_row = |index: usize| -> Vec<f64> {
            numbers(spelunk.get(index).map_or(&[][..], Vec::as_slice))
        };
        // The app writes `OptionsListAccount`; a raw game save writes `OptLacc`.
        let options: Vec<f64> = match save.get("OptionsListAccount") {
            Some(value) => numbers(&row_values(&value)),
            None => save
                .get("OptLacc")
                .map(|value| numbers(&row_values(&value)))
                .unwrap_or_default(),
        };
        let omni: Vec<f64> = save
            .get("Menagerie")
            .map(|value| row_values(&value))
            .and_then(|rows| rows.get(7).cloned())
            .map(|row| numbers(&row_values(&row)))
            .unwrap_or_default();
        let characters = save.character_count().max(1);
        let players = characters.min(TWIN_COUNT);

        let twins: Vec<Twin> = (0..TWIN_COUNT.min(rows.len()).min(players))
            .filter(|index| rows.get(*index).is_some_and(|row| !row.is_empty()))
            .map(|index| {
                let floor = cell(index, 0).unwrap_or(0.0).max(0.0) as usize;
                let untie_progress = cell(index, 3).unwrap_or(0.0);
                let untie_required = super::data::data()
                    .info
                    .untie_required
                    .get(index)
                    .copied()
                    .unwrap_or(0.0);
                let slots: [Item; 4] =
                    std::array::from_fn(|slot| item(rows.get(EQUIP_START + 4 * index + slot)));
                Twin::new(
                    index,
                    save.character_name(index),
                    floor.min(FLOOR_COUNT - 1),
                    untie_progress,
                    untie_required,
                    save.get(&format!("Lv0_{index}"))
                        .map(|row| row_values(&row))
                        .and_then(|row| row.get(17).and_then(number))
                        .map_or(0, |level| level.max(0.0).round() as u32),
                    slots,
                )
            })
            .collect();

        let inventory: Vec<Item> = (0..INVENTORY_SCAN)
            .map(|row| item(rows.get(INVENTORY_START + row)))
            .collect();
        let knowledge = numbers(rows.get(KNOWLEDGE_ROW).map_or(&[][..], Vec::as_slice));
        let doors = numbers(rows.get(super::DOOR_ROW).map_or(&[][..], Vec::as_slice));
        let emporium = rows
            .get(EMPORIUM_ROW)
            .and_then(|row| row.get(9))
            .and_then(Value::as_str)
            .unwrap_or_default();

        if spelunk.is_empty() {
            missing.push(
                "Spelunk: the slot symbols, floor flowers and legend points are read as zero"
                    .into(),
            );
        }
        if options.is_empty() {
            missing.push("OptLacc: the mastery and gemstone levels are read as zero".into());
        }
        if omni.is_empty() {
            missing.push("Menagerie: the Omni-Jade levels are read as zero".into());
        }

        let mut loadout = Self::new(
            twins,
            inventory,
            knowledge,
            omni,
            spelunk_row(SYMBOLS_ROW),
            spelunk_row(FLOWERS_ROW),
            spelunk_row(LEGEND_ROW),
            (GEMSTONE_START..GEMSTONE_START + 8)
                .map(|index| options.get(index).copied().unwrap_or(0.0))
                .collect(),
            doors,
            (0..players)
                .map(|index| {
                    save.get(&format!("Exp0_{index}"))
                        .map(|row| row_values(&row))
                        .and_then(|row| row.get(17).and_then(number))
                        .unwrap_or(0.0)
                })
                .collect(),
            rows.get(EMPORIUM_ROW)
                .and_then(|row| row.get(10))
                .and_then(number)
                .map_or(0, |phase| phase.max(0.0).round() as u32),
            options.get(MASTERY).copied().unwrap_or(0.0).max(0.0) as u32,
            options.get(MASTERY + 1).copied().unwrap_or(0.0).max(0.0) as u32,
            charmed_perk(emporium),
            players,
            missing,
        );
        loadout.account = crate::save::account::sneaking_account(save, &loadout);
        let gold = loadout.gold_bonuses();
        loadout.account.jade.gold_9 = gold.get(9);
        loadout.account.jade.gold_23 = gold.get(23);
        loadout.account.exp.gold_10 = gold.get(10);
        Some(loadout)
    }

    /// The saved item of one twin's slot, for the slot's reset control.
    pub fn saved_slot(&self, twin: usize, slot: SlotKind) -> Option<&Item> {
        self.twin(twin).map(|entry| entry.saved_slot(slot))
    }
}

/// Every cell of a row as a number, keeping the row's positions.
fn numbers(cells: &[Value]) -> Vec<f64> {
    cells
        .iter()
        .map(|cell| number(cell).unwrap_or(0.0))
        .collect()
}

/// One `[key, level]` row as an item, `Blank` when the row is missing.
fn item(row: Option<&Vec<Value>>) -> Item {
    let Some(row) = row else {
        return Item::blank();
    };
    let key = row.first().and_then(Value::as_str).unwrap_or_default();
    if key.is_empty() || key == "Blank" {
        return Item::blank();
    }
    let level = row.get(1).and_then(json::number).unwrap_or(0.0).max(0.0) as u32;
    Item::new(key, level)
}
