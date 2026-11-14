//! The account-wide systems the sneaking breakdown trees read.
//!
//! Ported from `js/stats/systems/w6/sneaking-stealth.js` (`_computeFactors`),
//! `sneaking-jade.js` (`computeJadeCoin`), `sneaking-exp.js`
//! (`sneakingExpAccountInputs`) and the helpers they call. The values are
//! resolved once when the save loads; a system the save does not carry reads
//! as its neutral value and the tree node names the field.

use super::Decoder;
use idleon_common::curve as formula;
use idleon_save::fields;
use idleon_sneaking::data::{self, Curve};
use idleon_sneaking::{Account, DoorAccount, ExpAccount, JadeAccount};
use serde_json::Value;

/// The character index every account term is resolved against.
///
/// Upstream picks the character with the strongest core stealth
/// (`findBestActiveChar`); the account terms only need a stable login context,
/// and the first character is the saved default.
const LOGIN: usize = 0;

impl Decoder<'_> {
    /// The breakdown terms the save carries.
    #[allow(clippy::field_reassign_with_default)]
    pub(crate) fn sneaking_account(&mut self, loadout: &idleon_sneaking::Loadout) -> Account {
        let tables = &data::data().account;
        let mut account = Account::default();
        account.talent_432 = self.talent_value(432).max(1.0);
        account.farm_rank_bonus = self.farm_rank_bonus();
        account.farm_level = self
            .characters
            .get(LOGIN)
            .and_then(|character| character.farming_level())
            .map_or(0.0, f64::from);
        account.compass_45 = self.compass(45);
        account.gambit_11 = 0.0;
        account.bubble_a10 = self.sneaking_bubble_value(&tables.bubbles.a10);
        account.star_sign_73 = self.star_sign(&tables.star_sign_bonuses.stealth);
        account.statue_26 = self.statue(26).max(0.0);
        account.card_crystal5_level = self.card_level("Crystal5");
        account.achievement_368 = self.achievement(368);
        account.voting_25 = self.voting(25);
        account.lamp_21 = self.lamp(2, 1);
        account.hole_upgrade_54 = self.hole_upgrade_54();
        account.fractal_unlocked = if self.ola(184) >= 40_000.0 { 1.0 } else { 0.0 };
        account.emperor_0 = self.emperor(0);
        account.rog_32 = self.rog_qty(32);
        account.jelly_16 = self.jelly(16);
        account.companion_163 = self.companion(163);
        account.bubble_y9 = self.sneaking_bubble_value(&tables.bubbles.y9);
        account.jade = self.jade_account(loadout);
        account.exp = self.exp_account(loadout);
        account.door = DoorAccount {
            y8: self.sneaking_bubble_value(&tables.bubbles.y8),
            gemstone_2: self.gemstone(2),
            true_battering: idleon_sneaking::ninja_bonus(
                &self.ninja_knowledge(),
                &self.menagerie_omni(),
                27,
            ),
            untie_vial: self.vial("6Untie"),
        };
        account
    }

    #[allow(clippy::field_reassign_with_default)]
    fn jade_account(&mut self, loadout: &idleon_sneaking::Loadout) -> JadeAccount {
        let tables = &data::data().account;
        let mut jade = JadeAccount::default();
        jade.gem_shop = 2_f64.powf(self.row_at("GemItemsPurchased", 10)).max(1.0);
        jade.monument = self.sneaking_monument(2, 2);
        jade.pristine = self.pristine_charm(13);
        jade.vial = self.vial("6Jade");
        jade.meal = self.meal("zJade");
        jade.card_level = self.card_level("w6b4");
        jade.killroy = self.killroy(2);
        jade.slab = self.slabbo(4);
        jade.stamp = self.stamp("JadeCoin");
        jade.crop = self.crop_scientist(2);
        jade.win = self.win_bonus(1);
        jade.msa = self.msa(7);
        jade.sigil = self.sigil(23);
        jade.arcade = self.arcade(35);
        jade.vault = self.vault(81);
        jade.star = self.star_sign(&tables.star_sign_bonuses.jade);
        jade.task_base = tables.task_shop_jade_base;
        jade.task_level = self.nested("TaskZZ2", &[5, 3]);
        jade.rift = if self.rift_skill_rank(16) > 1 {
            1.0
        } else {
            0.0
        };
        jade.compass = self.compass(41);
        jade.companion_163 = self.companion(163);
        jade.achievement_366 = self.achievement(366);
        jade.achievement_367 = self.achievement(367);
        jade.achievement_369 = self.achievement(369);
        jade.gemstone_1 = self.gemstone(1);
        jade.meritoc = self.meritoc(6);
        jade.exotic = self.exotic(58);
        let gold = loadout.gold_bonuses();
        jade.gold_9 = gold.get(9);
        jade.gold_23 = gold.get(23);
        jade
    }

    #[allow(clippy::field_reassign_with_default)]
    fn exp_account(&mut self, loadout: &idleon_sneaking::Loadout) -> ExpAccount {
        let tables = &data::data().account;
        let mut exp = ExpAccount::default();
        exp.all_skill_multi = (1.0 + self.meritoc(10) / 100.0)
            * (1.0 + self.legend(20) / 100.0)
            * (1.0 + self.companion(32));
        exp.arcade_52 = self.arcade(52);
        exp.compass_49 = self.compass(49);
        exp.gemstone_4 = self.gemstone(4);
        exp.vial = self.vial("6SneakEXP");
        exp.meal = self.meal("zSneakExp");
        exp.rift = if self.rift_skill_rank(16) > 0 {
            1.0
        } else {
            0.0
        };
        exp.pristine = if self.nested("Ninja", &[107, 7]) == 1.0 {
            idleon_opal::data::pristine(7)
        } else {
            0.0
        };
        exp.card_level = self.card_level("w6a4");
        exp.mainframe = self.mainframe(16).unwrap_or(0.0);
        exp.stamp = self.stamp("SneakExp");
        exp.star = self.star_sign(&tables.star_sign_bonuses.sneak_exp);
        exp.guild = formula::eval(
            &tables.guild14.formula,
            tables.guild14.x1,
            tables.guild14.x2,
            self.nested("Guild", &[0, 14]),
        );
        exp.talent_add = self.talent_value(431);
        exp.talent_multi = self.talent_value_mode(431, 2).max(1.0);
        exp.achievement_370 = 10.0 * self.achievement(370);
        exp.vote = self.voting(25);
        exp.vault = self.vault(82);
        exp.win = self.win_bonus(6);
        exp.companion_163 = self.companion(163);
        exp.gold_10 = loadout.gold_bonuses().get(10);
        exp
    }

    // ---- the helpers upstream calls ----

    /// `bubbleValByKey` for one of the sneaking data's bubbles.
    fn sneaking_bubble_value(&self, bubble: &idleon_sneaking::data::BubbleParam) -> f64 {
        let param = super::data::BubbleParam {
            cauldron: bubble.cauldron,
            index: bubble.index,
            x1: bubble.x1,
            x2: bubble.x2,
            formula: bubble.formula.clone(),
        };
        self.bubble_value(&param)
    }

    /// `maxTalentBonus(idx)` without the All-Talent levels.
    fn talent_value(&self, index: usize) -> f64 {
        let Some(curve) = self.talent_curve(index, 1) else {
            return 0.0;
        };
        let level = (0..self.save_character_count())
            .map(|character| self.talent_level(character, index))
            .fold(0.0, f64::max);
        formula::eval(&curve.formula, curve.x1, curve.x2, level)
    }

    /// `maxTalentBonusDetail(idx, mode: 2)`.
    fn talent_value_mode(&self, index: usize, mode: usize) -> f64 {
        let Some(curve) = self.talent_curve(index, mode) else {
            return 0.0;
        };
        let level = (0..self.save_character_count())
            .map(|character| self.talent_level(character, index))
            .fold(0.0, f64::max);
        formula::eval(&curve.formula, curve.x1, curve.x2, level)
    }

    fn talent_curve(&self, index: usize, mode: usize) -> Option<Curve> {
        let tables = &data::data().account.talents;
        match (index, mode) {
            (56, _) => Some(tables.statue.clone()),
            (207, _) => Some(tables.farm_rank.clone()),
            (432, _) => Some(tables.talent_432.clone()),
            (431, 2) => Some(tables.talent431tab2.clone()),
            (431, _) => Some(tables.sneaking_exp_add.clone()),
            _ => None,
        }
    }

    /// `FarmRankUpgBonus(4)`: the Land Rank factor on the stealth tree.
    fn farm_rank_bonus(&self) -> f64 {
        let level = self.nested("FarmRank", &[2, 4]);
        if level <= 0.0 {
            return 0.0;
        }
        let per_level = data::data()
            .info
            .farm_rank_per_level
            .get(4)
            .copied()
            .unwrap_or(0.0);
        let talent = self.talent_value(207).max(1.0);
        let exotic = 1.0 + self.exotic(14) / 100.0;
        talent * exotic * per_level * level
    }

    /// `votingBonusz(idx, votingMulti)`: the vote's bonus while it is the
    /// server-wide winner.
    pub fn voting(&self, index: usize) -> f64 {
        if self.active_vote() != Some(index) {
            return 0.0;
        }
        let base = data::data()
            .account
            .voting_bonuses
            .get(index * 3 + 1)
            .copied()
            .unwrap_or(0.0);
        base * self.voting_multi()
    }

    /// `saveData.activeVoteIdx`: the winning vote from
    /// `serverVars.voteCategories[0]`, which a save alone does not carry.
    fn active_vote(&self) -> Option<usize> {
        let vote = self
            .root
            .get("serverVars")?
            .get("voteCategories")?
            .as_array()?
            .first()
            .and_then(idleon_save::json::number)?;
        let whole = vote >= 0.0 && vote.floor() == vote && vote <= usize::MAX as f64;
        whole.then_some(vote as usize)
    }

    /// The `voting-multi` descriptor.
    fn voting_multi(&self) -> f64 {
        let inner = self.companion(41)
            + self.row_at("Dream", 13)
            + self.cosmo(2, 3)
            + self.win_bonus(22)
            + self.event_shop(17)
            + self.event_shop(13)
            + self.companion(19)
            + self.palette(32)
            + self.legend(22)
            + self.rog_qty(50);
        (1.0 + self.companion(161) / 100.0)
            * (1.0 + self.meritoc(9) / 100.0)
            * (1.0 + inner / 100.0)
    }

    /// `computeCosmoBonus(tier, idx)`.
    fn cosmo(&self, tier: usize, index: usize) -> f64 {
        let level = fields::at(self.save, "Holes", &[4 + tier, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        (idleon_opal::data::cosmo_base(tier, index) * level).floor()
    }

    /// `computeLampBonus(t, 1)`.
    pub fn lamp(&self, row: usize, column: usize) -> f64 {
        const GRID: [[f64; 3]; 5] = [
            [25.0, 10.0, 8.0],
            [15.0, 40.0, 10.0],
            [20.0, 35.0, 12.0],
            [5.0, 1.0, 1.0],
            [2.0, 2.0, 2.0],
        ];
        let level = fields::at(self.save, "Holes", &[21, 4 + 2 * row]).unwrap_or(0.0);
        let zenith_level = self.spelunk(45, 2);
        let zenith = (data::data().account.zenith_per_level * zenith_level).floor();
        GRID.get(row)
            .and_then(|cells| cells.get(column))
            .copied()
            .unwrap_or(0.0)
            * level
            * (1.0 + zenith / 100.0)
    }

    /// `B_UPG(54)`: the Hole upgrade that multiplies stealth.
    fn hole_upgrade_54(&self) -> f64 {
        let purchased = fields::at(self.save, "Holes", &[13, 54]).unwrap_or(0.0);
        if purchased <= 0.0 {
            return 1.0;
        }
        let value = fields::at(self.save, "Holes", &[9, 15]).unwrap_or(0.0);
        1.2_f64.powf(idleon_common::curve::log(value).floor())
    }

    /// `computeEmperorBon(idx)`.
    fn emperor(&self, index: usize) -> f64 {
        let count = self.ola(369).max(0.0) as usize;
        let value = self
            .data
            .emperor_bonus_val
            .get(index)
            .copied()
            .unwrap_or(0.0);
        let sum: f64 = (0..count)
            .filter(|round| {
                self.data.emperor_bonus_type.get(round % 48).copied() == Some(index as f64)
            })
            .map(|_| value)
            .sum();
        let multi = 1.0 + (self.arcane(48) + self.arcade(51) + self.jelly(28)) / 100.0;
        (sum * multi).floor()
    }

    /// `gemstoneBonus(idx)` for the account terms.
    fn gemstone(&self, index: usize) -> f64 {
        let Some(def) = data::item(&format!("NjGem{index}")) else {
            return 0.0;
        };
        let level = self.ola(233 + index).max(0.0);
        if level <= 0.5 {
            return 0.0;
        }
        let base: f64 = def.stat.trim().parse().unwrap_or(0.0);
        let value = base + def.max * (level / (1000.0 + level));
        if index == 5 {
            return value;
        }
        let first = self.gemstone(5);
        let talent = self.talent_value(432).max(1.0);
        value * (1.0 + first / 100.0) * talent
    }

    /// `computeMonumentROGbonus(row, index)`.
    fn sneaking_monument(&self, row: usize, index: usize) -> f64 {
        let slot = 10 * row + index;
        let level = fields::at(self.save, "Holes", &[15, slot]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let info = data::data()
            .account
            .monument_info
            .get(slot)
            .copied()
            .unwrap_or(0.0);
        if info <= 0.0 {
            return 0.0;
        }
        let mut holeoz = 1.0;
        if index != 9 {
            let cosmo = (idleon_opal::data::cosmo_base(0, 0)
                * fields::at(self.save, "Holes", &[4, 0]).unwrap_or(0.0))
            .floor();
            holeoz = 1.0 + self.sneaking_monument(row, 9) / 100.0 + cosmo / 100.0;
        }
        let fountain = self.fountain(row, 13);
        holeoz = (holeoz * (1.0 + fountain / 100.0)).max(1.0);
        if info < 30.0 {
            level * info * holeoz
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * info * holeoz).ceil()
        }
    }

    /// `computeVialByKey(effect)`.
    fn vial(&mut self, effect: &str) -> f64 {
        let Some(pipelines) = fields::field(self.save, "CauldronInfo") else {
            return 0.0;
        };
        let vial_levels = fields::object_row(&pipelines, 4).unwrap_or_default();
        let lab_mult = if self.mainframe(10) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        let rift_active = self.row_at("Rift", 0) > 34.0;
        let max_level_vials = if rift_active {
            vial_levels
                .iter()
                .filter(|level| level.unwrap_or(0.0) >= 13.0)
                .count() as f64
        } else {
            0.0
        };
        let dnzz = 2.0 * max_level_vials + self.vault(42);
        let meritoc = self.meritoc(20);
        data::data()
            .account
            .vials
            .iter()
            .filter(|vial| vial.effect == effect)
            .map(|vial| {
                let level = fields::number_at(&vial_levels, vial.index);
                if level <= 0.0 {
                    return 0.0;
                }
                let raw = formula::eval(&vial.formula, vial.x1, vial.x2, level);
                lab_mult * (1.0 + dnzz / 100.0) * (1.0 + meritoc / 100.0) * raw
            })
            .sum()
    }

    /// `computeMealBonus(effect)`.
    pub fn meal(&mut self, effect: &str) -> f64 {
        let Some(meals) = fields::field(self.save, "Meals") else {
            return 0.0;
        };
        let levels = fields::object_row(&meals, 0).unwrap_or_default();
        let Some(mastery) = fields::field(self.save, "CookMaster") else {
            return 0.0;
        };
        let mastery = fields::object_row(&mastery, 0).unwrap_or_default();
        let cook = self
            .cooking_meal_multi()
            .map_or(1.0, |cooking| cooking.value);
        data::data()
            .account
            .meals
            .iter()
            .filter(|meal| meal.effect == effect)
            .map(|meal| {
                let level = fields::number_at(&levels, meal.index);
                if level <= 0.0 {
                    return 0.0;
                }
                let mastery_level = fields::number_at(&mastery, meal.index);
                let mastery_multi = 1.0 + mastery_level / (mastery_level + 5.0);
                let ribbon = self.ribbon(28 + meal.index);
                mastery_multi * cook * ribbon * level * meal.per_level
            })
            .sum()
    }

    /// `computeStampBonusOfTypeX(type)`.
    pub fn stamp(&mut self, kind: &str) -> f64 {
        let Some(stamps) = data::data().account.stamp_types.get(kind) else {
            return 0.0;
        };
        let Some(levels) = fields::field(self.save, "StampLv") else {
            return 0.0;
        };
        let lab_double = if self.mainframe(7) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        let pristine = 1.0 + self.pristine(17) / 100.0;
        stamps
            .iter()
            .map(|stamp| {
                let row = fields::object_row(&levels, stamp.cat).unwrap_or_default();
                let level = fields::number_at(&row, stamp.idx);
                if level <= 0.0 {
                    return 0.0;
                }
                let value = formula::eval(&stamp.formula, stamp.x1, stamp.x2, level);
                if stamp.cat < 2 {
                    value * lab_double * pristine
                } else {
                    value
                }
            })
            .sum()
    }

    /// `computeSlabboBonus(4)`.
    fn slabbo(&mut self, index: usize) -> f64 {
        let cards = fields::field(self.save, "Cards1")
            .and_then(|value| value.as_array().map(Vec::len))
            .unwrap_or(0) as f64;
        let mainframe = self.mainframe(15).unwrap_or(0.0);
        let multi = (1.0 + self.meritoc(23) / 100.0)
            * (1.0 + self.legend(28) / 100.0)
            * (1.0 + self.vault(74) / 100.0);
        match index {
            4 => {
                if self.emporium(17) == 0.0 {
                    0.0
                } else {
                    5.0 * (1.0 + mainframe / 100.0)
                        * multi
                        * ((cards - 1000.0).max(0.0) / 10.0).floor()
                }
            }
            5 => {
                if self.emporium(18) == 0.0 {
                    0.0
                } else {
                    3.0 * (1.0 + mainframe / 100.0)
                        * multi
                        * ((cards - 1000.0).max(0.0) / 10.0).floor()
                }
            }
            6 => {
                if self.superbit(25) == 0.0 {
                    0.0
                } else {
                    3.0 * (1.0 + mainframe / 100.0)
                        * multi
                        * ((cards - 1300.0).max(0.0) / 5.0).floor()
                }
            }
            7 => {
                if self.superbit(34) == 0.0 {
                    0.0
                } else {
                    0.1 * (1.0 + mainframe / 100.0)
                        * multi
                        * ((cards - 1300.0).max(0.0) / 5.0).floor()
                }
            }
            _ => 0.0,
        }
    }

    /// `computeCropSC(idx)`.
    pub fn crop_scientist(&mut self, index: usize) -> f64 {
        const GATES: [usize; 10] = [27, 24, 25, 26, 23, 28, 29, 38, 40, 44];
        let gate = GATES.get(index).copied().unwrap_or(usize::MAX);
        if self.emporium(gate) == 0.0 {
            return 0.0;
        }
        let crops = fields::field(self.save, "FarmCrop")
            .and_then(|value| value.as_object().map(serde_json::Map::len))
            .unwrap_or(0) as f64;
        let multi = (1.0 + self.mainframe(17).unwrap_or(0.0) / 100.0)
            * (1.0 + (self.grimoire_22() + self.exotic(40) + self.vault(79)) / 100.0);
        match index {
            0 => 20.0 * crops * multi,
            1 => 1.02_f64.powf(crops) * multi,
            2 => 8.0 * crops * multi,
            3 => 1.1_f64.powf(crops) * multi,
            4 => 15.0 * crops * multi,
            5 => 7.0 * crops * multi,
            6 => 0.1 * crops * multi,
            7 => (crops - 100.0).max(0.0) * multi,
            8 => 5.0 * (crops - 200.0).max(0.0) * multi,
            9 => ((crops - 200.0) / 10.0).floor().max(0.0) * multi,
            _ => 0.0,
        }
    }

    /// `computeKillroyBonus(idx)`.
    fn killroy(&self, index: usize) -> f64 {
        let (value, base, scale) = match index {
            0 => (228, 300.0, 1.0),
            1 => (229, 300.0, 9.0),
            2 => (230, 300.0, 2.0),
            3 => (467, 200.0, 10.0),
            4 => (468, 200.0, 1.3),
            5 => (469, 150.0, 0.8),
            6 => (470, 250.0, 25.0),
            7 => (471, 200.0, 2.0),
            _ => return 1.0,
        };
        let value = self.ola(value);
        if index == 3 || index == 6 {
            value / (base + value) * scale
        } else {
            1.0 + value / (base + value) * scale
        }
    }

    /// `computeMSABonus(idx)`.
    fn msa(&self, index: usize) -> f64 {
        let Some(config) = data::data().account.msa.get(index) else {
            return 0.0;
        };
        let unlocked = match config.gate {
            0 => self.superbit(config.gate_idx) > 0.0,
            _ => self.emporium(config.gate_idx) > 0.0,
        };
        if !unlocked {
            return 0.0;
        }
        let stars: f64 = fields::row(self.save, "TotemInfo", &[0])
            .map(|row| row.iter().map(|cell| cell.unwrap_or(0.0)).sum())
            .unwrap_or(0.0);
        config.mult * ((stars - config.offset) / 10.0).floor().max(0.0)
    }

    /// `sigilBonus(idx)`.
    fn sigil(&mut self, index: usize) -> f64 {
        let Some(sigil) = fields::row(self.save, "CauldronP2W", &[4]) else {
            return 0.0;
        };
        let level = fields::number_at(&sigil, 1 + 2 * index);
        if level < -0.1 {
            return 0.0;
        }
        let Some(tiers) = data::data()
            .account
            .sigils
            .get(index)
            .and_then(Option::as_ref)
        else {
            return 0.0;
        };
        let base = if level < 0.5 {
            tiers[0]
        } else if level < 1.5 {
            tiers[1]
        } else if level < 2.5 {
            tiers[2]
        } else if level < 3.5 {
            tiers[3]
        } else {
            tiers.get(4).copied().unwrap_or(tiers[3])
        };
        let tier16 = self.nested("Sailing", &[3, 16]);
        let artifact = 1.0 + if tier16 == 0.0 { 0.0 } else { tier16.max(1.0) };
        let meritoc = 1.0 + self.meritoc(21) / 100.0;
        base * artifact * meritoc
    }

    /// `computeRiftSkillBonus(skillIdx, threshold)`'s rank ladder.
    pub fn rift_skill_rank(&self, skill: usize) -> i32 {
        if self.row_at("Rift", 0) < 15.0 {
            return 0;
        }
        let mut total = 0.0;
        for character in 0..self.save_character_count() {
            total += self
                .characters
                .get(character)
                .map_or(0.0, |character| character.skill(skill));
        }
        data::data()
            .account
            .rift_ranks
            .iter()
            .filter(|threshold| total >= **threshold)
            .count() as i32
    }

    /// `computeStarSignBonus(key)` for one of the three trees.
    fn star_sign(&self, bonuses: &std::collections::HashMap<String, f64>) -> f64 {
        let enabled = if self.row_at("Rift", 0) >= 10.0 {
            5.0 + self.shiny_bonus(3)
        } else {
            0.0
        };
        let unlocked = self.star_sg();
        let equipped: Vec<String> = fields::char_field(self.save, "PVtStarSign", LOGIN)
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty() && *entry != "_")
            .map(str::to_owned)
            .collect();
        let names = &data::data().account.star_sign_names;
        let mut total = 0.0;
        for (index, value) in bonuses {
            let Ok(index) = index.parse::<usize>() else {
                continue;
            };
            let active = equipped.iter().any(|entry| entry == &index.to_string())
                || (index as f64 >= 0.0
                    && (index as f64) < enabled
                    && names.get(index).is_some_and(|name| {
                        unlocked
                            .as_ref()
                            .and_then(Value::as_object)
                            .is_some_and(|map| map.contains_key(name))
                    }));
            if active && !(*value < 0.0 && (index as f64) < enabled) {
                total += *value;
            }
        }
        if total > 0.0 {
            total *= self.positive_star_sign_multi();
        }
        total
    }

    /// The save's `StarSg` object, in either the decoded or the string form.
    fn star_sg(&self) -> Option<Value> {
        match self.save.get("StarSg")? {
            Value::String(_) => idleon_save::json::parsed_field(self.save, "StarSg"),
            other => Some(other.clone()),
        }
    }

    /// `computePositiveStarSignMultiplier`: the chip pass and Seraph Cosmos.
    fn positive_star_sign_multi(&self) -> f64 {
        let enabled = if self.row_at("Rift", 0) >= 10.0 {
            5.0 + self.shiny_bonus(3)
        } else {
            0.0
        };
        let chip = self.has_star_chip();
        let equipped_pass = if chip && enabled < 1.0 { 2.0 } else { 1.0 };
        equipped_pass * self.seraph_multi()
    }

    /// Whether the character wears the Silkrode Nanochip (chip 15).
    fn has_star_chip(&self) -> bool {
        fields::field(self.save, "Lab")
            .and_then(|value| fields::object_row(&value, 1 + LOGIN))
            .is_some_and(|row| (0..7).any(|slot| fields::number_at(&row, slot) == 15.0))
    }

    /// `computeSeraphMulti`.
    fn seraph_multi(&self) -> f64 {
        let unlocked = self
            .star_sg()
            .and_then(|value| {
                value
                    .as_object()
                    .map(|map| map.contains_key("Seraph_Cosmos"))
            })
            .unwrap_or(false);
        if !unlocked {
            return 1.0;
        }
        let arcane = self.row_at("Arcane", 40);
        let summon = fields::field(self.save, &format!("Lv0_{LOGIN}"))
            .and_then(|value| fields::values(&value))
            .map(|row| fields::number_at(&row, 18))
            .unwrap_or(0.0);
        let base = 1.1 + arcane.min(10.0) / 100.0;
        let exponent = ((summon + 1.0) / 20.0).ceil();
        let seraph = 5_f64.min(base.powf(exponent));
        let enabled = if self.row_at("Rift", 0) >= 10.0 {
            5.0
        } else {
            0.0
        };
        let chip = if self.has_star_chip() && enabled >= 1.0 {
            2.0
        } else {
            1.0
        };
        chip * (1.0 + self.meritoc(22) / 100.0) * seraph
    }

    /// `pristineCharmBonus(idx)`.
    fn pristine(&self, index: usize) -> f64 {
        idleon_opal::data::pristine(index)
    }

    /// `pristine.resolve(idx)`: the charm's bonus while `Ninja[107][idx]` holds
    /// it equipped.
    pub fn pristine_charm(&self, index: usize) -> f64 {
        if self.nested("Ninja", &[107, index]) == 1.0 {
            self.pristine(index)
        } else {
            0.0
        }
    }

    /// `Ninja[103]` and the Omni-Jade row, which the ninja-bonus terms read.
    fn ninja_knowledge(&self) -> Vec<f64> {
        fields::row(self.save, "Ninja", &[103])
            .map(|row| row.into_iter().map(|cell| cell.unwrap_or(0.0)).collect())
            .unwrap_or_default()
    }

    fn menagerie_omni(&self) -> Vec<f64> {
        fields::row(self.save, "Menagerie", &[7])
            .map(|row| row.into_iter().map(|cell| cell.unwrap_or(0.0)).collect())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use idleon_research::model::Simulation;
    use serde_json::{Map, json};

    fn voting(server_vars: Option<serde_json::Value>, index: usize) -> f64 {
        let save = Map::new();
        let root = json!({ "serverVars": server_vars })
            .as_object()
            .expect("an object")
            .clone();
        let simulation = Simulation::default();
        let mut warnings = Vec::new();
        let view = Decoder::new(&save, &root, &[], &simulation, &mut warnings);
        view.voting(index)
    }

    #[test]
    fn the_server_wide_winner_pays_its_vote_bonus() {
        // Vote 34's base is 52, and the multiplier chain is neutral on an empty save.
        assert_eq!(voting(Some(json!({ "voteCategories": [34] })), 34), 52.0);
        assert_eq!(voting(Some(json!({ "voteCategories": [34] })), 33), 0.0);
    }

    #[test]
    fn a_save_without_the_vote_data_pays_nothing() {
        assert_eq!(voting(None, 34), 0.0);
        assert_eq!(voting(Some(json!({ "voteCategories": [] })), 34), 0.0);
    }
}
