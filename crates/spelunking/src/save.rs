//! The raw save fields the Spelunking inputs read.
//!
//! Ported from `buildSpelunkInputs` and `buildSpelunkStaticInputs` in
//! `js/stats/systems/w7/spelunk-inputs.js`: the decoded `Spelunk` rows, the
//! account options and gem purchases, the character roster, and the
//! `shopUpgBonus` array the depth formulas index. A save without the stamina
//! and shop rows carries no expedition state, so [`Save::from_parts`] returns
//! `None` and the section shows its import prompt instead of zeros.

use super::{Account, Character, Inputs, data, game_log, js_round};
use idleon_common::curve as formula;
use idleon_save::{json, number, row_values};
use serde_json::Value;

/// The decoded save, narrowed to the rows the Spelunking formulas read.
#[derive(Clone, Debug, Default)]
pub struct Save {
    /// `Spelunk`: `[0]` boss unlocks, `[1]` normal records, `[3]` stamina,
    /// `[4]` flags, `[5]` shop levels, `[6]` discoveries, `[7]` saved belt,
    /// `[47]` Manic records, `[48]` memorized elixirs.
    rows: Vec<Vec<Value>>,
    /// `GemItemsPurchased`: `[43]` Auto speed, `[44]` amber bags.
    gem_items: Vec<Value>,
    /// `OptionsListAccount`: `[478]` tutorial progress, `[538]` full-clear.
    options: Vec<Value>,
    characters: Vec<Row>,
    /// `shopUpgBonus` for all 80 rows.
    shop: Vec<f64>,
    /// `Gaming[12]`: the Super Bit keys the account owns.
    gaming: String,
    /// `Sailing[3][35]`: the Pointagon tier `chapterBonus` reads.
    pointagon: f64,
    /// Menagerie unlock 1, which the saved shortcut entry needs.
    shortcut: bool,
    /// Whether the wrapper carried the companion roster.
    partial: bool,
}

#[derive(Clone, Debug, Default)]
struct Row {
    id: usize,
    name: String,
    class_id: u32,
    level: f64,
    skill: f64,
    stamina: f64,
}

impl Save {
    /// Read the section state from a decoded save, if it carries one.
    pub fn from_parts(save: &idleon_save::Save) -> Option<Self> {
        let spelunk = save.get("Spelunk")?;
        let rows: Vec<Vec<Value>> = row_values(&spelunk).iter().map(row_values).collect();
        // Upstream requires the stamina row and the shop row before it simulates.
        if rows.len() <= 5 || rows[3].is_empty() || rows[5].is_empty() {
            return None;
        }
        let gem_items = save
            .get("GemItemsPurchased")
            .map(|value| row_values(&value))
            .unwrap_or_default();
        let options = save
            .get("OptionsListAccount")
            .or_else(|| save.get("OptLacc"))
            .map(|value| row_values(&value))
            .unwrap_or_default();
        let count = save.character_count().max(10);
        let characters: Vec<Row> = (0..count)
            .map(|id| {
                let levels = save
                    .get(&format!("Lv0_{id}"))
                    .map(|value| row_values(&value))
                    .unwrap_or_default();
                Row {
                    id,
                    name: save.character_name(id),
                    class_id: save
                        .get(&format!("CharacterClass_{id}"))
                        .and_then(|value| number(&value))
                        .unwrap_or(0.0) as u32,
                    level: levels.first().and_then(number).unwrap_or(0.0),
                    skill: levels.get(19).and_then(number).unwrap_or(0.0),
                    stamina: rows[3].get(id).and_then(number).unwrap_or(0.0),
                }
            })
            .collect();
        let shop = shop_upgrades(&rows, &characters);
        let partial = !companion_roster(save);
        let sailing = save.get("Sailing");
        let pointagon = sailing
            .as_ref()
            .and_then(|sailing| json::index(sailing, &[3, 35]))
            .and_then(number)
            .unwrap_or(0.0)
            .max(0.0);
        let menagerie = save.get("Menagerie");
        let shortcut = menagerie
            .as_ref()
            .and_then(|menagerie| json::index(menagerie, &[2, 1]))
            .and_then(number)
            .is_some_and(|owned| owned >= 1.0);
        Some(Self {
            rows,
            gem_items,
            options,
            characters,
            shop,
            gaming: field_text(save, "Gaming", 12),
            pointagon,
            shortcut,
            partial,
        })
    }

    /// `buildSpelunkInputs`, with the account inputs the stat context would supply.
    pub fn inputs(&self, account: &Account) -> Inputs {
        let spelunk = data();
        let characters = self
            .characters
            .iter()
            .map(|row| {
                let max_stamina = account
                    .max_stamina
                    .get(row.id)
                    .copied()
                    .filter(|max| max.is_finite() && *max >= 0.0)
                    .unwrap_or(row.stamina);
                Character {
                    id: row.id,
                    name: row.name.clone(),
                    class_id: row.class_id,
                    level: row.level,
                    stamina: row.stamina.clamp(0.0, max_stamina),
                    max_stamina,
                }
            })
            .collect();
        let mut unlocked = 1;
        for index in 0..9 {
            if self.cell(0, index) >= 1.0 {
                unlocked = index + 2;
            }
        }
        let caves = spelunk
            .caves
            .iter()
            .take(unlocked.min(spelunk.caves.len()))
            .cloned()
            .collect();
        let tools = spelunk
            .tools
            .iter()
            .filter(|tool| tool.id == 0 || self.shop_at(13 + tool.id) >= 1.0)
            .cloned()
            .collect();
        let elixir_owned =
            (js_round(self.shop_at(23).min(12.0)).max(0.0) as usize).min(spelunk.elixirs.len());
        let slots = js_round((1.0 + self.shop_at(24)).min(5.0)).clamp(0.0, 5.0) as usize;
        let dupes = js_round(self.shop_at(25).min(5.0)).clamp(0.0, 5.0) as usize;
        let memory_slots = js_round(self.shop_at(57)).max(0.0) as usize;
        let saved_elixirs: Vec<i32> = (0..5)
            .map(|slot| {
                self.rows
                    .get(7)
                    .and_then(|row| row.get(slot))
                    .and_then(number)
                    .filter(|id| id.is_finite())
                    .map_or(-1, |id| id as i32)
            })
            .collect();
        let memorized: Vec<usize> = self
            .rows
            .get(48)
            .map(|row| {
                row.iter()
                    .take(memory_slots)
                    .filter_map(number)
                    .filter(|id| id.is_finite() && *id >= 0.0)
                    .map(|id| id as usize)
                    .collect()
            })
            .unwrap_or_default();
        Inputs {
            characters,
            caves,
            tools,
            elixirs: spelunk.elixirs.clone(),
            shop: self.shop.clone(),
            power: account.power,
            holes_bonus: account.holes_bonus,
            amber: account.amber,
            regen: account.regen,
            grand: account.grand,
            elixir_owned,
            slots,
            dupes,
            memory_slots,
            nonconsume: self.nonconsume(),
            overuse: !spelunk.overuse_super_bit.is_empty()
                && self.gaming.contains(&spelunk.overuse_super_bit),
            switch_nova: self.cell(5, 38) >= 1.0,
            bags: self.gem_item(44).max(0.0),
            prisma: self.cell(4, 4),
            exalt: self.cell(4, 3),
            shortcut: self.shortcut,
            saved_elixirs,
            memorized,
            saved_manic: self.cell(4, 10) == 1.0,
            full_clear: self.option(538) == 1.0,
            auto: self.shop_at(12) == 1.0 || self.gem_item(43) >= 1.0,
            speed_max: self.gem_item(43).floor().max(0.0) as u32,
            manic: self.shop_at(53) >= 1.0,
            depths: self.row(1),
            manic_depths: self.row(47),
            bosses: self.row(0),
            grands: self.row(44),
            discoveries: self
                .rows
                .get(6)
                .map(|row| {
                    row.iter()
                        .filter_map(Value::as_str)
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            partial: self.partial,
        }
    }

    /// `min(.6, (shop[26] + chapterBonus(3, 2)) / 100)`: the chance a used
    /// elixir charge survives.
    fn nonconsume(&self) -> f64 {
        ((self.shop_at(26) + self.chapter_bonus(3, 2)) / 100.0).min(0.6)
    }

    /// `chapterBonus`: `Spelunk[8]` holds one level per chapter bonus, and a row
    /// flagged with the artifact scales by Pointagon's bonus.
    fn chapter_bonus(&self, chapter: usize, bonus: usize) -> f64 {
        let Some(row) = data()
            .chapters
            .get(chapter)
            .and_then(|rows| rows.get(bonus))
        else {
            return 0.0;
        };
        let level = self.cell(8, 4 * chapter + bonus);
        if level <= 0.0 {
            return 0.0;
        }
        let artifact = if row.artifact == 1.0 {
            1.0 + idleon_sailing::artifact_bonus(35, self.pointagon as u32) / 100.0
        } else {
            1.0
        };
        artifact * formula::eval(&row.kind, row.x1, row.x2, level)
    }

    /// `Spelunk[row][column]`, through the number coercion the formulas use.
    fn cell(&self, row: usize, column: usize) -> f64 {
        self.rows
            .get(row)
            .and_then(|row| row.get(column))
            .and_then(number)
            .unwrap_or(0.0)
    }

    /// A whole `Spelunk` row as numbers, blanks as zero.
    fn row(&self, index: usize) -> Vec<f64> {
        self.rows
            .get(index)
            .map(|row| row.iter().map(|cell| number(cell).unwrap_or(0.0)).collect())
            .unwrap_or_default()
    }

    fn shop_at(&self, index: usize) -> f64 {
        self.shop.get(index).copied().unwrap_or(0.0)
    }

    fn gem_item(&self, index: usize) -> f64 {
        self.gem_items.get(index).and_then(number).unwrap_or(0.0)
    }

    fn option(&self, index: usize) -> f64 {
        self.options.get(index).and_then(number).unwrap_or(0.0)
    }
}

/// `companionDataAvailable`: the wrapper carries a companion roster.
fn companion_roster(save: &idleon_save::Save) -> bool {
    save.get("companion")
        .as_ref()
        .and_then(|companion| companion.get("l"))
        .is_some_and(|list| list.is_array() || list.is_object())
}

/// One decoded row cell as text, for the encoded Super Bit lists.
fn field_text(save: &idleon_save::Save, field: &str, index: usize) -> String {
    save.get(field)
        .and_then(|value| json::index(&value, &[index]).cloned())
        .map(|cell| match cell {
            Value::String(text) => text,
            other => other.to_string(),
        })
        .unwrap_or_default()
}

/// `shopUpgBonus` for every row: `SpelunkUpg[idx][4] * level`, with the
/// account-wide multipliers the upstream function applies.
fn shop_upgrades(rows: &[Vec<Value>], characters: &[Row]) -> Vec<f64> {
    let per_level = &data().upgrade_per_level;
    let total_spelunk_level: f64 = characters.iter().map(|row| row.skill.max(0.0)).sum();
    let discoveries = rows.get(6).map_or(0, Vec::len);
    let depths: Vec<f64> = rows
        .get(1)
        .map(|row| row.iter().map(|cell| number(cell).unwrap_or(0.0)).collect())
        .unwrap_or_default();
    let total_depth: f64 = depths.iter().sum();
    let mut haul_digits = 0.0f64;
    for haul in rows.get(2).map(Vec::as_slice).unwrap_or(&[]) {
        let haul = number(haul).unwrap_or(0.0);
        haul_digits = js_round(haul_digits + game_log(haul).ceil());
    }
    let grands: f64 = rows
        .get(44)
        .map(|row| row.iter().filter_map(number).sum())
        .unwrap_or(0.0);
    let mut row47_sum = 0.0f64;
    for cell in rows
        .get(47)
        .map(Vec::as_slice)
        .unwrap_or(&[])
        .iter()
        .take(depths.len())
    {
        row47_sum = js_round(row47_sum + number(cell).unwrap_or(0.0));
    }
    let flag4_8 = rows
        .get(4)
        .and_then(|row| row.get(8))
        .and_then(number)
        .unwrap_or(0.0);
    (0..80)
        .map(|index| {
            let level = rows
                .get(5)
                .and_then(|row| row.get(index))
                .and_then(number)
                .unwrap_or(0.0)
                .max(0.0);
            let mut value = per_level.get(index).copied().unwrap_or(0.0) * level;
            match index {
                0 => value *= total_spelunk_level,
                8 => value *= 1.0 + total_spelunk_level / 100.0,
                1 | 10 => value *= 1.0 + 5.0 * discoveries as f64 / 100.0,
                2 => value *= 2.0 + total_depth / 100.0,
                9 => value *= 1.0 + total_depth / 100.0,
                3 => value *= 1.0 + 3.0 * haul_digits / 100.0,
                7 => value = 15.0 + value / (250.0 + value) * 45.0,
                19 => value = (50.0 - value).max(10.0),
                25 => value += 1.0,
                38 => value += 20.0,
                44..=46 => value *= grands,
                54..=56 => value *= 1.0 + 5.0 * row47_sum / 100.0,
                59..=61 => value *= flag4_8,
                _ => {}
            }
            value
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    /// The shape the harness's small save uses: ten characters, four unlocked
    /// caves and every unlock on the shop rows the section reads.
    fn small_save() -> Value {
        let mut rows: Vec<Value> = (0..58).map(|_| json!([])).collect();
        rows[0] = json!([1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        rows[1] = json!([42, 12, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        rows[3] = json!([100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100, 100]);
        let mut flags = vec![0; 30];
        flags[10] = 1;
        rows[4] = json!(flags);
        let mut shop = vec![0; 68];
        shop[12] = 1;
        shop[23] = 12;
        shop[24] = 4;
        shop[25] = 5;
        for slot in shop.iter_mut().take(18).skip(14) {
            *slot = 1;
        }
        shop[53] = 1;
        shop[57] = 2;
        rows[5] = json!(shop);
        rows[6] = json!(["Smol_Pebbles", "Smooth_Rocks"]);
        rows[7] = json!([0, 2, 7, -1, -1]);
        let mut manic = vec![0; 15];
        manic[0] = 9;
        rows[47] = json!(manic);
        rows[48] = json!([7]);
        let mut save = json!({
            "Research": "[]",
            "Spelunk": Value::Array(rows).to_string(),
            "OptionsListAccount": json!({
                "0": 0,
                "478": 15,
                "538": 1,
            }).to_string(),
            "GemItemsPurchased": json!({ "43": 4 }).to_string(),
        });
        let lv0 = json!({
            "0": 500,
            "19": 10,
        });
        for id in 0..10 {
            save[format!("Lv0_{id}")] = Value::String(lv0.to_string());
            save[format!("CharacterClass_{id}")] = Value::String((10 + id).to_string());
        }
        save
    }

    fn read(save: &Value) -> Inputs {
        Save::from_parts(&idleon_save::Save::new(save.clone()))
            .expect("the stamina and shop rows are present")
            .inputs(&Account::neutral())
    }

    #[test]
    fn the_harness_shape_reads_every_unlock() {
        let inputs = read(&small_save());
        assert_eq!(inputs.characters.len(), 10);
        assert_eq!(inputs.characters[0].name, "Character 1");
        assert_eq!(inputs.characters[0].stamina, 100.0);
        assert_eq!(inputs.characters[0].max_stamina, 100.0);
        assert_eq!(inputs.caves.len(), 5);
        assert_eq!(inputs.caves[0].name, "Pebble Cove");
        assert_eq!(inputs.tools.len(), 5);
        assert_eq!(inputs.elixir_owned, 12);
        assert_eq!(inputs.slots, 5);
        assert_eq!(inputs.dupes, 5);
        assert_eq!(inputs.memory_slots, 2);
        assert_eq!(inputs.saved_elixirs, vec![0, 2, 7, -1, -1]);
        assert_eq!(inputs.memorized, vec![7]);
        assert!(inputs.saved_manic);
        assert!(inputs.full_clear);
        assert!(inputs.auto);
        assert_eq!(inputs.speed_max, 4);
        assert!(inputs.manic);
        assert_eq!(inputs.depths[0], 42.0);
        assert_eq!(inputs.manic_depths[0], 9.0);
        assert!(
            inputs.partial,
            "the fixture shape carries no companion list"
        );
    }

    #[test]
    fn a_save_without_the_stamina_row_has_no_expedition_state() {
        let mut save = small_save();
        let mut rows: Vec<Value> = serde_json::from_str(save["Spelunk"].as_str().unwrap()).unwrap();
        rows[3] = json!([]);
        save["Spelunk"] = Value::String(Value::Array(rows).to_string());
        assert!(Save::from_parts(&idleon_save::Save::new(save)).is_none());
    }
}
