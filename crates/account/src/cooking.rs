//! Cooking, breeding and summoning terms the research tree reads.
//!
//! Ports `cookingMealMulti` (`js/stats/systems/common/cooking.js`),
//! `computeShinyBonusS` (`js/stats/systems/w4/breeding.js`) and
//! `computeWinBonus` (`js/stats/systems/w6/summoning.js`).

use super::Decoder;
use idleon_save::fields;

/// The cooking meal multiplier and the terms it is built from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Cooking {
    pub value: f64,
}

impl Decoder<'_> {
    /// `cookingMealMulti`. `None` when the save carries no breeding data, the
    /// field the shiny and summoning terms need.
    pub(crate) fn cooking_meal_multi(&mut self) -> Option<Cooking> {
        if fields::field(self.save, "Breeding").is_none()
            && fields::field(self.save, "Summon").is_none()
        {
            self.warn(
                "Summon",
                "no summoning or breeding data; the meal multiplier stays at 1",
            );
            return None;
        }
        let mainframe = self.mainframe(116).unwrap_or(0.0);
        let shiny = self.shiny_bonus(20);
        let winner = self.win_bonus(26);
        let companion = 25.0 * self.companion(162);
        let value = (1.0 + (mainframe + shiny) / 100.0)
            * (1.0 + winner / 100.0)
            * (1.0 + companion / 100.0);
        Some(Cooking { value })
    }

    /// `computeShinyBonusS`.
    pub(crate) fn shiny_bonus(&self, category: u32) -> f64 {
        let mut total = 0.0;
        for world in 0..4 {
            let Some(types) = self.data.pet_shiny_type.get(world) else {
                continue;
            };
            let exps = self.nested_row("Breeding", &[22 + world]);
            if exps.is_empty() {
                continue;
            }
            for (pet, shiny_type) in types.iter().enumerate() {
                let Some(shiny_type) = shiny_type else {
                    continue;
                };
                let exp = fields::number_at(&exps, pet);
                if exp <= 0.0 {
                    continue;
                }
                if self
                    .data
                    .shiny_type_to_cat
                    .get(*shiny_type as usize)
                    .copied()
                    != Some(category)
                {
                    continue;
                }
                let level = self.shiny_pet_level(exp);
                let per_level = self
                    .data
                    .shiny_bonus_per_level
                    .get(*shiny_type as usize)
                    .copied()
                    .unwrap_or(0.0);
                total += (level * per_level).round();
            }
        }
        total
    }

    /// `shinyPetLevel`.
    pub(crate) fn shiny_pet_level(&self, exp: f64) -> f64 {
        if exp <= 0.0 {
            return 0.0;
        }
        let thresholds = (19.0 + 10.0 * self.menagerie_unlock(11)).round();
        let mut level = 1.0;
        for index in 0..thresholds as usize {
            let requirement =
                (1.0 + (index as f64 + 1.0).powf(1.6)).floor() * 1.7f64.powf(index as f64 + 1.0);
            if exp > requirement {
                level = index as f64 + 2.0;
            }
        }
        level
    }

    /// `computeWinBonus`.
    pub fn win_bonus(&self, index: usize) -> f64 {
        self.win_bonus_from(index, self.summon_wins(self.ola(319) as usize))
    }

    /// `computeWinBonus` when the caller passes no account options: the endless
    /// wins are left out, as `computeSummWinBonus(save, null)` reads it.
    pub(crate) fn win_bonus_without_options(&self, index: usize) -> f64 {
        self.win_bonus_from(index, self.summon_wins(0))
    }

    fn win_bonus_from(&self, index: usize, wins: Vec<f64>) -> f64 {
        let raw = wins.get(index).copied().unwrap_or(0.0);
        if raw <= 0.0 {
            return 0.0;
        }
        if matches!(index, 20 | 22 | 24 | 31) {
            return raw;
        }
        let pristine = if self.nested("Ninja", &[107, 8]) == 1.0 {
            30.0
        } else {
            0.0
        };
        let gems = self.row_at("GemItemsPurchased", 11);
        let rarity = self.nested("Sailing", &[3, 32]);
        let artifact = if rarity > 0.0 {
            self.data.artifacts.get(32).copied().unwrap_or(0.0) * rarity
        } else {
            0.0
        };
        let task = self.nested("TaskZZ2", &[5, 4]).min(10.0);
        let wb31 = wins.get(31).copied().unwrap_or(0.0);
        let emperor = self.emperor_8();
        let godshard = if self.ola_text(379).contains("GODSHARD_SET") {
            self.data.set_bonus("GODSHARD_SET")
        } else {
            0.0
        };
        let achievement_379 = self.achievement(379);
        let achievement_373 = self.achievement(373);
        let ban_i = self
            .save
            .get("BundlesReceived")
            .and_then(|value| {
                let value = match value {
                    serde_json::Value::String(_) => {
                        idleon_save::json::parsed_field(self.save, "BundlesReceived")?
                    }
                    other => other.clone(),
                };
                value.get("ban_i").and_then(idleon_save::json::number)
            })
            .unwrap_or(0.0);
        let ban_i = if ban_i > 0.0 { 50.0 } else { 0.0 };
        let mut winner_sum = artifact + task + achievement_379 + achievement_373 + godshard + ban_i;
        if index != 19 {
            winner_sum += wb31 + emperor;
        }
        let base = if (20..=33).contains(&index) { 1.0 } else { 3.5 };
        base * raw
            * (1.0 + pristine / 100.0)
            * (1.0 + 10.0 * gems / 100.0)
            * (1.0 + winner_sum / 100.0)
    }

    /// `computeSummWinBonus`: normal wins repeat their bonus index, endless
    /// wins cycle through the 40-entry table.
    fn summon_wins(&self, endless: usize) -> Vec<f64> {
        let mut bonus = vec![0.0; 32];
        if let Some(wins) = fields::field(self.save, "Summon").and_then(|rows| {
            idleon_save::json::index(&rows, &[1]).and_then(|row| row.as_array().cloned())
        }) {
            {
                for name in &wins {
                    let Some(name) = name.as_str() else {
                        continue;
                    };
                    if name.starts_with("rift") {
                        continue;
                    }
                    if let Some((index, value)) = self.data.summon_normal_bonus.get(name) {
                        let index = (index.unwrap_or(0.0) - 1.0).round() as usize;
                        if index < 32 {
                            bonus[index] += value.unwrap_or(0.0);
                        }
                    }
                }
            }
        }
        for win in 0..endless {
            let slot = win % 40;
            let kind = self
                .data
                .summon_endless_type
                .get(slot)
                .copied()
                .unwrap_or(0.0)
                - 1.0;
            let value = self
                .data
                .summon_endless_val
                .get(slot)
                .copied()
                .unwrap_or(0.0);
            if kind >= 0.0 && (kind as usize) < 32 {
                bonus[kind as usize] += value;
            }
        }
        bonus
    }
}
