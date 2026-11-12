//! Lab connectivity and mainframe bonuses.
//!
//! Ports `computeLabConnectivity` and `mainframeBonus` from
//! `js/stats/systems/w4/lab.js`. The lab is a graph of players, mainframe
//! bonuses and jewels; a bonus is active when a connected player reaches it.

use super::Decoder;
use idleon_save::fields;
use std::collections::BTreeSet;

/// The lab state one run derived.
pub(crate) struct Lab {
    bonus: Vec<[f64; 5]>,
    connected: Vec<bool>,
    jewels: Vec<bool>,
    jewel_rows: &'static [(f64, f64, f64, String)],
}

impl Decoder<'_> {
    /// The mainframe bonus `e`, `None` when the save carries no lab layout.
    pub fn mainframe(&mut self, index: usize) -> Option<f64> {
        if self.deriving_lab {
            return Some(0.0);
        }
        if fields::field(self.save, "Lab").is_none() {
            self.warn(
                "Lab",
                "not in the save; every mainframe bonus stays at its inactive value",
            );
            return None;
        }
        self.deriving_lab = true;
        let lab = self.lab();
        self.deriving_lab = false;
        let value = lab.mainframe(index)?;
        if index == 9 {
            // `mainframeBonus(9)` scales its row by the account's millions of
            // Green Mushroom kills, on top of jewel 113.
            let base = value + lab.mainframe(113).unwrap_or(0.0);
            let millions = self.mush_kills() / 1e6;
            return Some(
                base * if millions < 1e8 {
                    millions.floor()
                } else {
                    millions
                },
            );
        }
        Some(value)
    }

    /// `_totalMushGKills`: every character's kills on the map that fights `mushG`.
    fn mush_kills(&self) -> f64 {
        let Some(map) = (0..idleon_stats_data::map_count())
            .find(|map| idleon_stats_data::afk_target(*map) == Some("mushG"))
        else {
            return 0.0;
        };
        let required = idleon_stats_data::kill_requirement(map);
        let mut total = 0.0;
        for character in 0..self.characters.len() {
            let Some(row) = fields::char_field(self.save, "KLA", character) else {
                continue;
            };
            let Some(cells) = fields::object_row(&row, map) else {
                continue;
            };
            total += required - cells.first().copied().flatten().unwrap_or(0.0);
        }
        total.max(0.0)
    }

    /// Derive the lab graph once per decode.
    fn lab(&mut self) -> Lab {
        let bonuses = self.lab_bonuses();
        let jewels = self.data.jewels.len();
        let mut connected = vec![false; bonuses.len()];
        let mut jewel_connected = vec![false; jewels];
        let lab_row = fields::row(self.save, "Lab", &[0]).unwrap_or_default();
        let mut positions = [(0.0, 0.0); 12];
        let mut in_lab = [false; 12];
        for (index, position) in positions.iter_mut().enumerate() {
            *position = (
                fields::number_at(&lab_row, 2 * index),
                fields::number_at(&lab_row, 2 * index + 1),
            );
            in_lab[index] = position.0 > 0.0 || position.1 > 0.0;
        }
        let task_range = self.nested("TaskZZ2", &[3, 4]);
        let dream_range = self.row_at("Dream", 8);
        let win_bonus = self.win_bonus(4);
        let flat = task_range + dream_range + win_bonus;
        let mut gem_distance = 80.0 + flat;

        for pass in 0..10 {
            let mut player_distance = [0.0; 12];
            for (index, distance) in player_distance.iter_mut().enumerate() {
                if in_lab[index] {
                    *distance = self.player_distance(index);
                }
            }
            connected.fill(false);
            jewel_connected.fill(false);
            let mut visited = BTreeSet::new();
            let mut queue: Vec<usize> = Vec::new();
            for node in 0..12 {
                if !in_lab[node] {
                    continue;
                }
                let distance = euclid(43.0, 229.0, positions[node].0, positions[node].1);
                if distance < player_distance[node] {
                    queue.push(node);
                    visited.insert(node);
                    break;
                }
            }
            let mut cursor = 0;
            while cursor < queue.len() {
                let source = queue[cursor];
                cursor += 1;
                let (sx, sy) = positions[source];
                for node in 0..12 + bonuses.len() + jewels {
                    if visited.contains(&node) {
                        continue;
                    }
                    if node < 12 {
                        if !in_lab[node] {
                            continue;
                        }
                        let distance = euclid(sx, sy, positions[node].0, positions[node].1);
                        if distance < player_distance[node] {
                            queue.push(node);
                            visited.insert(node);
                        }
                    } else if node < 12 + bonuses.len() {
                        let bonus = node - 12;
                        if connected[bonus] {
                            continue;
                        }
                        let threshold = if bonus == 13 || bonus == 8 {
                            80.0
                        } else {
                            gem_distance
                        };
                        let row = bonuses[bonus];
                        let distance = euclid(sx, sy, row[0], row[1]);
                        if distance < threshold {
                            connected[bonus] = true;
                            visited.insert(node);
                        }
                    } else {
                        let jewel = node - 12 - bonuses.len();
                        if jewel_connected[jewel] || !self.jewel_unlocked(jewel) {
                            continue;
                        }
                        let threshold = match jewel {
                            9 | 19 => 80.0,
                            21..=23 => 100.0,
                            _ => gem_distance,
                        };
                        let row = &self.data.jewels[jewel];
                        let distance = euclid(sx, sy, row.0, row.1);
                        if distance < threshold {
                            jewel_connected[jewel] = true;
                            visited.insert(node);
                        }
                    }
                }
            }
            let next = (80.0
                * (1.0
                    + (lab_mainframe(
                        &bonuses,
                        &connected,
                        &jewel_connected,
                        &self.data.jewels,
                        109,
                    ) + lab_mainframe(
                        &bonuses,
                        &connected,
                        &jewel_connected,
                        &self.data.jewels,
                        13,
                    )) / 100.0))
                .floor()
                + flat;
            if pass > 0 && next == gem_distance {
                break;
            }
            gem_distance = next;
        }
        Lab {
            bonus: bonuses,
            connected,
            jewels: jewel_connected,
            jewel_rows: &self.data.jewels,
        }
    }

    /// `buildLabMainBonus`: the fixed rows plus the dynamic emporium rows.
    fn lab_bonuses(&self) -> Vec<[f64; 5]> {
        let mut rows: Vec<[f64; 5]> = self
            .data
            .lab_bonus
            .iter()
            .map(|row| [row.0, row.1, row.2, row.3, row.4])
            .collect();
        for row in &self.data.lab_dynamic {
            if self.emporium(row.6 as usize) > 0.0 {
                rows.push([row.0, row.1, row.2, row.3, row.4]);
            }
        }
        rows
    }

    /// `labJewelUnlocked`: `Lab[14][idx] == 1`.
    fn jewel_unlocked(&self, index: usize) -> bool {
        fields::row(self.save, "Lab", &[14])
            .map(|row| fields::number_at(&row, index) == 1.0)
            .unwrap_or(false)
    }

    /// `computePlayerDist`, the range mainframe links reach from a player.
    fn player_distance(&mut self, player: usize) -> f64 {
        let lab_level = fields::char_field(self.save, "Lv0", player)
            .and_then(|row| {
                idleon_save::json::index(&row, &[12]).and_then(idleon_save::json::number)
            })
            .unwrap_or(0.0);
        let mut base = 50.0 + 2.0 * lab_level;
        let lab_row = fields::row(self.save, "Lab", &[0]).unwrap_or_default();
        let x = fields::number_at(&lab_row, 2 * player);
        let y = fields::number_at(&lab_row, 2 * player + 1);
        if self.jewel_unlocked(5) {
            let jewel = &self.data.jewels[5];
            if euclid(x, y, jewel.0, jewel.1) < 150.0 {
                base *= 1.25;
            }
        }
        let meal_px = self.nested("Meals", &[0, 11]) * 2.0 + self.nested("Meals", &[0, 25]) * 2.0;
        let crystal_3 = self.card_level("Crystal3");
        let flat = base + meal_px + (2.0 * crystal_3).min(50.0);
        let bubonic = self.bubonic_purple(player);
        let line = self.meal_line_pct();
        let chip_6 = 12.0 * self.chip_count(player, 6);
        let arena = 20.0 * self.pet_arena(13);
        let line_width = self.bonus_line_width(player);
        let shiny = self.shiny_bonus(19);
        flat * (1.0 + (bubonic + line + chip_6 + arena + line_width + shiny) / 100.0)
    }

    /// `computeBubonicPurple`. Its talent system is not ported; a save with
    /// any Bubonic Conjuror levels is warned about instead of guessed.
    fn bubonic_purple(&mut self, _player: usize) -> f64 {
        let has_bubonic = (0..self.characters.len()).any(|index| {
            self.save
                .get(&format!("SL_{index}"))
                .and_then(|value| idleon_save::json::index(value, &[535]))
                .and_then(idleon_save::json::number)
                .is_some_and(|level| level > 0.0)
        });
        if has_bubonic {
            self.warn(
                "SL_<n>[535]",
                "the Bubonic Conjuror lab range bonus needs the talent system; left at 0",
            );
        }
        0.0
    }

    /// `computeMealBonusLinePct`.
    fn meal_line_pct(&mut self) -> f64 {
        let eel = self.nested("Meals", &[0, 40]);
        if eel <= 0.0 {
            return 0.0;
        }
        self.cooking_meal_multi()
            .map(|meal| meal.value * self.ribbon(68) * eel)
            .unwrap_or(0.0)
    }

    /// `computeChip6Count`: chips of type 6 in a player's slots.
    pub(crate) fn chip_count(&self, player: usize, chip: usize) -> f64 {
        fields::row(self.save, "Lab", &[1 + player])
            .map(|row| {
                (0..7)
                    .filter(|slot| fields::number_at(&row, *slot) == chip as f64)
                    .count() as f64
            })
            .unwrap_or(0.0)
    }

    /// `computePetArenaBonus`: whether the arena's wave count clears the tier.
    pub fn pet_arena(&self, index: usize) -> f64 {
        let waves = self.ola(89);
        let mut tier = 0;
        for (threshold_index, threshold) in self.data.pet_arena_thresholds.iter().enumerate() {
            if waves >= *threshold {
                tier = threshold_index + 1;
            } else {
                break;
            }
        }
        if tier > index { 1.0 } else { 0.0 }
    }

    /// `computeBonusLineWidth`.
    fn bonus_line_width(&self, player: usize) -> f64 {
        let gem_slots = 2.0 * self.row_at("GemItemsPurchased", 123);
        let mut lab_characters = Vec::new();
        for index in 0..self.characters.len() {
            if self.text_field(&format!("AFKtarget_{index}")).as_deref() == Some("Laboratory") {
                lab_characters.push(index);
            }
        }
        if let Some(order) = lab_characters.iter().position(|index| *index == player) {
            return if (order as f64) < gem_slots {
                30.0
            } else {
                0.0
            };
        }
        if (player as f64) < gem_slots && self.has_bonus_major(player, 2.0) {
            30.0
        } else {
            0.0
        }
    }

    fn text_field(&self, key: &str) -> Option<String> {
        self.save.get(key).map(|value| match value {
            serde_json::Value::String(text) => text.clone(),
            other => other.to_string(),
        })
    }

    /// `hasBonusMajor` from `js/stats/systems/w5/divinity.js`.
    fn has_bonus_major(&self, player: usize, god_type: f64) -> bool {
        if self.companion(0) > 0.0
            && self
                .characters
                .first()
                .and_then(|character| character.divinity())
                .is_some_and(|divinity| divinity >= 2)
        {
            return true;
        }
        for slot in [29, 30] {
            let god = self.nested("Holes", &[11, slot]);
            if god >= 0.0 && self.god_type(god) == god_type {
                return true;
            }
        }
        let chosen = self.ola(425);
        if chosen > 0.0 && self.god_type((chosen - 1.0).max(0.0)) == god_type {
            return true;
        }
        if god_type == 2.0 && self.grid.level(173) >= 1 {
            return true;
        }
        let god = fields::at(self.save, "Divinity", &[player + 12]).unwrap_or(-1.0);
        god >= 0.0 && self.god_type(god) == god_type
    }

    fn god_type(&self, god: f64) -> f64 {
        self.data
            .god_types
            .get(god as usize)
            .copied()
            .unwrap_or(-1.0)
    }
}

impl Lab {
    /// `mainframeBonus` from `js/stats/systems/w4/lab.js`.
    fn mainframe(&self, index: usize) -> Option<f64> {
        if self.bonus.is_empty() {
            return None;
        }
        Some(lab_mainframe(
            &self.bonus,
            &self.connected,
            &self.jewels,
            self.jewel_rows,
            index,
        ))
    }
}

/// The recursive mainframe lookup, shared by the connectivity pass and the
/// finished lab state.
fn lab_mainframe(
    bonuses: &[[f64; 5]],
    connected: &[bool],
    jewels: &[bool],
    jewel_rows: &[(f64, f64, f64, String)],
    index: usize,
) -> f64 {
    if index < 100 {
        let Some(row) = bonuses.get(index) else {
            return 0.0;
        };
        if !connected.get(index).copied().unwrap_or(false) {
            return row[3];
        }
        let active = row[4];
        return match index {
            8 => active + lab_mainframe(bonuses, connected, jewels, jewel_rows, 119) / 100.0,
            13 => active,
            15 => active + lab_mainframe(bonuses, connected, jewels, jewel_rows, 118),
            17 => active + lab_mainframe(bonuses, connected, jewels, jewel_rows, 120),
            _ => active,
        };
    }
    let jewel = index - 100;
    if !jewels.get(jewel).copied().unwrap_or(false) {
        return 0.0;
    }
    let Some(row) = jewel_rows.get(jewel) else {
        return 0.0;
    };
    if index == 119 {
        return row.2;
    }
    row.2 * lab_mainframe(bonuses, connected, jewels, jewel_rows, 8)
}

/// The game's fast distance: an octagonal approximation.
fn euclid(x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let dx = (x2 - x1).abs();
    let dy = (y2 - y1).abs();
    0.9604339 * dx.max(dy) + 0.397824735 * dx.min(dy)
}
