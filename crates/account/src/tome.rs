//! The Tome score, ported from `js/stats/systems/w4/tome-score.js`.
//!
//! The research tree reads the Tome score for its lore episode bonus. Every
//! slot is computed from the save field the game stores it in. Two upstream
//! steps are not ported and are warned about instead of guessed: the star
//! talent total needs the talent system, and the live drop-rate refresh needs
//! the combat stats.

use super::Decoder;
use idleon_save::Character;
use idleon_save::fields;
use serde_json::Value;

impl Decoder<'_> {
    /// `computeTomeScore`, phase 1.
    pub(crate) fn tome_points(&mut self) -> Option<f64> {
        if fields::field(self.save, "Cards0").is_none()
            && fields::field(self.save, "Cards1").is_none()
            && fields::field(self.save, "StampLv").is_none()
        {
            self.warn(
                "Cards0",
                "the save carries no tome inputs; the tome score stays at 0",
            );
            return None;
        }
        let mut total = 0.0;
        for (slot, (half, mode, points)) in self.data.tome.iter().copied().enumerate() {
            let qty = if slot == 15 {
                self.warn(
                    "SL_<char>",
                    "the star talent tome slot needs the talent system; left at 0",
                );
                0.0
            } else {
                self.tome_qty(slot)
            };
            total += (tome_pct(qty, mode, half) * points).ceil();
        }
        self.warn(
            "PVStatList_<char>",
            "the live drop-rate refresh of the tome score is not recomputed; the saved value is used",
        );
        Some(total)
    }

    /// `tomeQTY`: the raw quantity behind one slot.
    fn tome_qty(&self, slot: usize) -> f64 {
        if slot < 64 {
            return self.tome_qty_low(slot);
        }
        self.tome_qty_high(slot)
    }

    fn tome_qty_low(&self, slot: usize) -> f64 {
        let ola = |index: usize| self.ola(index);
        let sum =
            |values: Vec<Option<f64>>| -> f64 { values.iter().filter_map(|value| *value).sum() };
        match slot {
            0 => self.stamp_total(),
            1 => self.statue_total(),
            2 => self.card_total(),
            3 => self.talent_max_total(),
            4 => self.quest_total(),
            5 => self.characters.iter().map(Character::class_level).sum(),
            6 => self.task_total("TaskZZ1"),
            7 => fields::field(self.save, "AchieveReg")
                .and_then(|value| value.as_array().cloned())
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| idleon_save::json::number(item) == Some(-1.0))
                        .count() as f64
                })
                .unwrap_or(0.0),
            8 => ola(198),
            9 => ola(208),
            10 => self.cards1_prefix("Trophy"),
            11 => self
                .characters
                .iter()
                .map(|character| {
                    (1..=21)
                        .map(|skill| character.skill(skill - 1).max(0.0))
                        .sum::<f64>()
                })
                .sum(),
            12 => ola(201),
            13 => self.nested("TaskZZ0", &[0, 2]),
            14 => ola(172),
            15 => 0.0,
            16 => {
                let crystals = ola(202);
                if crystals > 0.0 { 1.0 / crystals } else { 0.0 }
            }
            17 => {
                let value = ola(71);
                let mut rank = 1;
                for index in 0..60 {
                    if value
                        < self
                            .data
                            .dungeon_rank_thresholds
                            .get(index)
                            .copied()
                            .unwrap_or(f64::INFINITY)
                    {
                        rank = index;
                        break;
                    }
                }
                rank as f64
            }
            18 => ola(200),
            19 => fields::field(self.save, "SSprog")
                .and_then(|value| value.as_array().cloned())
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| {
                            item.as_array()
                                .and_then(|row| row.get(1))
                                .and_then(idleon_save::json::number)
                                == Some(1.0)
                        })
                        .count() as f64
                })
                .unwrap_or(0.0),
            20 => ola(203),
            21 => self.cards1_prefix("Obol"),
            22 => (0..4)
                .map(|cauldron| {
                    fields::object_row(
                        &fields::field(self.save, "CauldronInfo").unwrap_or(Value::Null),
                        cauldron,
                    )
                    .map(sum)
                    .unwrap_or(0.0)
                })
                .sum(),
            23 => fields::object_row(
                &fields::field(self.save, "CauldronInfo").unwrap_or(Value::Null),
                4,
            )
            .map(sum)
            .unwrap_or(0.0),
            24 => fields::object_row(
                &fields::field(self.save, "CauldronP2W").unwrap_or(Value::Null),
                4,
            )
            .map(|sigils| {
                (0..sigils.len().div_ceil(2))
                    .map(|index| fields::number_at(&sigils, 1 + 2 * index) + 1.0)
                    .sum()
            })
            .unwrap_or(0.0),
            25 => ola(199),
            26 => (self.scalar("CYDeliveryBoxComplete")
                + self.scalar("CYDeliveryBoxStreak")
                + self.scalar("CYDeliveryBoxMisc"))
            .round(),
            27 => ola(204),
            28 => ola(205),
            29 => ola(206),
            30 => 1000.0 - ola(207),
            31 => ola(211),
            32 => ola(212),
            33 => ola(213),
            34 => ola(214),
            35 => ola(215),
            36 => ola(209),
            37 => sum(fields::row(self.save, "TotemInfo", &[0]).unwrap_or_default()),
            38 => self.death_note_total(),
            39 => fields::object(self.save, "WeeklyBoss")
                .map(|boss| {
                    boss.iter()
                        .filter(|(key, value)| {
                            key.starts_with("d_") && idleon_save::json::number(value) == Some(-1.0)
                        })
                        .count() as f64
                })
                .unwrap_or(0.0),
            40 => (3..=8).map(|row| self.nested("Refinery", &[row, 1])).sum(),
            41 => sum(self.row_of("Atoms")),
            42 => (0..27).map(|row| self.row_at("Tower", row)).sum(),
            43 => self.chest_quantity("Critter11A"),
            44 => ola(224),
            45 => self.row_at("Rift", 0),
            46 => self.max_pet_power(),
            47 => 1000.0 - ola(220),
            48 => (0..10)
                .map(|row| {
                    self.nested("Cooking", &[row, 6])
                        + self.nested("Cooking", &[row, 7])
                        + self.nested("Cooking", &[row, 8])
                })
                .sum(),
            49 => self.breeding_totals().0,
            50 => fields::row(self.save, "Meals", &[0])
                .map(sum)
                .unwrap_or(0.0),
            51 => self.breeding_totals().1,
            52 => fields::row(self.save, "Lab", &[15])
                .map(|row| {
                    row.iter()
                        .filter_map(|value| *value)
                        .map(|value| value.max(0.0))
                        .sum()
                })
                .unwrap_or(0.0),
            53 => sum(self.row_of("FamValColosseumHighscores")),
            54 => ola(217),
            55 => fields::field(self.save, "StuG")
                .and_then(|value| value.as_array().cloned())
                .map(|items| {
                    items
                        .iter()
                        .filter(|item| idleon_save::json::number(item).unwrap_or(0.0) >= 2.0)
                        .count() as f64
                })
                .unwrap_or(0.0),
            56 => 1000.0 - ola(218),
            57 => fields::field(self.save, "Boats")
                .and_then(|value| value.as_array().cloned())
                .map(|rows| {
                    rows.iter()
                        .map(|row| {
                            idleon_save::json::index(row, &[3])
                                .and_then(idleon_save::json::number)
                                .unwrap_or(0.0)
                                + idleon_save::json::index(row, &[5])
                                    .and_then(idleon_save::json::number)
                                    .unwrap_or(0.0)
                        })
                        .sum()
                })
                .unwrap_or(0.0),
            58 => (self.row_at("Divinity", 25) - 10.0).max(0.0),
            59 => self.nested("GamingSprout", &[28, 1]),
            60 => sum(fields::row(self.save, "Sailing", &[3]).unwrap_or_default()),
            61 => self.nested("Sailing", &[1, 0]),
            62 => fields::field(self.save, "Captains")
                .and_then(|value| value.as_array().cloned())
                .map(|rows| {
                    rows.iter()
                        .take(20)
                        .map(|row| {
                            idleon_save::json::index(row, &[3])
                                .and_then(idleon_save::json::number)
                                .unwrap_or(0.0)
                        })
                        .fold(0.0, f64::max)
                })
                .unwrap_or(0.0),
            63 => self.nested("GamingSprout", &[32, 1]).max(ola(210)),
            _ => 0.0,
        }
    }

    /// `StatueLevels_<n>` rows hold `[level, exp]` pairs; the tome sums levels.
    fn statue_total(&self) -> f64 {
        let Some(rows) =
            fields::field(self.save, "StatueLevels_0").and_then(|v| v.as_array().cloned())
        else {
            return 0.0;
        };
        rows.iter()
            .map(|row| {
                idleon_save::json::number(row).unwrap_or_else(|| {
                    row.as_array()
                        .and_then(|cells| cells.first())
                        .and_then(idleon_save::json::number)
                        .unwrap_or(0.0)
                })
            })
            .sum()
    }

    pub(crate) fn row_of(&self, key: &str) -> Vec<Option<f64>> {
        fields::numbers(self.save, key).unwrap_or_default()
    }

    pub(crate) fn row_of_nested(&self, key: &str, path: &[usize]) -> Vec<Option<f64>> {
        fields::row(self.save, key, path).unwrap_or_default()
    }

    /// `tasksGlobalData[n]` rows, columns 0..8: the task board completion count.
    fn task_total(&self, key: &str) -> f64 {
        let Some(rows) = fields::field(self.save, key).and_then(|value| value.as_array().cloned())
        else {
            return 0.0;
        };
        rows.iter()
            .filter_map(|row| row.as_array())
            .map(|row| {
                row.iter()
                    .take(8)
                    .map(|cell| idleon_save::json::number(cell).unwrap_or(0.0))
                    .sum::<f64>()
            })
            .sum()
    }

    /// A scalar save field, as the loader copies `CYDeliveryBox*` across.
    fn scalar(&self, key: &str) -> f64 {
        self.save
            .get(key)
            .and_then(idleon_save::json::number)
            .unwrap_or(0.0)
    }

    fn stamp_total(&self) -> f64 {
        let Some(value) = fields::field(self.save, "StampLv") else {
            return 0.0;
        };
        fn sum_object(value: &Value) -> f64 {
            value
                .as_object()
                .map(|fields| {
                    fields
                        .iter()
                        .filter(|(key, _)| key.as_str() != "length")
                        .map(|(_, value)| idleon_save::json::number(value).unwrap_or(0.0))
                        .sum()
                })
                .unwrap_or(0.0)
        }
        match &value {
            Value::Array(rows) => rows.iter().map(sum_object).sum(),
            other => sum_object(other),
        }
    }

    fn card_total(&self) -> f64 {
        let Some(cards) = fields::object(self.save, "Cards0") else {
            return 0.0;
        };
        cards.keys().map(|key| self.card_level(key)).sum::<f64>()
    }

    fn talent_max_total(&self) -> f64 {
        let mut max_per_skill: std::collections::BTreeMap<String, f64> = Default::default();
        for character in 0..self.characters.len() {
            let Some(row) = fields::char_field(self.save, "SM", character) else {
                continue;
            };
            let Some(fields) = row.as_object() else {
                continue;
            };
            for (skill, level) in fields {
                let level = idleon_save::json::number(level).unwrap_or(0.0);
                let entry = max_per_skill.entry(skill.clone()).or_insert(0.0);
                if level > *entry {
                    *entry = level;
                }
            }
        }
        max_per_skill.values().sum()
    }

    fn quest_total(&self) -> f64 {
        let mut unique: std::collections::BTreeSet<String> = Default::default();
        for character in 0..self.characters.len() {
            let Some(row) = fields::char_field(self.save, "QuestComplete", character) else {
                continue;
            };
            let Some(fields) = row.as_object() else {
                continue;
            };
            for (quest, status) in fields {
                if idleon_save::json::number(status) == Some(1.0) {
                    unique.insert(quest.clone());
                }
            }
        }
        unique.len() as f64
    }

    pub(crate) fn cards1_prefix(&self, prefix: &str) -> f64 {
        fields::field(self.save, "Cards1")
            .and_then(|value| value.as_array().cloned())
            .map(|items| {
                items
                    .iter()
                    .filter(|item| item.as_str().is_some_and(|name| name.starts_with(prefix)))
                    .count() as f64
            })
            .unwrap_or(0.0)
    }

    fn chest_quantity(&self, item: &str) -> f64 {
        let (Some(order), Some(quantity)) = (
            fields::field(self.save, "ChestOrder"),
            fields::field(self.save, "ChestQuantity"),
        ) else {
            return 0.0;
        };
        let (Some(order), Some(quantity)) = (order.as_array(), quantity.as_array()) else {
            return 0.0;
        };
        for (index, name) in order.iter().enumerate() {
            if name.as_str() == Some(item) {
                return quantity
                    .get(index)
                    .and_then(idleon_save::json::number)
                    .unwrap_or(0.0);
            }
        }
        0.0
    }

    fn max_pet_power(&self) -> f64 {
        let mut best = 0.0;
        for key in ["Pets", "PetsStored"] {
            if let Some(rows) =
                fields::field(self.save, key).and_then(|value| value.as_array().cloned())
            {
                for row in &rows {
                    let power = idleon_save::json::index(row, &[2])
                        .and_then(idleon_save::json::number)
                        .unwrap_or(0.0);
                    if power > best {
                        best = power;
                    }
                }
            }
        }
        best
    }

    /// `breedingLevelTotals`: `(shiny, breedability)`.
    fn breeding_totals(&self) -> (f64, f64) {
        let mut shiny = 0.0;
        let mut breedability = 0.0;
        let unlocked = self.nested("Breeding", &[2, 3]) > 0.0;
        for world in 0..5 {
            let Some(pets) = self.data.pet_world_pets.get(world) else {
                continue;
            };
            for (pet, name) in pets.iter().enumerate() {
                if name.starts_with('_') {
                    continue;
                }
                let shiny_exp = self.nested("Breeding", &[22 + world, pet]);
                let breed_exp = self.nested("Breeding", &[13 + world, pet]);
                let multi = if unlocked {
                    1.0 + ((breed_exp + 1.0).powf(0.725)).max(1.0).ln()
                } else {
                    1.0
                };
                shiny += self.shiny_pet_level(shiny_exp).max(1.0);
                breedability += ((multi - 1.0).powf(0.8).floor() + 1.0).min(9.0);
            }
        }
        (shiny, breedability)
    }

    fn death_note_total(&self) -> f64 {
        let mut total = 0.0;
        let chars = self.characters.len();
        for world in &self.data.death_note_mobs {
            for (mob, requirement) in world {
                if *mob < 0 {
                    continue;
                }
                let mut kills = 0.0;
                for character in 0..chars {
                    kills +=
                        requirement - self.nested(&format!("KLA_{character}"), &[*mob as usize, 0]);
                }
                if kills > 0.0 {
                    total += idleon_common::curve::log(kills).ceil();
                }
            }
        }
        if let Some(cells) = fields::field(self.save, "Ninja").and_then(|value| {
            idleon_save::json::index(&value, &[105]).and_then(|row| row.as_array().cloned())
        }) {
            for cell in &cells {
                let value = idleon_save::json::number(cell).unwrap_or(0.0);
                if value > 0.0 {
                    total += idleon_common::curve::log(value).ceil();
                }
            }
        }
        total
    }
}

/// `tomePCT`.
fn tome_pct(qty: f64, mode: f64, half: f64) -> f64 {
    match mode as i32 {
        0 => {
            if qty < 0.0 {
                0.0
            } else {
                (1.7 * qty / (qty + half)).powf(0.7)
            }
        }
        1 => {
            let log = idleon_common::curve::log(qty);
            2.4 * log / (2.0 * log + half)
        }
        2 => (qty / half).min(1.0),
        3 => {
            if qty > 5.0 * half {
                0.0
            } else {
                (1.2 * (6.0 * half - qty) / (7.0 * half - qty)).powf(5.0)
            }
        }
        4 => {
            let value = half.min(qty);
            (2.0 * value / (value + half)).powf(0.7)
        }
        _ => 0.0,
    }
}
