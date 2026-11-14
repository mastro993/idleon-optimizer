//! The remaining save-derived providers the damage descriptor and the Max HP,
//! Max MP and Movement Speed descriptors read, ported from the same upstream
//! `js/stats/systems/` modules.
//!
//! Every method here is one upstream function. A value that needs a table this
//! build does not carry is recorded in [`Ctx::missing`] rather than guessed.

use crate::systems::Ctx;
use idleon_common::curve;
use idleon_save::fields;

impl Ctx<'_, '_> {
    /// `TalentCalc(616)`: `min(talent(616), floor(highest Beginner level / 10))`.
    pub(crate) fn beginner_weapon_cap(&mut self) -> f64 {
        let mut highest = 0.0f64;
        for character in 0..self.characters() {
            let class = fields::char_field(self.save(), "CharacterClass", character)
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            if class >= 6.0 {
                continue;
            }
            let level = fields::char_field(self.save(), "Lv0", character)
                .and_then(|value| idleon_save::json::index(&value, &[0]).cloned())
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            highest = highest.max(level);
        }
        if highest <= 0.0 {
            return 0.0;
        }
        self.talent(616).min((highest / 10.0).floor())
    }

    /// `TotalFoodBonuses("WeaponPowerBoosts")`, with the food effect multiplier.
    pub(crate) fn weapon_food(&mut self) -> f64 {
        let amount = self.food_amount("WeaponPowerBoosts");
        if amount <= 0.0 {
            return 0.0;
        }
        amount * self.food_effect_multi()
    }

    /// The equipped food items whose `Effect` matches, summed by `Amount`.
    pub(crate) fn food_amount(&mut self, effect: &str) -> f64 {
        let slots = fields::char_field(self.save(), "FoodSlO", self.character)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .round()
            .max(0.0) as usize;
        let names =
            fields::string_row(self.save(), &format!("EquipOrder_{}", self.character), &[2])
                .unwrap_or_default();
        let qtys = fields::char_field(self.save(), "EquipQTY", self.character)
            .and_then(|value| fields::object_row(&value, 2))
            .unwrap_or_default();
        let mut total = 0.0;
        for index in 0..slots {
            let Some(name) = names.get(index).map(String::as_str) else {
                continue;
            };
            if name.is_empty() || name == "Blank" {
                continue;
            }
            let Some(item) = self.data.items.get(name) else {
                continue;
            };
            if item.effect.as_deref() != Some(effect) {
                continue;
            }
            if fields::number_at(&qtys, index) <= 0.0 {
                continue;
            }
            total += item.amount;
        }
        total
    }

    /// `FoodBonuses("BoostsEffectBonus")`: every food-effect source, as a
    /// multiplier, in the descriptor's order.
    pub(crate) fn food_effect_multi(&mut self) -> f64 {
        let talent = self.talent(631);
        let stats = self.box_reward("PowerFoodEffect")
            + self.statue_bonus(3)
            + self.decoder.etc(9, self.character)
            + self.stamp("BFood")
            + self.star_sign_bonus("FoodEffect")
            + self.card_bonus(48)
            + self.card_set_bonus("1")
            + talent;
        1.0 + stats / 100.0
    }

    /// `computeCardSetBonus(charIdx, setKey)`: `CSetEq_<char>[IDforCardSETbonus]`.
    pub(crate) fn card_set_bonus(&self, set: &str) -> f64 {
        let Some(name) = self.data.cards.id_for_set.get(set) else {
            return 0.0;
        };
        fields::char_field(self.save(), "CSetEq", self.character)
            .and_then(|value| value.get(name).cloned())
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
    }

    /// `computeChipBonus(effectKey, charIdx)`: the summed value of the chips
    /// with that effect equipped in the character's lab slots.
    pub(crate) fn chip_bonus(&self, key: &str) -> f64 {
        let chips = fields::field(self.save(), "Lab")
            .and_then(|value| fields::object_row(&value, 1 + self.character))
            .unwrap_or_default();
        let mut total = 0.0;
        for chip in chips {
            let Some(kind) = chip else {
                continue;
            };
            if kind < 0.0 {
                continue;
            }
            if let Some(row) = self.data.chips.get(kind as usize)
                && row.key == key
            {
                total += row.value;
            }
        }
        total
    }

    /// `shrine(id)`: the shrine bonus at the selected map.
    pub(crate) fn shrine(&mut self, index: usize, map: usize) -> f64 {
        let base = self.data.shrine_base(index);
        let per_level = self.data.shrine_per_level(index);
        if base == 0.0 && per_level == 0.0 {
            return 0.0;
        }
        let Some(row) = fields::field(self.save(), "Shrine")
            .and_then(|value| fields::object_row(&value, index))
        else {
            return 0.0;
        };
        let level = fields::number_at(&row, 3);
        if level <= 0.0 {
            return 0.0;
        }
        let shrine_map = fields::number_at(&row, 0);
        let artifact_global = fields::at(self.save(), "Sailing", &[3, 0]).unwrap_or(0.0) > 0.0;
        let exact = map as f64 == shrine_map;
        let world_tour = self.decoder.mainframe(5) == Some(1.0)
            && (map as f64 / 50.0).floor() == (shrine_map / 50.0).floor();
        if !artifact_global && !exact && !world_tour {
            return 0.0;
        }
        let card = 1.0 + self.data.boss3b_card_pct * self.decoder.card_level("Boss3B") / 100.0;
        card * ((level - 1.0) * per_level + base)
    }

    /// `computeSaltLick(index)`.
    pub(crate) fn salt_lick(&self, index: usize) -> f64 {
        let purchased = fields::at(self.save(), "SaltLick", &[index]).unwrap_or(0.0);
        if purchased <= 0.0 {
            return 0.0;
        }
        purchased * self.data.salt_licks.get(index).copied().unwrap_or(0.0)
    }

    /// `computePetArenaBonus(index)`: one arena tier, as a flag.
    pub(crate) fn pet_arena(&self, index: usize) -> f64 {
        if self.decoder.pet_arena(index) > 0.0 {
            1.0
        } else {
            0.0
        }
    }

    /// `mainframeBonus(index)`.
    pub(crate) fn mainframe(&mut self, index: usize) -> f64 {
        self.decoder.mainframe(index).unwrap_or(0.0)
    }

    /// `computeArtifactBonus(index, character)`.
    pub(crate) fn artifact(&mut self, index: usize) -> f64 {
        let tier = fields::at(self.save(), "Sailing", &[3, index]).unwrap_or(0.0);
        if tier <= 0.0 {
            return 0.0;
        }
        let mut value = self.data.artifact(index);
        match index {
            2 | 10 | 18 | 20 => {
                let slab = (self.decoder.slab_count().max(0.0) - 500.0).max(0.0) / 10.0;
                let slab = slab.floor();
                let mer = self.decoder.meritoc(23);
                let legend = self.decoder.legend(28);
                let vault = self.decoder.vault(74);
                let slabbo = (1.0 + mer / 100.0) * (1.0 + legend / 100.0) * (1.0 + vault / 100.0);
                value *= (1.0 + self.mainframe(15) / 100.0) * slabbo * slab;
            }
            1 => value *= self.highest_class_level(),
            3 | 5 => {
                value *= fields::char_field(self.save(), "Lv0", self.character)
                    .and_then(|row| idleon_save::json::index(&row, &[13]).cloned())
                    .and_then(|cell| idleon_save::json::number(&cell))
                    .unwrap_or(0.0)
            }
            8 => value *= self.artifact_build_speed(),
            23 => {
                value *= fields::char_field(self.save(), "Lv0", self.character)
                    .and_then(|row| idleon_save::json::index(&row, &[15]).cloned())
                    .and_then(|cell| idleon_save::json::number(&cell))
                    .unwrap_or(0.0)
            }
            25 => {
                let stat = crate::primary_stat_for_class(self.class());
                let total = crate::stats::total_stat_here(self, stat);
                value *= (total / 100.0).floor();
            }
            27 => value *= curve::log(fields::at(self.save(), "Sailing", &[1, 0]).unwrap_or(1.0)),
            29 => value *= curve::log(fields::at(self.save(), "Divinity", &[39]).unwrap_or(1.0)),
            _ => {}
        }
        if self.data.artifact_tier_bonus(index, tier as usize) {
            value *= tier;
        }
        value
    }

    fn highest_class_level(&self) -> f64 {
        (0..self.characters())
            .map(|character| {
                fields::char_field(self.save(), "Lv0", character)
                    .and_then(|value| idleon_save::json::index(&value, &[0]).cloned())
                    .and_then(|value| idleon_save::json::number(&value))
                    .unwrap_or(0.0)
            })
            .fold(0.0, f64::max)
    }

    /// `computeCompassBonus(index)`.
    pub(crate) fn compass(&self, index: usize) -> f64 {
        self.decoder.compass(index)
    }

    /// `computeExoticBonus(index)`.
    pub(crate) fn exotic(&self, index: usize) -> f64 {
        self.decoder.exotic(index)
    }

    /// `computeShinyBonusS(category)`.
    pub(crate) fn shiny(&self, category: u32) -> f64 {
        self.decoder.shiny_bonus(category)
    }

    /// `tome.resolve(id)`.
    pub(crate) fn tome_bonus(&mut self, index: usize) -> f64 {
        let Some(data) = self.data.tome_bonus(index).cloned() else {
            return 0.0;
        };
        let unlocked = match data.unlock_type.as_str() {
            "always" => true,
            "eventShop" => self.decoder.event_shop(data.unlock_index) >= 1.0,
            _ => self.option(data.unlock_index) >= 1.0,
        };
        if !unlocked {
            return 0.0;
        }
        let score = self.tome_points();
        let scaled = ((score - data.threshold).max(0.0) / data.divisor).floor();
        let base = data.base * scaled.powf(data.exponent);
        if base <= 0.0 {
            return 0.0;
        }
        let troll = idleon_account::sets::set_bonus(self.decoder, "TROLL_SET", self.character);
        base * (1.0 + (self.decoder.grimoire_level(17) + troll) / 100.0)
    }

    /// `computeMonumentROGbonus(t, i)`.
    pub(crate) fn monument_rog(&mut self, tier: usize, index: usize) -> f64 {
        let slot = 10 * tier + index;
        let level = fields::at(self.save(), "Holes", &[15, slot]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let bonus = self.decoder.monument_bonus(slot);
        if bonus <= 0.0 {
            return 0.0;
        }
        let mut holeoz = 1.0;
        if index != 9 {
            holeoz =
                1.0 + self.monument_rog(tier, 9) / 100.0 + self.decoder.cosmo_bonus(0, 0) / 100.0;
        }
        holeoz *= 1.0 + self.fountain_bon(tier, 13) / 100.0;
        if bonus < 30.0 {
            level * bonus * holeoz.max(1.0)
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * bonus * holeoz.max(1.0)).ceil()
        }
    }

    /// `bonTOT`: one fountain upgrade's bonus at the save's levels.
    pub(crate) fn fountain_bon(&self, water: usize, upgrade: usize) -> f64 {
        let level = fields::at(self.save(), "Holes", &[31, water, upgrade]).unwrap_or(0.0);
        let marble = fields::at(self.save(), "Holes", &[32, water, upgrade]).unwrap_or(0.0);
        let per_level = idleon_fountain::upgrade(water, upgrade)
            .map(|row| row.per_level)
            .unwrap_or(0.0);
        let marble = if marble <= 0.0 {
            1.0
        } else {
            1.5 + 0.5 * marble
        };
        (marble * level * per_level).round()
    }

    /// `jellyRewardBonus(index)`.
    pub(crate) fn jelly_reward(&self, index: usize) -> f64 {
        self.decoder.jelly(index)
    }

    /// `mineheadBonusQTY(index)`.
    pub(crate) fn minehead_bonus(&self, index: usize) -> f64 {
        self.decoder.minehead_bonus(index)
    }

    /// `coralKidUpgradeEffect(index, level)`.
    pub(crate) fn coral_kid(&self, index: usize, level: f64) -> f64 {
        let lv = level.max(0.0);
        match index {
            0 => 10.0 * lv,
            1 => (2.0 * lv).round(),
            2 => lv / (25.0 + lv) * 20.0,
            3 => lv.round(),
            4 => (2.0 * lv).round(),
            5 => lv / (40.0 + lv) * 100.0,
            6 => (100.0 * lv).round(),
            7 => (10.0 * lv).round(),
            8 => (50.0 * lv).round(),
            9 => (125.0 * lv / (lv + 100.0)).round(),
            10 => (5.0 * lv).round(),
            11 => lv.round(),
            _ => 0.0,
        }
    }

    /// `statueBonus(S, index)`: the Royal Guardian outpost's statue value.
    pub(crate) fn royal_statue(&self, index: usize) -> f64 {
        let level = fields::at(self.save(), "RoyalG", &[0, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let armory_level = fields::at(self.save(), "RoyalG", &[2, 45]).unwrap_or(0.0);
        let armory = armory_level * self.data.armory_per_level(45);
        (1.0 + armory / 100.0)
            * (self.data.outpost_requirement(index)
                + self.data.outpost_cost(index) * (level - 1.0).max(0.0))
    }

    /// `computeDivinityBless(index)`.
    pub(crate) fn divinity_bless(&self, index: usize) -> f64 {
        let level = fields::at(self.save(), "Divinity", &[28 + index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let base = match self.data.god_bless_base(index) {
            value if value > 0.0 => value,
            _ => 1.0,
        };
        let div25 = fields::at(self.save(), "Divinity", &[25]).unwrap_or(0.0);
        let emp = if self.decoder.emporium(33) > 0.0 {
            1.0 + 0.05 * (div25 - 10.0).max(0.0)
        } else {
            1.0
        };
        level * base * emp
    }

    /// `computeDivinityMinor(character, style)`.
    pub(crate) fn divinity_minor(&self, style: i32) -> f64 {
        idleon_account::divinity::minor_for(self.decoder, self.character, style)
    }

    /// `computeEclipseSkulls()`.
    pub(crate) fn eclipse_skulls(&mut self) -> f64 {
        let rift = self.rift_level();
        let mut count = 0.0;
        let worlds = self.decoder.death_note_mob_names().to_vec();
        for world in worlds.iter().take(6) {
            for mob in world {
                let Some(map) = (0..idleon_stats_data::map_count())
                    .find(|map| idleon_stats_data::afk_target(*map) == Some(mob.as_str()))
                else {
                    continue;
                };
                let required = idleon_stats_data::kill_requirement(map);
                let mut total_kills = 0.0;
                for character in 0..self.characters() {
                    let left = fields::char_field(self.save(), "KLA", character)
                        .and_then(|value| idleon_save::json::index(&value, &[map]).cloned())
                        .and_then(|value| value.as_array().and_then(|row| row.first()).cloned())
                        .and_then(|value| idleon_save::json::number(&value))
                        .unwrap_or(0.0);
                    total_kills += required - left;
                }
                if death_note_rank(total_kills, rift) >= 15.0 {
                    count += 1.0;
                }
            }
        }
        5.0 * count
    }

    /// `computeKillroyDMG()`.
    pub(crate) fn killroy_dmg(&mut self) -> f64 {
        let Some(best) = fields::field(self.save(), "KRbest") else {
            return 0.0;
        };
        let mut total = 0.0;
        for world in self.decoder.death_note_mob_names() {
            for mob in world {
                total += best
                    .get(mob)
                    .and_then(idleon_save::json::number)
                    .unwrap_or(0.0);
            }
        }
        total.max(0.0).powf(0.4).floor()
    }

    /// `computeRiftSkillETC(idx)`.
    pub(crate) fn rift_skill_etc(&mut self, index: usize) -> f64 {
        let ranks = self.skill_ranks();
        let threshold = (index as f64 + 2.0).round();
        let increment = match index {
            1 => 10.0,
            3 => 1.0,
            4 => 25.0,
            _ => 5.0,
        };
        let excluded = [0usize, 2, 3, 5, 6, 8];
        let mut result = 7.0;
        for skill in 0..18 {
            if !matches!(index, 1 | 3 | 4) && excluded.contains(&skill) {
                continue;
            }
            if skill < ranks.len() && ranks[skill] > threshold {
                result += increment;
            }
        }
        result
    }

    fn skill_ranks(&self) -> Vec<f64> {
        let mut ranks = Vec::new();
        for skill in 0..24 {
            let mut total = 0.0;
            for character in 0..self.characters() {
                total += fields::char_field(self.save(), "Lv0", character)
                    .and_then(|row| idleon_save::json::index(&row, &[skill + 1]).cloned())
                    .and_then(|cell| idleon_save::json::number(&cell))
                    .unwrap_or(0.0);
            }
            ranks.push(if total < 150.0 {
                0.0
            } else if total < 200.0 {
                1.0
            } else if total < 300.0 {
                2.0
            } else if total < 400.0 {
                3.0
            } else if total < 500.0 {
                4.0
            } else if total < 750.0 {
                5.0
            } else if total < 1000.0 {
                6.0
            } else {
                7.0
            });
        }
        ranks
    }

    /// `computeSummUpgBonus(t)`.
    pub(crate) fn summ_upg(&mut self, index: usize) -> f64 {
        let level = fields::at(self.save(), "Summon", &[0, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let Some(row) = self.data.summon_upgrade(index).cloned() else {
            return 0.0;
        };
        if row.per_level <= 0.0 {
            return 0.0;
        }
        let mut moltoz = 1.0;
        let gilded = fields::field(self.save(), "Holes")
            .and_then(|value| idleon_save::json::index(&value, &[28]).cloned())
            .and_then(|value| value.as_array().cloned())
            .is_some_and(|row| {
                row.iter()
                    .any(|cell| idleon_save::json::number(cell) == Some(index as f64))
            });
        if gilded {
            moltoz = 2.0;
            let talent = self.talent_best(597);
            moltoz += (talent / 100.0 - 1.0).max(0.0);
            if index != 78
                && let Some(bonus) = self.data.summon_upgrade(78)
            {
                moltoz += fields::at(self.save(), "Summon", &[0, 78]).unwrap_or(0.0)
                    * bonus.per_level
                    / 100.0;
            }
        }
        if row.stone_eligible == 1.0 {
            let trial = fields::field(self.save(), "KRbest")
                .and_then(|value| value.get(format!("SummzTrz{}", row.stone_type)).cloned())
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            if trial > 0.0 {
                moltoz *= 1.0 + trial;
            }
        }
        moltoz * level * row.per_level
    }

    /// `RandoListo2[13]` for the Cove's total-damage upgrade (19).
    pub(crate) fn cglunko_dmg(&self) -> f64 {
        self.option(630 + 19) * self.data.cglunko_per_level(19)
    }

    /// `computeStickerDmgMulti()`.
    pub(crate) fn sticker_dmg(&self) -> f64 {
        let grid = self.decoder.grid_bonus(47);
        if grid < 1.0 {
            return 1.0;
        }
        let stickers: f64 = fields::field(self.save(), "Research")
            .and_then(|value| idleon_save::json::index(&value, &[9]).cloned())
            .and_then(|value| value.as_array().cloned())
            .map(|row| {
                row.iter()
                    .map(|cell| idleon_save::json::number(cell).unwrap_or(0.0))
                    .sum()
            })
            .unwrap_or(0.0);
        1.0 + grid * stickers / 100.0
    }

    /// `computeAllShimmerBonuses()`.
    pub(crate) fn shimmer(&self) -> f64 {
        let tier = fields::at(self.save(), "Sailing", &[3, 31]).unwrap_or(0.0);
        if tier > 0.0 {
            (1.0 + tier).clamp(1.0, 4.0)
        } else {
            1.0
        }
    }

    /// `totalGreenSigils()`.
    pub(crate) fn green_sigils(&self) -> f64 {
        let mut total = 0.0;
        for sigil in 0..24 {
            if fields::at(self.save(), "CauldronP2W", &[4, 1 + 2 * sigil]).unwrap_or(0.0) >= 4.0 {
                total += 1.0;
            }
        }
        total
    }

    /// `computeConstMasteryBonus(1)`: the Rift-gated tower mastery.
    pub(crate) fn const_mastery(&self) -> f64 {
        if self.rift_level() < 40.0 {
            return 0.0;
        }
        let total: f64 = (0..27)
            .map(|index| fields::at(self.save(), "Tower", &[index]).unwrap_or(0.0))
            .sum();
        (2.0 * ((total - 750.0) / 10.0).floor()).max(0.0)
    }

    /// `computeFamBonusQTYs(ci)[key]`.
    pub(crate) fn family(&mut self, key: usize) -> f64 {
        self.family_bonuses().get(&key).copied().unwrap_or(0.0)
    }

    /// `computeRooBonus(index)`.
    pub(crate) fn roo(&self, index: usize) -> f64 {
        self.decoder.roo(index)
    }

    /// `bubbaRoGBonuses(index)`.
    pub(crate) fn bubba_rog(&mut self, index: usize) -> f64 {
        let mascot = self.decoder.companion(51);
        let megaflesh = |i: f64| -> f64 {
            let value = fields::at(self.save(), "Bubba", &[1, 8]).unwrap_or(0.0);
            if value > i {
                if i == 11.0 { value - 11.0 } else { 1.0 }
            } else {
                0.0
            }
        };
        let all = 20.0
            * ([1.0, 3.0, 6.0, 9.0, 11.0]
                .iter()
                .map(|i| megaflesh(*i))
                .sum::<f64>());
        let base = self.data.bubba_base(index);
        let level = fields::at(self.save(), "Bubba", &[1, 3]).unwrap_or(0.0);
        let factor = ((level - (index as f64 - 1.0)) / 7.0).ceil();
        ((1.0 + all / 100.0) * (1.0 + mascot) * base * factor).max(0.0)
    }

    /// `farmRankUpgBonus(index)`.
    pub(crate) fn farm_rank_upgrade(&mut self, index: usize) -> f64 {
        let level = fields::at(self.save(), "FarmRank", &[2, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let talent = self.talent_best(207).max(1.0);
        let exotic = 1.0 + self.exotic(14) / 100.0;
        let base = self.data.farm_rank_per_level(index);
        if matches!(index, 4 | 9 | 14 | 19) {
            talent * exotic * base * level
        } else {
            talent * exotic * 1.7 * base * level / (level + 80.0)
        }
    }

    /// `getBuffBonus(id, tab)`: the buff rows an import does not carry are 0,
    /// exactly as `buffsActiveData` is empty for a plain save.
    pub(crate) fn buff(&self, _id: usize) -> f64 {
        0.0
    }

    /// `weeklyBossData.g`.
    pub(crate) fn weekly_boss(&self) -> f64 {
        idleon_save::json::parsed_field(self.decoder.save(), "WeeklyBoss")
            .and_then(|value| value.get("g").cloned())
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
    }

    /// Whether the Reliquarium (Quest105) is in any inventory.
    pub(crate) fn reliquarium(&self) -> bool {
        (0..self.characters()).any(|character| {
            fields::string_row(self.save(), &format!("InventoryOrder_{character}"), &[])
                .or_else(|| {
                    fields::field(self.save(), &format!("InventoryOrder_{character}")).and_then(
                        |value| {
                            value.as_array().map(|row| {
                                row.iter()
                                    .map(|cell| cell.as_str().unwrap_or_default().to_string())
                                    .collect()
                            })
                        },
                    )
                })
                .is_some_and(|row| row.iter().any(|cell| cell == "Quest105"))
        })
    }

    /// `Scurge` level at the selected map (`Menagerie[5][2]` on map 306).
    pub(crate) fn scurge_level(&self, map: usize) -> f64 {
        if map != 306 {
            return 0.0;
        }
        fields::at(self.save(), "Menagerie", &[5, 2]).unwrap_or(0.0)
    }

    /// `TotalQuestsComplete`.
    pub(crate) fn quests(&self) -> f64 {
        self.decoder.quest_total()
    }

    /// `computeMSABonus(index)`.
    pub(crate) fn msa(&mut self, index: usize) -> f64 {
        // [gate, gate index, multiplier, offset]; gate 0 = superbit, 1 = emporium.
        const CONFIG: [(u8, usize, f64, f64); 11] = [
            (0, 7, 1.0, 0.0),
            (0, 13, 13.0, 0.0),
            (0, 3, 1.0, 0.0),
            (0, 20, 50.0, 0.0),
            (0, 11, 1.12, 0.0),
            (0, 16, 1.0, 0.0),
            (1, 12, 1.75, 0.0),
            (1, 13, 2.0, 0.0),
            (1, 14, 1.5, 0.0),
            (0, 27, 2.5, 300.0),
            (0, 44, 0.3, 300.0),
        ];
        let Some((gate, gate_index, multiplier, offset)) = CONFIG.get(index).copied() else {
            return 0.0;
        };
        let unlocked = if gate == 0 {
            self.decoder.superbit(gate_index) == 1.0
        } else {
            self.decoder.emporium(gate_index) > 0.0
        };
        if !unlocked {
            return 0.0;
        }
        let stars: f64 = fields::field(self.save(), "TotemInfo")
            .and_then(|value| fields::object_row(&value, 0))
            .map(|row| row.iter().filter_map(|cell| *cell).sum())
            .unwrap_or(0.0);
        let base = ((stars - offset) / 10.0).floor().max(0.0);
        multiplier * base
    }
}

/// `deathNoteRank`.
fn death_note_rank(kills: f64, rift: f64) -> f64 {
    if kills < 25_000.0 {
        0.0
    } else if kills < 100_000.0 {
        1.0
    } else if kills < 250_000.0 {
        2.0
    } else if kills < 500_000.0 {
        3.0
    } else if kills < 1e6 {
        4.0
    } else if kills < 5e6 {
        5.0
    } else if kills < 1e8 {
        7.0
    } else if kills > 1e9 && rift >= 20.0 {
        20.0
    } else {
        10.0
    }
}

/// `primaryStatForClass`'s index in [`crate::Stat`].
pub(crate) fn primary_stat_index(class: usize) -> crate::Stat {
    if class < 1 {
        return crate::Stat::Str;
    }
    if class < 6 {
        return crate::Stat::Luk;
    }
    match 6 + 12 * ((class - 6) / 12) {
        18 => crate::Stat::Agi,
        30 => crate::Stat::Wis,
        _ => crate::Stat::Str,
    }
}

impl Ctx<'_, '_> {
    /// `max-hp`: `_customBlock_PlayerHPmax` (LIST[0] × LIST[1] × LIST[2]).
    pub(crate) fn max_hp(&mut self, map: usize) -> f64 {
        let str_total = crate::stats::total_stat_here(self, crate::Stat::Str);
        let talent0 = self.talent(0);
        let talent642 = self.talent(642);
        let talent95 = self.talent(95);
        let str_portion = (str_total * (1.0 + talent95 / 100.0)).powf(1.05);
        let statue4 = self.statue_bonus(4);
        let food = self.food_amount("HpBaseBoosts") * self.food_effect_multi() + statue4;
        let list0 = 15.0
            + self.card_bonus(1)
            + self.bubble("BaseHP")
            + self.stamp("BaseHP")
            + food
            + self.box_reward("baseHP")
            + talent0
            + talent642
            + str_portion;
        let mut list1 = (1.0
            + (self.talent(92) + self.talent(272) + self.decoder.etc(15, self.character)) / 100.0)
            * (1.0 + self.shrine(1, map) / 100.0);
        let gf = self.gold_food_bonus("MaxHPpct");
        if gf > 0.0 {
            list1 *= 1.0 + gf / 100.0;
        }
        list1 *= 1.0 - self.buff(108) / 100.0;
        list1 *= 1.0 + self.box_reward("pctHP") / 100.0;
        list1 *= 1.0 + (self.family(18) + self.card_bonus(8)) / 100.0;
        list1 *= 1.0 + self.star_sign_bonus("TotalHP") / 100.0;
        let list2 = if map > 19 && map < 24 { 164.0 } else { 1.0 };
        list0 * list1 * list2
    }

    /// `max-mp`: `_customBlock_PlayerMPmax`.
    pub(crate) fn max_mp(&mut self) -> f64 {
        let wis = crate::stats::total_stat_here(self, crate::Stat::Wis);
        let list0 = 10.0
            + self.card_bonus(3)
            + self.bubble("BaseMP")
            + self.stamp("BaseMP")
            + self.talent(1)
            + wis
            + self.box_reward("baseMP");
        let list1 = (1.0 + (self.talent(452) + self.talent(272)) / 100.0)
            * (1.0 + (self.box_reward("pctMP") + self.card_bonus(29)) / 100.0);
        list0 * list1
    }

    /// `movement-speed`: `PlayerSpeedBonus`.
    pub(crate) fn movement_speed(&mut self) -> f64 {
        let food = self.food_amount("MoveSpdBoosts") * self.food_effect_multi();
        let agi = crate::stats::total_stat_here(self, crate::Stat::Agi);
        let agi_scale = if agi < 1000.0 {
            ((agi + 1.0).powf(0.4) - 1.0) / 40.0
        } else {
            (agi - 1000.0) / (agi + 2500.0) * 0.5 + 0.371
        };
        let pct =
            food + self.talent(266) + self.stamp("PctMoveSpd") + self.option(438) + self.buff(273);
        let mut speed = (pct
            + self.statue_bonus(1)
            + self.star_sign_bonus("MoveSpd")
            + self.decoder.etc(1, self.character)
            + self.card_bonus(6)
            + self.talent(77))
            / 100.0
            + agi_scale / 2.2
            + 1.0;
        if speed <= 2.0 {
            let salt = self.salt_lick(7);
            let chip = self.chip_bonus("move");
            let talent = self.talent(641);
            let sigil = self.sigil(13);
            speed = if speed > 1.75 {
                (speed + talent / 100.0).min(2.0)
            } else {
                (speed + (salt + chip + talent + sigil) / 100.0).min(1.75)
            };
        }
        (100.0 * speed).floor() / 100.0
    }
}

impl Ctx<'_, '_> {
    /// `computeConstMasteryBonus(2)`: the tower-level mastery share.
    fn const_mastery_for(&self, kind: usize) -> f64 {
        if self.rift_level() < 40.0 {
            return 0.0;
        }
        let total: f64 = (0..27)
            .map(|index| fields::at(self.save(), "Tower", &[index]).unwrap_or(0.0))
            .sum();
        let threshold = [250.0, 500.0, 750.0, 1000.0, 1250.0, 1500.0, 2500.0];
        match kind {
            0 => (total / 10.0).floor().max(0.0),
            1 => (2.0 * ((total - threshold[2]) / 10.0).floor()).max(0.0),
            2 => (5.0 * ((total - threshold[4]) / 10.0).floor()).max(0.0),
            3 => {
                if total >= threshold[1] {
                    35.0
                } else {
                    0.0
                }
            }
            4 => {
                if total >= threshold[3] {
                    100.0
                } else {
                    0.0
                }
            }
            5 => {
                if total >= threshold[5] {
                    100.0
                } else {
                    0.0
                }
            }
            6 if total >= threshold[6] => 30.0,
            6 => 0.0,
            _ => 0.0,
        }
    }

    /// `computeOwnedItemCount(name)`: chest quantity plus refinery storage.
    fn owned_item_count(&self, name: &str) -> f64 {
        let order = fields::field(self.save(), "ChestOrder")
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let qty = fields::field(self.save(), "ChestQuantity")
            .and_then(|value| fields::objects_row(&value))
            .unwrap_or_default();
        let mut total = 0.0;
        for (index, item) in order.iter().enumerate() {
            if item.as_str() == Some(name) {
                total += fields::number_at(&qty, index);
            }
        }
        let refinery = fields::field(self.save(), "Refinery");
        if let Some(refinery) = refinery {
            let names =
                idleon_save::json::index(&refinery, &[1]).and_then(|v| v.as_array().cloned());
            let stored =
                idleon_save::json::index(&refinery, &[2]).and_then(|v| v.as_array().cloned());
            if let (Some(names), Some(stored)) = (names, stored) {
                for (index, item) in names.iter().enumerate() {
                    if item.as_str() == Some(name) {
                        total += stored
                            .get(index)
                            .and_then(idleon_save::json::number)
                            .unwrap_or(0.0);
                    }
                }
            }
        }
        total
    }

    /// `computePlayerBuildSpd(character)`.
    fn player_build_spd(&mut self, character: usize) -> f64 {
        let level = fields::char_field(self.save(), "Lv0", character)
            .and_then(|value| idleon_save::json::index(&value, &[8]).cloned())
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let previous = self.character;
        self.character = character;
        let base = 3.0 * (level / 2.0 + 0.7).powf(1.6);
        let bubble = self.bubble("Construction");
        let bubble_multi = 1.0 + level * bubble / 100.0;
        let post_office = fields::char_field(self.save(), "POu", character)
            .and_then(|value| fields::objects_row(&value))
            .map(|row| fields::number_at(&row, 17))
            .unwrap_or(0.0);
        let add = self.stamp("BuildProd")
            + 0.25 * post_office
            + self.guild(5)
            + self.decoder.etc(30, character)
            + (5.0 * self.decoder.achievement(153)).min(5.0)
            + self.const_mastery_for(2)
            + self.vial("Contspd")
            + self.decoder.arcade(44)
            + self.decoder.voting(18)
            + self.summ_upg(48) * idleon_account::vault_killz::vault_killz(self.decoder)[11]
            + self.bubba_rog(1);
        let win = self.decoder.win_bonus_without_options(13);
        let true_multi = (1.0 + win / 100.0)
            * (1.0 + self.decoder.palette(25) / 100.0)
            * (1.0 + self.vial("6turtle") / 100.0);
        let talent = {
            let raw = self.decoder.talent_level(character, 131);
            if raw > 0.0 {
                self.decoder.talent_param(131).map_or(1.0, |param| {
                    let value = curve::eval(&param.formula, param.x1, param.x2, raw);
                    let atom =
                        fields::at(self.save(), "Atoms", &[1]).unwrap_or(0.0) * self.data.atom(1);
                    let refinery = self.owned_item_count("Refinery1");
                    1.0 + value * (atom + curve::log(refinery)) / 100.0
                })
            } else {
                1.0
            }
        };
        self.character = previous;
        base * bubble_multi * (1.0 + add / 100.0) * true_multi * talent
    }

    /// `computeArtifactBonus(8)`: the build-speed logarithm.
    fn artifact_build_speed(&mut self) -> f64 {
        let mut total = 0.0;
        for character in 0..self.characters() {
            total += self.player_build_spd(character);
        }
        curve::log(total.max(1.0))
    }
}
