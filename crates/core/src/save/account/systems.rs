//! The small upstream systems the research multiplier tree reads.
//!
//! Each function ports one lookup from `js/stats/systems` or
//! `js/stats/data`; the comment above it names the upstream source. The save
//! accessors return neutral values when a save lacks the field, and the caller
//! warns through [`Decoder::omission`] where the term matters.

use super::Decoder;
use super::fields;
use crate::data::game;
use serde_json::Value;

impl Decoder<'_> {
    /// A number from `OptLacc`, the account's option list.
    pub(super) fn ola(&self, index: usize) -> f64 {
        fields::field(self.save, "OptLacc")
            .and_then(|value| {
                super::super::json::index(&value, &[index]).and_then(super::super::json::number)
            })
            .unwrap_or(0.0)
    }

    /// `OptLacc` as a string, as the game sometimes writes lists there.
    pub(super) fn ola_text(&self, index: usize) -> String {
        fields::field(self.save, "OptLacc")
            .and_then(|value| {
                super::super::json::index(&value, &[index]).map(|cell| match cell {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
            })
            .unwrap_or_default()
    }

    /// `Research[row][column]`.
    pub(super) fn research(&self, row: usize, column: usize) -> Option<f64> {
        fields::at(self.save, "Research", &[row, column])
    }

    /// `Spelunk[row][column]`.
    pub(super) fn spelunk(&self, row: usize, column: usize) -> f64 {
        fields::at(self.save, "Spelunk", &[row, column]).unwrap_or(0.0)
    }

    /// A row of `Spelunk`.
    pub(super) fn spelunk_row(&self, row: usize) -> Vec<Option<f64>> {
        fields::row(self.save, "Spelunk", &[row]).unwrap_or_default()
    }

    /// A row of a top-level array field.
    pub(super) fn row(&self, key: &str) -> Vec<Option<f64>> {
        fields::numbers(self.save, key).unwrap_or_default()
    }

    /// `value || 0` from a row.
    pub(super) fn row_at(&self, key: &str, index: usize) -> f64 {
        fields::number_at(&self.row(key), index)
    }

    /// A row inside a nested field.
    pub(super) fn nested_row(&self, key: &str, path: &[usize]) -> Vec<Option<f64>> {
        fields::row(self.save, key, path).unwrap_or_default()
    }

    /// A nested number.
    pub(super) fn nested(&self, key: &str, path: &[usize]) -> f64 {
        fields::at(self.save, key, path).unwrap_or(0.0)
    }

    /// An array's length.
    pub(super) fn array_len(&self, key: &str) -> usize {
        fields::len_of(self.save, key)
    }

    /// The save constant scaling the research EXP curve. Upstream defaults to
    /// 1.01 when the wrapper has no `serverVars`; the value is not guessed.
    pub(super) fn server_var(&mut self) -> f64 {
        if let Some(value) = self
            .save
            .get("serverVarResXP")
            .and_then(super::super::json::number)
        {
            return value.max(1.01);
        }
        if let Some(value) = self
            .root
            .get("serverVars")
            .and_then(|vars| vars.get("A_ResXP"))
            .and_then(super::super::json::number)
        {
            return value.max(1.01);
        }
        // Upstream defaults the curve constant to 1.01 when the wrapper has no
        // `serverVars`; the value is a documented default, not a guess.
        1.01
    }

    /// `eventShopOwned` / `buildEventShopArray` from `js/game-helpers.js`.
    pub(super) fn event_shop_flags(&self) -> Vec<u8> {
        let text = self.ola_text(311);
        encoded_flags(&text, &self.data.encoding)
    }

    /// One event shop item.
    pub(super) fn event_shop(&self, index: usize) -> f64 {
        self.event_shop_flags().get(index).copied().unwrap_or(0) as f64
    }

    /// One superbit.
    pub(super) fn superbit(&self, index: usize) -> f64 {
        self.superbit_flags().get(index).copied().unwrap_or(0) as f64
    }

    /// One emporium bonus.
    pub(super) fn emporium(&self, index: usize) -> f64 {
        self.emporium_flags().get(index).copied().unwrap_or(0) as f64
    }

    /// `superBitType` / `buildSuperBitArray` from `js/game-helpers.js`.
    pub(super) fn superbit_flags(&self) -> Vec<u8> {
        let text = fields::field(self.save, "Gaming")
            .and_then(|value| {
                super::super::json::index(&value, &[12]).map(|cell| match cell {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
            })
            .unwrap_or_default();
        encoded_flags(&text, &self.data.encoding)
    }

    /// `emporiumBonus` / `buildEmporiumArray` from `js/game-helpers.js`.
    pub(super) fn emporium_flags(&self) -> Vec<u8> {
        let text = fields::field(self.save, "Ninja")
            .and_then(|value| {
                super::super::json::index(&value, &[102, 9]).map(|cell| match cell {
                    Value::String(text) => text.clone(),
                    other => other.to_string(),
                })
            })
            .unwrap_or_default();
        encoded_flags(&text, &self.data.encoding)
    }

    /// One minehead floor bonus.
    pub(super) fn minehead_bonus(&self, index: usize) -> f64 {
        self.minehead().get(index).copied().unwrap_or(0.0)
    }

    /// `buildMhqArray` from `js/stats/systems/w7/minehead.js`.
    pub(super) fn minehead(&self) -> Vec<f64> {
        let floor = self
            .research(7, 4)
            .or_else(|| {
                self.save.get("stateR7").and_then(|value| {
                    super::super::json::index(value, &[4]).and_then(super::super::json::number)
                })
            })
            .unwrap_or(0.0);
        self.data
            .minehead_bonus
            .iter()
            .enumerate()
            .map(|(index, value)| if floor > index as f64 { *value } else { 0.0 })
            .collect()
    }

    /// `computeUniqueSushi` from `js/stats/systems/w7/sushi.js`.
    pub(super) fn unique_sushi(&self) -> Option<u32> {
        let tiers = self.save.get("Sushi").and_then(|value| {
            let value = match value {
                Value::String(_) => super::super::json::parsed_field(self.save, "Sushi")?,
                other => other.clone(),
            };
            super::super::json::index(&value, &[5]).cloned()
        })?;
        let tiers = tiers.as_array()?;
        let mut count = 0;
        for tier in tiers {
            if super::super::json::number(tier).unwrap_or(0.0) < 0.0 {
                break;
            }
            count += 1;
        }
        Some(count)
    }

    /// `buildRogArray` from `js/stats/systems/w7/sushi.js`.
    pub(super) fn rog(&self) -> Vec<f64> {
        let unique = self.unique_sushi().unwrap_or(0);
        self.data
            .rog_bonus
            .iter()
            .enumerate()
            .map(|(index, value)| if unique > index as u32 { *value } else { 0.0 })
            .collect()
    }

    /// `rogBonusQTY`.
    pub(super) fn rog_qty(&self, index: usize) -> f64 {
        let unique = self.unique_sushi().unwrap_or(0);
        if unique > index as u32 {
            self.data.rog_bonus.get(index).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// `jellyRewardBonus` from `js/stats/data/w7/jelly-operator.js`: the
    /// obstruction is `Research[7][9]`.
    pub(super) fn jelly(&self, index: usize) -> f64 {
        let obstruction = self.research(7, 9).unwrap_or(0.0);
        if obstruction > index as f64 {
            self.data.jelly_reward.get(index).copied().unwrap_or(0.0)
        } else {
            0.0
        }
    }

    /// `jellyRewardBonus(0/23/46/54)` folded into one factor.
    pub(super) fn jelly_research_multi(&self) -> f64 {
        [0, 23, 46, 54].into_iter().fold(1.0, |multi, index| {
            multi * (1.0 + self.jelly(index) / 100.0)
        })
    }

    /// `menagerieUnlockOwned` from `js/stats/systems/w7/menagerie.js`.
    pub(super) fn menagerie_unlock(&self, index: usize) -> f64 {
        let row = self.nested_row("Menagerie", &[2]);
        if row.is_empty() {
            return 0.0;
        }
        if fields::number_at(&row, index) >= 1.0 {
            1.0
        } else {
            0.0
        }
    }

    /// `menagerieBoostLevel`.
    fn menagerie_boost_level(&self, index: usize) -> f64 {
        fields::number_at(&self.nested_row("Menagerie", &[3]), index)
    }

    /// `menagerieBoostQTY`.
    pub(super) fn menagerie_boost_qty(&self, index: usize) -> f64 {
        if self.nested_row("Menagerie", &[3]).is_empty() {
            return 0.0;
        }
        self.data.menagerie_boost.get(index).copied().unwrap_or(0.0)
            * self.menagerie_boost_level(index)
    }

    /// `hasBundle` from `js/stats/systems/w7/menagerie.js`.
    pub(super) fn bundle(&self, key: &str) -> bool {
        let Some(bundles) = fields::object(self.save, "BundlesReceived") else {
            return false;
        };
        let holder = match bundles.get("h") {
            Some(Value::Object(inner)) => inner,
            _ => &bundles,
        };
        holder
            .get(key)
            .and_then(super::super::json::number)
            .is_some_and(|value| value == 1.0)
    }

    /// `companionBonusForSave`.
    pub(super) fn companion(&self, index: usize) -> f64 {
        let roster = self.companions();
        match roster.iter().find(|(id, _)| *id == index as u32) {
            Some((_, enhanced)) => self.data.companion(index, *enhanced),
            None => 0.0,
        }
    }

    /// `companionLevel2`.
    pub(super) fn companion_level2(&self, index: usize) -> bool {
        self.companions()
            .iter()
            .any(|(id, enhanced)| *id == index as u32 && *enhanced)
    }

    /// Companion ids owned and whether each is enhanced, from
    /// `companion.l` plus the `OptLacc[606]` token list.
    fn companions(&self) -> Vec<(u32, bool)> {
        if let Some(companions) = self
            .companions_from_list()
            .or_else(|| self.companions_from_tokens())
        {
            return companions;
        }
        Vec::new()
    }

    fn companions_from_list(&self) -> Option<Vec<(u32, bool)>> {
        let list = self.root.get("companion")?.get("l")?;
        let entries: Vec<String> = match list {
            Value::Array(items) => items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect(),
            Value::Object(fields) => {
                let mut keys: Vec<&String> = fields.keys().collect();
                keys.sort_by_key(|key| key.parse::<u64>().unwrap_or(u64::MAX));
                keys.iter()
                    .filter_map(|key| fields.get(*key).and_then(Value::as_str))
                    .map(str::to_owned)
                    .collect()
            }
            _ => return None,
        };
        let mut roster = Vec::new();
        for entry in entries {
            let fields: Vec<&str> = entry.split(',').collect();
            let Some(id) = fields.first().and_then(|id| id.trim().parse::<u32>().ok()) else {
                continue;
            };
            let level_two = fields
                .get(4)
                .and_then(|flag| flag.trim().parse::<f64>().ok())
                .unwrap_or(0.0);
            roster.push((id, level_two >= 1.0));
        }
        Some(roster)
    }

    fn companions_from_tokens(&self) -> Option<Vec<(u32, bool)>> {
        let tokens = self.ola_text(606);
        if tokens.is_empty() || tokens == "0" {
            return None;
        }
        let roster: Vec<(u32, bool)> = tokens
            .split(',')
            .filter_map(|entry| entry.trim().parse::<u32>().ok())
            .filter(|id| (*id as usize) < self.data.companion_bonus.len())
            .map(|id| (id, false))
            .collect();
        (!roster.is_empty()).then_some(roster)
    }

    /// `calcAllBonusMultiWith` from `js/stats/systems/w7/research-math.js`.
    pub(super) fn all_bonus_multi(&self) -> f64 {
        let companion_55 = self.companion(55);
        let companion_0 = if self.companion(0) > 0.0
            && self
                .characters
                .first()
                .and_then(|character| character.divinity())
                .is_some_and(|divinity| divinity >= 2)
            && self.grid.level(173) > 0
        {
            5.0
        } else {
            0.0
        };
        let clouds = self.cloud_bonus(71) + self.cloud_bonus(72) + self.cloud_bonus(76);
        1.0 + (companion_55 + companion_0 + clouds + self.rog_qty(53)) / 100.0
    }

    /// `comp52TrueMulti` from `save/loader.js`.
    pub(super) fn companion_52(&self) -> f64 {
        (1.0 + self.companion(52))
            * (1.0 + self.companion(153) + if self.companion_level2(153) { 1.0 } else { 0.0 })
    }

    /// `cachedStickerFixed`.
    pub(super) fn sticker_fixed(&self) -> f64 {
        let level = self.research(9, 1).unwrap_or(0.0);
        let base = self.data.sticker_base.get(1).copied().unwrap_or(5.0);
        let superbit_62 = 1.0 + 20.0 * self.superbit(62) / 100.0;
        superbit_62 * level * base
    }

    /// `cachedBoonyCount`.
    pub(super) fn boony_count(&self) -> f64 {
        fields::row(self.save, "Research", &[11])
            .map(|row| row.len() as f64)
            .unwrap_or(0.0)
    }

    /// `Button_Bonuses(0)` and the same value without grid 125, so the
    /// simulator can apply grid changes live (`recomputeDerivedBonuses`).
    pub(super) fn button0(&self) -> (f64, f64) {
        let presses = self.ola(594);
        if presses <= 0.0 {
            return (0.0, 0.0);
        }
        let companion_147 = self.companion(147);
        let grid_125 = self.grid_bonus(125);
        let base_multi = 1.0 + companion_147 / 100.0;
        let full_multi = base_multi * (1.0 + grid_125 / 100.0);
        let slot0 = (presses / 45.0).floor() * 5.0 + (presses % 45.0).min(5.0);
        (slot0 * 2.0 * base_multi, slot0 * 2.0 * full_multi)
    }

    /// `KillroyBonuses(5)` from `js/stats/defs/helpers.js`.
    pub(super) fn killroy5(&self) -> f64 {
        let ola = self.ola(469);
        1.0 + ola / (150.0 + ola) * 0.8
    }

    /// `Dream[14]`, the Nonstop Studies level.
    pub(super) fn dream14(&self) -> f64 {
        self.row_at("Dream", 14)
    }

    /// `Cglunko_upgBon(11)`.
    pub(super) fn cglunko11(&self) -> f64 {
        self.ola(641) * 1.0
    }

    /// `Fountain_BonTOT(2,16)`.
    pub(super) fn fountain_2_16(&self) -> f64 {
        let level = fields::at(self.save, "Holes", &[31, 2, 16])
            .unwrap_or(0.0)
            .max(0.0);
        let master_level = fields::at(self.save, "Holes", &[32, 2, 16]).unwrap_or(0.0);
        let master = if master_level <= 0.0 {
            1.0
        } else {
            1.5 + 0.5 * master_level
        };
        (master * level).round()
    }
}

/// `str.includes(N2L[t])` for every index in `N2L`.
fn encoded_flags(text: &str, encoding: &str) -> Vec<u8> {
    (0..encoding.chars().count())
        .map(|index| {
            encoding
                .chars()
                .nth(index)
                .is_some_and(|letter| text.contains(letter)) as u8
        })
        .collect()
}

/// The grid node list, used by tests to name nodes.
#[allow(dead_code)]
fn node_name(cell: usize) -> Option<&'static str> {
    game().node_at(cell).map(|node| node.name.as_str())
}

impl Decoder<'_> {}
