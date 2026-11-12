//! The account systems the research multiplier tree reads.
//!
//! Ports of the roll-up lookups in `js/stats/systems`: royal outposts,
//! meritocracy, legend points, vault and grimoire upgrades, exotic crops,
//! mastery, achievements, emperor bonuses, arcane upgrades, the prisma and
//! bubble lookups behind the gallery, palettes, and spelunking shop upgrades.

use super::Decoder;
use super::data::{BubbleParam, account_data};
use idleon_save::fields;
use serde_json::{Map, Value};

/// `vaultUpgBonus` from `js/stats/systems/common/vault.js`.
///
/// The level is `UpgVault[index]`; the mastery levels that multiply every
/// upgrade are the ones at 32, 61 and 89, and the upgrades in
/// `VAULT_NO_MASTERY` skip them.
pub(crate) fn vault_upgrade_bonus(save: &Map<String, Value>, index: usize) -> f64 {
    let data = account_data();
    let vault = fields::numbers(save, "UpgVault").unwrap_or_default();
    let level = fields::number_at(&vault, index);
    if level <= 0.0 {
        return 0.0;
    }
    let per_level = data.vault_per_level.get(index).copied().unwrap_or(0.0);
    if per_level == 0.0 {
        return 0.0;
    }
    let mut base = level * per_level;
    if index == 0 {
        base += (level - 25.0).max(0.0) + (level - 50.0).max(0.0) + (level - 100.0).max(0.0);
    }
    if index == 60 {
        base += (level - 25.0).max(0.0)
            + (level - 50.0).max(0.0)
            + 2.0 * (level - 100.0).max(0.0)
            + 3.0 * (level - 200.0).max(0.0)
            + 5.0 * (level - 300.0).max(0.0)
            + 7.0 * (level - 400.0).max(0.0)
            + 10.0 * (level - 450.0).max(0.0);
        base *= 1.0 + (level / 25.0).floor() / 5.0;
    }
    if !data.vault_no_mastery.contains(&(index as u32)) {
        let mastery = if index < 32 {
            fields::number_at(&vault, 32)
        } else if index <= 60 {
            fields::number_at(&vault, 61)
        } else if index <= 88 {
            fields::number_at(&vault, 89)
        } else {
            0.0
        };
        base *= 1.0 + mastery / 100.0;
    }
    base
}

impl Decoder<'_> {
    /// `RoyalGuardian("outpostROGBonus", 1)`: the research focus selection.
    pub(crate) fn royal_research(&self) -> f64 {
        let selected = fields::at(self.save, "RoyalG", &[3, 2])
            .unwrap_or(0.0)
            .floor();
        if selected != 1.0 {
            return 1.0;
        }
        let progress = (self.total_statz()[4] - 6.0).max(1.0);
        1.0 + progress / (40.0 + progress) * 2.0
    }

    /// `TotalStatz` from `js/stats/systems/w7/royal-guardian.js`, reduced to
    /// the outpost count the research focus reads.
    pub(crate) fn total_statz(&self) -> [f64; 5] {
        let maps = fields::field(self.save, "RoyalMaps")
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let built = maps
            .iter()
            .filter(|row| row.as_array().is_some_and(|cells| cells.len() >= 3))
            .count() as f64;
        [0.0, 0.0, 0.0, 0.0, built]
    }

    /// `computeMeritocBonusz` from `js/stats/systems/w7/meritoc.js`.
    pub(crate) fn meritoc(&self, option: usize) -> f64 {
        let active_vote = self.ola(453);
        if option as f64 != active_vote {
            return 0.0;
        }
        let Some(kills_left) = self.kla_map(250) else {
            return 0.0;
        };
        if kills_left > 0.0 {
            return 0.0;
        }
        let base = self.data.meritoc_base.get(option).copied().unwrap_or(0.0);
        if base <= 0.0 {
            return 0.0;
        }
        let can_vote = self.ola(472) == 1.0;
        let clam_work3 = if self.ola(464) > 3.0 { 1.0 } else { 0.0 };
        let companion_39 = self.companion(39);
        let legend_24 = self.legend(24);
        let arcade_59 = self.arcade(59);
        let event_shop_23 = self.event_shop(23);
        let rog_51 = self.rog_qty(51);
        let jelly_33 = self.jelly(33);
        let companion_161 = self.companion(161);
        let add_sum = 5.0 * clam_work3
            + companion_39
            + legend_24
            + arcade_59
            + 20.0 * event_shop_23
            + rog_51
            + jelly_33;
        let multi = (if can_vote { 1.0 } else { 0.25 }) + add_sum / 100.0;
        base * (1.0 + companion_161 / 100.0) * multi
    }

    /// `KLA[c][250]`, written as a number or a one-element row.
    pub(crate) fn kla_map(&self, map: usize) -> Option<f64> {
        let value = fields::field(self.save, "KLA_0")?;
        let row = idleon_save::json::index(&value, &[map])?;
        if let Some(number) = idleon_save::json::number(row) {
            return Some(number);
        }
        row.as_array()
            .and_then(|cells| cells.first())
            .and_then(idleon_save::json::number)
    }

    /// `legendPTSbonus`.
    pub(crate) fn legend(&self, index: usize) -> f64 {
        let points = self.spelunk(18, index);
        let per_point = self
            .data
            .legend_per_point
            .get(index)
            .copied()
            .unwrap_or(0.0);
        (points * per_point).round()
    }

    /// `vaultUpgBonus` from `js/stats/systems/common/vault.js`.
    pub fn vault(&self, index: usize) -> f64 {
        vault_upgrade_bonus(self.save, index)
    }

    /// `grimoireUpgBonus22` from `js/stats/systems/mc/grimoire.js`.
    pub(crate) fn grimoire_22(&self) -> f64 {
        let level_22 = self.row_at("Grimoire", 22);
        let level_36 = self.row_at("Grimoire", 36);
        level_22 * (1.0 + level_36 / 100.0)
    }

    /// `computeExoticBonus(idx)` from `js/stats/systems/w6/farming.js`.
    pub(crate) fn exotic(&self, index: usize) -> f64 {
        let Some(param) = self.data.exotic(index) else {
            return 0.0;
        };
        let level = self.row_at("FarmUpg", index + 20);
        if level <= 0.0 {
            return 0.0;
        }
        let mut value = if param.decay {
            param.base * level / (param.denom + level)
        } else {
            param.base * level
        };
        if self.spelunk_row(50).contains(&Some(index as f64)) {
            value *= 2.0;
        }
        if index == 39 {
            value = value.min(50.0);
        }
        value
    }

    /// `researchMasteryBonuses` from `js/stats/systems/w7/research-math.js`.
    pub(crate) fn mastery(&self) -> Mastery {
        let unlocked = self.row_at("Rift", 0) >= 15.0;
        let total = if unlocked {
            let saved: f64 = self
                .characters
                .iter()
                .filter_map(|character| character.research_level())
                .map(|level| level as f64)
                .sum();
            // The forecast advances the active character only, never the rest.
            saved + (self.research_level as f64 - self.save_research_level as f64)
        } else {
            0.0
        };
        let reached = |threshold: f64| if total >= threshold { 1.0 } else { 0.0 };
        Mastery {
            additive: 10.0 * reached(150.0) + 15.0 * reached(400.0),
            exp_multi: 1.0 + 10.0 * reached(750.0) / 100.0,
            grid_points: (reached(300.0) + reached(500.0)) as u32,
        }
    }

    /// `achieveStatus` from `js/stats/systems/common/achievement.js`.
    pub(crate) fn achievement(&self, index: usize) -> f64 {
        let completed = fields::field(self.save, "AchieveReg").and_then(|value| {
            idleon_save::json::index(&value, &[index]).and_then(idleon_save::json::number)
        }) == Some(-1.0);
        if !completed {
            return 0.0;
        }
        match index {
            4 | 27 | 37 | 44 | 107 | 109 | 117 => 5.0,
            108 => 10.0,
            99 | 104 | 112 => 20.0,
            _ => 1.0,
        }
    }

    /// `computeEmperorBon(8)` from `js/stats/systems/w6/emperor.js`.
    pub(crate) fn emperor_8(&self) -> f64 {
        let count = self.ola(369) as usize;
        let value = self.data.emperor_bonus_val.get(8).copied().unwrap_or(0.0);
        let mut sum = 0.0;
        for round in 0..count {
            let slot = round % 48;
            if self.data.emperor_bonus_type.get(slot).copied() == Some(8.0) {
                sum += value;
            }
        }
        let multi = 1.0 + (self.arcane(48) + self.arcade(51) + self.jelly(28)) / 100.0;
        (sum * multi).floor()
    }

    /// `arcaneUpgBonus` from `js/stats/systems/mc/tesseract.js`.
    pub(crate) fn arcane(&self, index: usize) -> f64 {
        let level = self.row_at("Arcane", index);
        if level <= 0.0 {
            return 0.0;
        }
        let per_level = self
            .data
            .arcane_per_level
            .get(index)
            .copied()
            .unwrap_or(1.0);
        if self.data.arcane_no_multi.contains(&(index as u32)) {
            return level * per_level;
        }
        level * per_level * (1.0 + self.arcane(39) / 100.0)
    }

    /// Whether the save's Prisma list marks the bubble at `cauldron[index]`.
    pub(crate) fn bubble_prismad_at(&self, cauldron: usize, index: usize) -> bool {
        let Some(letter) = self.data.encoding.chars().nth(cauldron) else {
            return false;
        };
        let needle = format!("{letter}{index},");
        self.ola_text(384).contains(&needle)
    }

    /// One bubble's value: the curve at its saved level, times its Prisma
    /// multiplier when the save marks it.
    pub(crate) fn bubble_value(&self, bubble: &BubbleParam) -> f64 {
        let level = fields::field(self.save, "CauldronInfo")
            .and_then(|value| fields::object_row(&value, bubble.cauldron))
            .map(|row| fields::number_at(&row, bubble.index))
            .unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let base = idleon_common::curve::eval(&bubble.formula, bubble.x1, bubble.x2, level);
        let prisma = if self.bubble_prismad_at(bubble.cauldron, bubble.index) {
            self.prisma_bonus_multi().max(1.0)
        } else {
            1.0
        };
        base * prisma
    }

    /// `getPrismaBonusMult` from `js/stats/systems/w2/alchemy.js`.
    pub(crate) fn prisma_bonus_multi(&self) -> f64 {
        let purple_sigils = fields::row(self.save, "CauldronP2W", &[4])
            .map(|row| {
                (0..24)
                    .filter(|index| fields::number_at(&row, 1 + 2 * index) >= 3.0)
                    .count() as f64
            })
            .unwrap_or(0.0);
        let trophy = fields::field(self.save, "Cards1")
            .and_then(|value| value.as_array().cloned())
            .is_some_and(|items| items.iter().any(|item| item.as_str() == Some("Trophy23")));
        let sum = self.arcane(45)
            + self.arcade(54)
            + self.rog_qty(23)
            + self.jelly(36)
            + if trophy { 10.0 } else { 0.0 }
            + self.palette(28)
            + 0.2 * purple_sigils
            + self.exotic(48)
            + self.legend(36)
            + 50.0 * self.companion(88);
        (2.0 + sum / 100.0).min(5.0)
    }

    /// `bubbleBonusY13` from `js/stats/systems/w2/alchemy.js`.
    pub(crate) fn bubble_y13(&self) -> f64 {
        self.bubble_value(&self.data.bubbles.y13)
    }

    /// `computePaletteBonus` from `js/stats/systems/w7/spelunking.js`.
    pub(crate) fn palette(&self, index: usize) -> f64 {
        let level = self.spelunk(9, index);
        if level <= 0.0 {
            return 0.0;
        }
        let Some(param) = self.data.palette(index) else {
            return 0.0;
        };
        let mut raw = if param.decay {
            level / (level + param.denom) * param.coeff
        } else {
            level * param.coeff
        };
        let superbit_59 = self.superbit(59);
        if let Some((bit, _)) = self
            .data
            .palette_superbit_pairs
            .iter()
            .find(|(_, palette)| {
                *palette == index as u32 && self.superbit(*palette as usize) == 1.0
            })
        {
            raw *= 2.0 + 0.5 * superbit_59;
            let _ = bit;
        }
        let legend_multi = 1.0 + self.legend(10) / 100.0;
        let stars = self.spelunk(49, index);
        let black_star_multi = (1.0 + (stars + (stars / 2.0).floor())).max(1.0);
        let lore = if self.spelunk(0, 8) >= 1.0 { 1.0 } else { 0.0 };
        raw * legend_multi * black_star_multi * (1.0 + 0.5 * lore)
    }

    /// `cachedSailingArt37`: five points for the fixture, capped at ten.
    pub(crate) fn sailing_art37(&self) -> f64 {
        self.nested("Sailing", &[3, 37]).round().clamp(0.0, 10.0)
    }

    /// `Sailing[3][36]`, the Ender Pearl research AFK term, capped at six.
    pub(crate) fn sailing_art36(&self) -> f64 {
        self.nested("Sailing", &[3, 36]).round().clamp(0.0, 6.0)
    }

    /// `researchGridPointsTaskLevel`: `TaskZZ2[6][3]`.
    pub(crate) fn task_grid_points(&self) -> f64 {
        self.nested("TaskZZ2", &[6, 3]).max(0.0)
    }

    /// `shopUpgBonus` from `js/stats/systems/w7/spelunking.js`.
    pub(crate) fn spelunk_upgrade(&self, index: usize) -> f64 {
        let shop_level = self.spelunk(5, index).max(0.0);
        if shop_level <= 0.0 {
            return 0.0;
        }
        let per_level = self
            .data
            .spelunk_upg_per_level
            .get(index)
            .copied()
            .unwrap_or(0.0);
        let mut value = per_level * shop_level;
        let total_spelunk_level: f64 = self
            .characters
            .iter()
            .map(|character| character.skill(19).max(0.0))
            .sum();
        let discoveries = self
            .save
            .get("Spelunk")
            .and_then(|value| idleon_save::json::index(value, &[6]))
            .and_then(|value| value.as_array())
            .map(Vec::len)
            .unwrap_or(0);
        let depths = self.spelunk_row(1);
        let total_depth: f64 = depths.iter().filter_map(|depth| *depth).sum();
        let hauls = self.spelunk_row(2);
        let haul_digits: f64 = hauls
            .iter()
            .filter_map(|haul| *haul)
            .fold(0.0, |sum, haul| {
                (sum + idleon_common::curve::log(haul).ceil()).round()
            });
        let grand: f64 = self.spelunk_row(44).iter().filter_map(|value| *value).sum();
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
            44..=46 => value *= grand,
            54..=56 => {
                let row = self.spelunk_row(47);
                let sum: f64 = row
                    .iter()
                    .take(depths.len())
                    .filter_map(|value| *value)
                    .sum();
                value *= 1.0 + 5.0 * sum / 100.0;
            }
            59..=61 => value *= self.spelunk(4, 8),
            _ => {}
        }
        value
    }

    /// `petArenaCrucBonus` from `js/stats/systems/w7/menagerie.js`.
    pub(crate) fn crucible(&self, index: usize) -> f64 {
        if self.menagerie_unlock(8) != 1.0 {
            return 0.0;
        }
        let best = self.ola(628);
        let mut count = 0.0;
        for (threshold_index, threshold) in self.data.arena_thresholds.iter().take(16).enumerate() {
            if best >= *threshold {
                count = threshold_index as f64 + 1.0;
            } else {
                break;
            }
        }
        if count > index as f64 { 1.0 } else { 0.0 }
    }

    /// `arcadeBonus` from `js/stats/systems/w2/arcade.js`.
    pub(crate) fn arcade(&self, index: usize) -> f64 {
        let Some((formula, base, denom)) = self.data.arcade(index) else {
            return 0.0;
        };
        let level = self.row_at("ArcadeUpg", index);
        if level <= 0.0 {
            return 0.0;
        }
        let raw = if formula == "add" {
            if denom != 0.0 {
                ((base + denom) / denom + 0.5 * (level - 1.0)) / (base / denom) * level * base
            } else {
                base * level
            }
        } else {
            base * level / (level + denom)
        };
        let maxed = match level as i64 {
            101 => 2.0,
            102 => 6.0,
            _ => 1.0,
        };
        let companion_27 = if self.companion(27) == 1.0 { 2.0 } else { 1.0 };
        maxed * companion_27 * raw
    }

    /// `computeCardLv` from `js/stats/systems/common/cards.js`.
    pub(crate) fn card_level(&self, key: &str) -> f64 {
        let Some(cards) = fields::object(self.save, "Cards0") else {
            return 0.0;
        };
        let qty = cards
            .get(key)
            .and_then(idleon_save::json::number)
            .unwrap_or(0.0);
        if qty <= 0.0 {
            return 0.0;
        }
        let rift_5star = if self.row_at("Rift", 0) >= 45.0 {
            1.0
        } else {
            0.0
        };
        let spelunk_6star = if self.spelunk(0, 2) >= 1.0 { 1.0 } else { 0.0 };
        let max_stars = (4.0f64 + rift_5star + spelunk_6star).round();
        let mut level = 1.0;
        if key == "Boss3B" {
            for star in 0..max_stars as u32 {
                if qty > 1.5 * (star as f64 + 1.0 + (star / 3) as f64).powi(2) {
                    level = star as f64 + 2.0;
                }
            }
        } else {
            let base_req = self.data.card_base_req.get(key).copied().unwrap_or(10.0);
            for star in 0..max_stars as u32 {
                let threshold = base_req
                    * (star as f64
                        + 1.0
                        + (star / 3) as f64
                        + 16.0 * (star / 4) as f64
                        + 100.0 * (star / 5) as f64)
                        .powi(2);
                if qty > threshold {
                    level = star as f64 + 2.0;
                }
            }
        }
        let ola_155 = self.ola_text(155);
        if ola_155.split(',').any(|entry| entry == key) && level < 6.0 {
            level = 6.0;
        }
        let ola_603 = self.ola_text(603);
        if ola_603.split(',').any(|entry| entry == key) && level < 7.0 {
            level = 7.0;
        }
        level
    }

    /// `cloudBonus` from `js/game-helpers.js`.
    pub(crate) fn cloud_bonus(&self, index: usize) -> f64 {
        let Some(weekly_boss) = fields::object(self.save, "WeeklyBoss") else {
            return 0.0;
        };
        if weekly_boss
            .get(&format!("d_{index}"))
            .and_then(idleon_save::json::number)
            == Some(-1.0)
        {
            1.0
        } else {
            0.0
        }
    }

    /// `ribbonBonusAt` from `js/game-helpers.js`.
    pub(crate) fn ribbon(&self, index: usize) -> f64 {
        let level = self.row_at("Ribbon", index);
        if level <= 0.0 {
            return 1.0;
        }
        let emperor = if self.ola_text(379).contains("EMPEROR_SET") {
            self.data.set_bonus("EMPEROR_SET")
        } else {
            0.0
        };
        let emp_term = (level / 4.0).floor() * (emperor / 4.0);
        let cloud_73 = (level / 10.0).floor() * self.cloud_bonus(73);
        let jelly_term = (level / 20.0).floor() * self.jelly(60);
        let base = 5.0 * level + (level / 2.0).floor() * (4.0 + 6.5 * (level / 5.0).floor());
        1.0 + (base + emp_term + cloud_73 + jelly_term) / 100.0
    }
}

/// The research mastery terms.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Mastery {
    pub additive: f64,
    pub exp_multi: f64,
    pub grid_points: u32,
}
