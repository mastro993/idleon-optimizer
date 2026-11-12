//! Equipment, obol and gallery bonuses, ported from
//! `js/stats/systems/common/etcBonus.js` and its sources.
//!
//! `EtcBonuses(108)` is one total across every gear source, applied as a single
//! multiplier. The sources are the equipped items, obols, gallery nametags,
//! trophies, hatrack hats and the Cape Vine.

use super::Decoder;
use super::fields;
use serde_json::Value;

impl Decoder<'_> {
    /// `EtcBonuses(108)` as the research equipment multiplier, capped at 15x.
    pub(super) fn research_equipment_multi(&mut self) -> Option<f64> {
        let stat = self.data.etc_stat(108)?.to_string();
        let has_gear = fields::field(self.save, "EMm0_0").is_some()
            || fields::field(self.save, "EquipOrder_0").is_some()
            || fields::field(self.save, "ObolEqO0_0").is_some()
            || fields::field(self.save, "Spelunk").is_some();
        if !has_gear {
            self.warn(
                "EMm0_0",
                "no equipment, obol or gallery data; the equipment multiplier stays at 1",
            );
            return None;
        }
        let total = self.equipment_bonus(&stat, 0)
            + self.obol_bonus(&stat, 0)
            + self.nametag_bonus(&stat, 0)
            + self.trophy_bonus(&stat, 0)
            + self.hatrack_bonus(&stat, 0)
            + self.capevine_bonus(&stat, 0);
        Some((1.0 + total / 100.0).clamp(1.0, 15.0))
    }

    /// `EtcBonuses(index)` for one character: every gear source added up, as
    /// `etcBonus.resolve` does. A save without the gear fields totals zero.
    pub(super) fn etc(&self, index: usize, character: usize) -> f64 {
        let Some(stat) = self.data.etc_stat(index).map(str::to_owned) else {
            return 0.0;
        };
        self.equipment_bonus(&stat, character)
            + self.obol_bonus(&stat, character)
            + self.nametag_bonus(&stat, character)
            + self.trophy_bonus(&stat, character)
            + self.hatrack_bonus(&stat, character)
            + self.capevine_bonus(&stat, character)
    }

    /// `equipment.resolve`: every gear slot's matching UQ stat.
    pub(super) fn equipment_bonus(&self, stat: &str, character: usize) -> f64 {
        let Some(order) = fields::field(self.save, &format!("EquipOrder_{character}")) else {
            return 0.0;
        };
        let gear = fields::field(self.save, &format!("EMm0_{character}"));
        let tools = fields::field(self.save, &format!("EMm1_{character}"));
        if gear.is_none() && tools.is_none() {
            return 0.0;
        }
        let grid_172 = self.grid_bonus(172);
        let grid_172_multi = if grid_172 > 0.0 {
            1.0 + grid_172 / 100.0
        } else {
            1.0
        };
        let gallery = self.gallery_on(character);
        let premhat = self.premhat_on(character);
        let capevine = self.capevine_on(character);
        let mut skip: Vec<usize> = Vec::new();
        if gallery {
            skip.extend([10, 14]);
        }
        if premhat {
            skip.push(8);
        }
        if capevine {
            skip.push(12);
        }
        let mut chips: Vec<usize> = Vec::new();
        if self.char_has_chip(character, "pend") {
            chips.push(3);
        }
        if self.char_has_chip(character, "key1") {
            chips.push(9);
        }
        if self.char_has_chip(character, "troph") {
            chips.push(10);
        }
        let mut total = 0.0;
        for (row, max_slot) in [(0usize, 15usize), (1, 7)] {
            let Some(order_row) = cell(&order, row) else {
                continue;
            };
            let emm_row = match row {
                0 => gear.clone(),
                _ => tools.clone(),
            };
            let Some(emm_row) = emm_row.as_ref() else {
                continue;
            };
            for slot in 0..=max_slot {
                if skip.contains(&slot) {
                    continue;
                }
                let Some(item_name) = cell(order_row, slot).and_then(Value::as_str) else {
                    continue;
                };
                let saved = cell(emm_row, slot);
                let mut value = 0.0;
                for uq in 1..=2 {
                    let built_in = self
                        .data
                        .items
                        .get(item_name)
                        .map(|item| {
                            let (stat, value) = if uq == 1 {
                                (&item.0, item.1)
                            } else {
                                (&item.2, item.3)
                            };
                            (stat.as_str(), value)
                        })
                        .filter(|(stat, _)| !stat.is_empty() && *stat != "0" && *stat != "Blank");
                    let saved_value = saved
                        .and_then(|row| row.get(format!("UQ{uq}val")))
                        .and_then(super::super::json::number)
                        .unwrap_or(0.0);
                    let effective = match &built_in {
                        Some((stat, _)) => Some((*stat).to_owned()),
                        None if saved_value > 0.0 => saved
                            .and_then(|row| row.get(format!("UQ{uq}txt")))
                            .and_then(Value::as_str)
                            .map(str::to_owned),
                        None => None,
                    };
                    if effective.as_deref() == Some(stat) {
                        value += built_in.map(|(_, value)| value).unwrap_or(0.0) + saved_value;
                    }
                }
                if value <= 0.0 {
                    continue;
                }
                if row == 0 && chips.contains(&slot) {
                    value *= 2.0;
                }
                if row == 0 && slot == 15 {
                    value *= grid_172_multi;
                }
                total += value;
            }
        }
        total
    }

    /// `obol.resolve`: personal and family obols with a matching built-in stat.
    pub(super) fn obol_bonus(&self, stat: &str, character: usize) -> f64 {
        let personal = self.scan_obols(
            fields::field(self.save, &format!("ObolEqO0_{character}")),
            fields::field(self.save, &format!("ObolEqMAP_{character}")),
            stat,
        );
        let family = self.scan_obols(
            fields::field(self.save, "ObolEqO1"),
            fields::field(self.save, "ObolEqMAPz1"),
            stat,
        );
        personal + family
    }

    fn scan_obols(&self, names: Option<Value>, maps: Option<Value>, stat: &str) -> f64 {
        let (Some(names), Some(maps)) = (names, maps) else {
            return 0.0;
        };
        let mut total = 0.0;
        for slot in 0..row_len(&names) {
            let Some(name) = cell(&names, slot).and_then(Value::as_str) else {
                continue;
            };
            if name.is_empty() || name == "Blank" || name == "Null" {
                continue;
            }
            let Some(item) = self.data.items.get(name) else {
                continue;
            };
            let built_in = if item.0 == stat {
                Some((1, item.1))
            } else if item.2 == stat {
                Some((2, item.3))
            } else {
                None
            };
            let Some((uq, value)) = built_in else {
                continue;
            };
            let rolled = cell(&maps, slot)
                .and_then(|row| row.get(format!("UQ{uq}val")))
                .and_then(super::super::json::number)
                .unwrap_or(0.0);
            total += value + rolled;
        }
        total
    }

    /// `nametag.resolve`: tier × gallery multiplier × UQ value.
    fn nametag_bonus(&self, stat: &str, character: usize) -> f64 {
        if !self.gallery_on(character) {
            return 0.0;
        }
        let levels = self.spelunk_row(17);
        let gallery = self.gallery_bonus_multi(character);
        let mut total = 0.0;
        for (index, level) in levels.iter().enumerate() {
            let level = level.unwrap_or(0.0);
            if level < 1.0 {
                continue;
            }
            let Some(entries) = self.data.nametags.get(&index.to_string()) else {
                continue;
            };
            let tier = self
                .data
                .nametag_tier
                .get((level as usize - 1).min(4))
                .copied()
                .unwrap_or(1.0);
            for (entry_stat, value) in entries {
                if entry_stat == stat {
                    total += tier * gallery * value;
                }
            }
        }
        total
    }

    /// `trophy.resolve`: slot tier × gallery multiplier × UQ value.
    fn trophy_bonus(&self, stat: &str, character: usize) -> f64 {
        if !self.gallery_on(character) {
            return 0.0;
        }
        let slots = self.spelunk_row(16);
        let gallery = self.gallery_bonus_multi(character);
        let mut total = 0.0;
        for (index, trophy) in slots.iter().enumerate() {
            let trophy_id = trophy.unwrap_or(0.0) as usize;
            if trophy_id < 1 {
                continue;
            }
            let Some(entries) = self.data.trophies.get(&trophy_id.to_string()) else {
                continue;
            };
            let tier = self.trophy_tier(index);
            for (entry_stat, value) in entries {
                if entry_stat == stat {
                    total += tier * gallery * value;
                }
            }
        }
        total
    }

    /// `premhat.resolve`: hatrack multiplier × UQ value.
    fn hatrack_bonus(&self, stat: &str, character: usize) -> f64 {
        if !self.premhat_on(character) {
            return 0.0;
        }
        let hats = fields::strings(self.save, "Spelunk", &[46]).unwrap_or_default();
        let hatrack = self.hatrack_bonus_multi();
        let mut total = 0.0;
        for name in &hats {
            let Some(entries) = self.data.hats.get(name) else {
                continue;
            };
            for (entry_stat, value) in entries {
                if entry_stat == stat {
                    total += hatrack * value;
                }
            }
        }
        total
    }

    /// `capevine.resolve`: cape vine multiplier × UQ value.
    fn capevine_bonus(&self, stat: &str, character: usize) -> f64 {
        if !self.capevine_on(character) {
            return 0.0;
        }
        let capes = fields::strings(self.save, "Spelunk", &[54]).unwrap_or_default();
        let mut total = 0.0;
        for (slot, name) in capes.iter().enumerate() {
            let Some(entries) = self.data.capes.get(name) else {
                continue;
            };
            let multi = self.capevine_bonus_multi(slot);
            for (entry_stat, value) in entries {
                if entry_stat == stat {
                    total += multi * value;
                }
            }
        }
        total
    }

    /// `galleryBonusMulti`.
    fn gallery_bonus_multi(&self, character: usize) -> f64 {
        let gallery_level = self.spelunk(13, 4);
        let trophy_chip = if self.chip_count(character, 16) > 0.0 {
            10.0
        } else {
            0.0
        };
        let clam_work7 = if self.ola(464) > 7.0 { 3.0 } else { 0.0 };
        let killroy = {
            let ola = self.ola(467);
            ola / (200.0 + ola) * 10.0
        };
        let bubble = self.bubble_y13().min(20.0);
        let card = self.card_level("w7a11").min(10.0);
        let companion = self.companion(49);
        let rog = self.rog_qty(54);
        let sum = 3.0 * gallery_level
            + trophy_chip
            + clam_work7
            + killroy
            + bubble
            + card
            + companion
            + rog;
        1.0 + sum / 100.0
    }

    /// `HatrackBonusMulti`.
    fn hatrack_bonus_multi(&self) -> f64 {
        let hats = fields::strings(self.save, "Spelunk", &[46])
            .map(|hats| hats.len() as f64)
            .unwrap_or(0.0);
        let companion = self.companion(31);
        let event = 10.0 * self.event_shop(30);
        let minehead = self.minehead_bonus(21);
        let rog = self.rog_qty(36);
        1.0 + (hats + companion + event + minehead + rog) / 100.0
    }

    /// `capevineBonusMulti`.
    fn capevine_bonus_multi(&self, slot: usize) -> f64 {
        let capes = fields::strings(self.save, "Spelunk", &[54])
            .map(|capes| capes.len() as f64)
            .unwrap_or(0.0);
        let golden_slots =
            (1.0 + self.ola(669) + 3.0 * if self.bundle("ban_o") { 1.0 } else { 0.0 }).round();
        let multi = 1.0 + (capes + self.companion(105)) / 100.0;
        if gold(golden_slots, slot) {
            multi
        } else {
            0.25 * multi
        }
    }

    /// `trophyTier`.
    fn trophy_tier(&self, slot: usize) -> f64 {
        if slot < 48 {
            return 0.3;
        }
        let offset = slot - 48;
        if self.podiums_owned_lv4() > offset as f64 {
            return 2.5;
        }
        if self.podiums_owned_lv3() > offset as f64 {
            return 2.0;
        }
        if self.podiums_owned_lv2() > offset as f64 {
            return 1.5;
        }
        1.0
    }

    fn podiums_owned_lv4(&self) -> f64 {
        let sail_33 = self.nested("Sailing", &[3, 33]);
        let companion_28 = self.companion(28).min(1.0);
        let companion_28_enhanced = if self.companion_level2(28) { 1.0 } else { 0.0 };
        let event = self.event_shop(29);
        (companion_28
            + companion_28_enhanced
            + (event
                + ((sail_33 / 6.0).floor().min(1.0)
                    + ((sail_33 / 7.0).floor().min(1.0)
                        + if self.bundle("ban_m") { 1.0 } else { 0.0 }))))
        .round()
    }

    fn podiums_owned_lv3(&self) -> f64 {
        let gems = self.row_at("GemItemsPurchased", 40);
        let sail_33 = self.nested("Sailing", &[3, 33]);
        ((gems / 3.0).floor() + (sail_33 / 5.0).floor().min(1.0) + self.podiums_owned_lv4()).round()
    }

    fn podiums_owned_lv2(&self) -> f64 {
        let gems = self.row_at("GemItemsPurchased", 40);
        let sail_33 = self.nested("Sailing", &[3, 33]);
        let companion_42 = self.companion(42);
        let clam_work0 = if self.ola(464) > 0.0 { 1.0 } else { 0.0 };
        let killroy = {
            let ola = self.ola(467);
            ola / (200.0 + ola) * 10.0
        };
        let legend = self.legend(9);
        let sail =
            ((sail_33.round() - 2.0).clamp(0.0, 2.0) - (sail_33 / 5.0).floor().min(1.0)).max(0.0);
        (2.0 * clam_work0
            + killroy.min(2.0)
            + companion_42
            + (gems / 2.0).floor()
            + self.podiums_owned_lv3()
            + legend
            + sail)
            .round()
    }

    /// `galleryBonusSystemOn`: KLA map 250 cleared.
    fn gallery_on(&self, character: usize) -> bool {
        self.map_cleared(250, character)
    }

    /// `premHatBonusSystemOn`: KLA map 50 cleared.
    fn premhat_on(&self, character: usize) -> bool {
        self.map_cleared(50, character)
    }

    /// `capevineBonusSystemOn`: KLA map 100 cleared.
    fn capevine_on(&self, character: usize) -> bool {
        self.map_cleared(100, character)
    }

    fn map_cleared(&self, map: usize, character: usize) -> bool {
        let Some(value) = fields::char_field(self.save, "KLA", character) else {
            return false;
        };
        let Some(row) = super::super::json::index(&value, &[map]) else {
            return false;
        };
        let kills = super::super::json::number(row)
            .or_else(|| {
                row.as_array()
                    .and_then(|cells| cells.first())
                    .and_then(super::super::json::number)
            })
            .unwrap_or(0.0);
        kills <= 0.0
    }

    /// `charHasChip`.
    fn char_has_chip(&self, character: usize, effect: &str) -> bool {
        fields::row(self.save, "Lab", &[1 + character])
            .map(|chips| {
                chips.iter().flatten().any(|chip| {
                    *chip >= 0.0
                        && self
                            .data
                            .chip_effects
                            .get(*chip as usize)
                            .is_some_and(|key| key == effect)
                })
            })
            .unwrap_or(false)
    }
}

/// How many slots a row carries, whether it is an array or a sparse object.
fn row_len(value: &Value) -> usize {
    match value {
        Value::Array(items) => items.len(),
        Value::Object(fields) => fields
            .keys()
            .filter_map(|key| key.parse::<usize>().ok())
            .max()
            .map_or(0, |last| last + 1),
        _ => 0,
    }
}

/// One slot of a row the game may write as an array or as a sparse object.
fn cell(value: &Value, slot: usize) -> Option<&Value> {
    match value {
        Value::Array(items) => items.get(slot),
        Value::Object(fields) => fields.get(&slot.to_string()),
        _ => None,
    }
}

fn gold(golden_slots: f64, slot: usize) -> bool {
    slot == 999 || golden_slots > slot as f64
}
