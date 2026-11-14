//! The save-derived systems the stat pipeline reads, ported from the upstream
//! `js/stats/systems/` modules.
//!
//! Every provider resolves against the same [`account::Decoder`] the other
//! sections use; a system whose formula is not ported records itself in
//! [`Ctx::missing`] instead of returning a number nobody can justify.

use crate::data::{self, Data};
use idleon_account::Decoder;
use idleon_common::curve;
use idleon_save::fields;
use idleon_stats_data as stats;

/// One resolution context: the save decoder, the static tables and the terms
/// this build could not justify.
pub(crate) struct Ctx<'a, 'b> {
    pub decoder: &'b mut Decoder<'a>,
    pub data: &'static Data,
    pub character: usize,
    pub missing: Vec<String>,
    /// `totalTomePoints`, computed on first use.
    pub(crate) tome: Option<f64>,
}

impl<'a, 'b> Ctx<'a, 'b> {
    pub(crate) fn new(decoder: &'b mut Decoder<'a>, character: usize) -> Self {
        Self {
            decoder,
            data: data::data(),
            character,
            missing: Vec::new(),
            tome: None,
        }
    }

    /// Records a term this build does not model, once.
    pub(crate) fn miss(&mut self, term: &str, source: &str) {
        let note = format!("{term}: not modelled ({source})");
        if !self.missing.contains(&note) {
            self.missing.push(note);
        }
    }

    pub(crate) fn save(&self) -> &'a serde_json::Map<String, serde_json::Value> {
        self.decoder.save()
    }

    /// `Lv0_<char>[index]`, the character's class level or skill row.
    pub(crate) fn level(&self, index: usize) -> f64 {
        self.decoder
            .characters()
            .get(self.character)
            .map_or(0.0, |row| row.skill(index))
    }

    /// `Lv0_<char>[index]`: one raw row entry, for the upstream indices that
    /// are read directly rather than through the skill numbering.
    pub(crate) fn raw_level(&self, index: usize) -> f64 {
        self.decoder
            .characters()
            .get(self.character)
            .map_or(0.0, |row| row.raw(index).unwrap_or(0.0))
    }

    /// `Lv0_<char>[0]`, the class level.
    pub(crate) fn class_level(&self) -> f64 {
        self.decoder
            .characters()
            .get(self.character)
            .map_or(0.0, |row| row.class_level())
    }

    /// `CharacterClass_<char>`.
    pub(crate) fn class(&self) -> usize {
        fields::char_field(self.save(), "CharacterClass", self.character)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .max(0.0) as usize
    }

    /// `optionsListData[index]`, the saved account progress.
    pub(crate) fn option(&self, index: usize) -> f64 {
        self.decoder.option(index)
    }

    /// How many character rows the save carries.
    pub(crate) fn characters(&self) -> usize {
        self.decoder
            .characters()
            .len()
            .max(self.decoder.save_character_count())
    }

    /// `getTalentNumber(1, index)`: the active character's talent value.
    pub(crate) fn talent(&mut self, index: usize) -> f64 {
        self.talent_tab(index, 1)
    }

    /// `getTalentNumber(1, index)` on one talent tab.
    pub(crate) fn talent_tab(&mut self, index: usize, tab: usize) -> f64 {
        let raw = self.decoder.talent_level(self.character, index);
        if raw <= 0.0 {
            return 0.0;
        }
        let level = raw
            + idleon_account::talent::bonus_levels(
                self.decoder,
                index,
                self.character,
                self.character,
                false,
                false,
            );
        let param = if tab == 2 {
            self.decoder.talent_param2(index)
        } else {
            self.decoder.talent_param(index)
        };
        let Some(param) = param else {
            return 0.0;
        };
        curve::eval(&param.formula, param.x1, param.x2, level)
    }

    /// `getbonus2(index, …, mode: 'max')`: the best character's talent value.
    pub(crate) fn talent_best(&mut self, index: usize) -> f64 {
        idleon_account::talent::max_bonus(self.decoder, index, self.character)
    }

    /// A talent's value on the active character (`GetTalentNumber(1, id)`).
    pub(crate) fn talent_here(&mut self, index: usize) -> f64 {
        let level = self.decoder.talent_level(self.character, index);
        if level <= 0.0 {
            return 0.0;
        }
        let Some(param) = self.decoder.talent_param(index) else {
            return 0.0;
        };
        curve::eval(&param.formula, param.x1, param.x2, level)
    }

    /// `computeStampBonusOfTypeX(type)`, with the exalted doubler chain.
    pub(crate) fn stamp(&mut self, kind: &str) -> f64 {
        let doubler = self.stamp_doubler();
        let lab_double = if self.decoder.mainframe(7) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        let pristine = 1.0 + self.decoder.pristine(17) / 100.0;
        let Some(levels) = fields::field(self.save(), "StampLv") else {
            return 0.0;
        };
        let mut total = 0.0;
        for stamp in self.data.stamps(kind) {
            let row = fields::object_row(&levels, stamp.cat).unwrap_or_default();
            let level = fields::number_at(&row, stamp.idx);
            if level <= 0.0 {
                continue;
            }
            let mut value = curve::eval(&stamp.formula, stamp.x1, stamp.x2, level);
            if self.stamp_exalted(stamp.cat, stamp.idx) {
                value *= 1.0 + doubler / 100.0;
            }
            if stamp.cat < 2 {
                value *= lab_double * pristine;
            }
            total += value;
        }
        // `BaseDmg`, `BaseHP`, `BaseAcc` and `BaseDef` stamps take the vault 16
        // multiplier after the loop; every `Eff` key takes talent 625.
        if matches!(kind, "BaseDmg" | "BaseHP" | "BaseAcc" | "BaseDef") {
            let vault = self.decoder.vault(16);
            if vault > 0.0 {
                total *= 1.0 + vault / 100.0;
            }
        }
        if kind.contains("Eff") {
            total *= self.talent(625).max(1.0);
        }
        total
    }

    /// `isExalted`: the saved Compass row 4 lists the stamp's letter and index.
    fn stamp_exalted(&self, cat: usize, idx: usize) -> bool {
        let letter = ["_", "a", "b", "c"].get(cat).copied().unwrap_or("_");
        let key = format!("{letter}{idx}");
        fields::row(self.save(), "Compass", &[4])
            .map(|row| row.len())
            .is_some()
            && fields::field(self.save(), "Compass")
                .and_then(|value| idleon_save::json::index(&value, &[4]).cloned())
                .and_then(|row| row.as_array().cloned())
                .is_some_and(|cells| cells.iter().any(|cell| cell.as_str() == Some(&key)))
    }

    /// `computeStampDoublerSources().total`.
    fn stamp_doubler(&mut self) -> f64 {
        let atom12 = fields::at(self.save(), "Atoms", &[12]).unwrap_or(0.0);
        let pristine20 = self.decoder.pristine(20);
        let compass = fields::at(self.save(), "Compass", &[0, 76]).unwrap_or(0.0);
        let compass76 = compass
            * stats::capacity()
                .compass_per_level
                .get(76)
                .copied()
                .unwrap_or(0.0);
        let emperor = idleon_account::sets::set_bonus(self.decoder, "EMPEROR_SET", self.character);
        let event18 = 20.0 * self.decoder.event_shop(18);
        let palette23 = self.decoder.palette(23);
        let exotic49 = self.decoder.exotic(49);
        let spelunk43 = fields::at(self.save(), "Spelunk", &[4, 3])
            .unwrap_or(0.0)
            .round();
        let legend36 = self.decoder.legend(36);
        let sushi17 = self.decoder.rog_qty(17);
        let jelly50 = self.decoder.jelly(50) / 100.0;
        100.0
            + atom12
            + pristine20
            + compass76
            + emperor
            + event18
            + palette23
            + exotic49
            + spelunk43
            + legend36
            + sushi17
            + jelly50
    }

    /// `bubbleValByKey(key)`: the value a descriptor reads a bubble by, before
    /// the stat pipeline's slab and Tome post-passes.
    pub(crate) fn bubble(&mut self, key: &str) -> f64 {
        self.bubble_inner(key, false, false)
    }

    /// `computeAlchBubble(bonusType)`: the value `computeTotalStat` reads.
    ///
    /// Same bubble, then the post-passes the game applies only inside the stat
    /// pipeline: the slab multiplier for the W2/W4 slider bubbles, the Tome
    /// multiplier for W8/A9/M9, the class-level and family-level passes, and
    /// the class zeroing of mismatched bubbles.
    pub(crate) fn bubble_total(&mut self, key: &str) -> f64 {
        let base = self.bubble_inner(key, false, true);
        if base <= 0.0 {
            return 0.0;
        }
        if class_zeroed(key, self.class()) {
            return 0.0;
        }
        let mut value = base;
        if matches!(key, "W2" | "W4" | "A2" | "A4" | "M2" | "M4") {
            let discoveries = fields::len_of(self.save(), "Cards1");
            value *= (discoveries / 100).max(1) as f64;
        }
        if matches!(key, "W8" | "A9" | "M9") {
            let tome = self.tome_points();
            value *= (((tome - 5000.0) / 2000.0).floor()).max(1.0);
            let troll = if self.decoder.ola_text(379).contains("TROLL_SET") {
                idleon_account::sets::set_bonus(self.decoder, "TROLL_SET", self.character)
            } else {
                0.0
            };
            value *= 1.0 + (self.decoder.grimoire_level(17) + troll) / 100.0;
        }
        if matches!(key, "W7" | "A8" | "M7") {
            value *= (((self.class_level() - 500.0) / 10.0).floor()).max(1.0);
        }
        let family = match key {
            "W9AllCharz" => Some(0usize),
            "A7AllCharz" => Some(1),
            "M8AllCharz" => Some(2),
            _ => None,
        };
        if let Some(family) = family {
            let levels = self.family_level_total(family);
            value *= 1.0 + 4.0 * levels / (levels + 1000.0);
        }
        if let Some(big) = self
            .data
            .big_bubbles
            .get(self.bubble_cauldron(key))
            .cloned()
            && big.key != key
            && let Some(bubble) = self.data.bubble(key)
            && big.indices.contains(&bubble.index)
        {
            value *= self.bubble(&big.key).max(1.0);
        }
        value
    }

    /// The `Tome` score, computed once per context.
    pub(crate) fn tome_points(&mut self) -> f64 {
        if let Some(points) = self.tome {
            return points;
        }
        let points = self.decoder.tome_points().unwrap_or(0.0);
        self.tome = Some(points);
        points
    }

    fn bubble_cauldron(&self, key: &str) -> usize {
        self.data.bubble(key).map_or(0, |bubble| bubble.cauldron)
    }

    /// `_familyLevelTotal(family)`: the summed class levels of one branch.
    fn family_level_total(&self, family: usize) -> f64 {
        let mut total = 0.0;
        for character in 0..self.characters() {
            let class = fields::char_field(self.save(), "CharacterClass", character)
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            let branch = if class < 6.0 {
                None
            } else if class < 18.0 {
                Some(0)
            } else if class < 30.0 {
                Some(1)
            } else if class < 42.0 {
                Some(2)
            } else {
                None
            };
            if branch == Some(family) {
                total += fields::char_field(self.save(), "Lv0", character)
                    .and_then(|value| idleon_save::json::index(&value, &[0]).cloned())
                    .and_then(|value| idleon_save::json::number(&value))
                    .unwrap_or(0.0);
            }
        }
        total
    }

    fn bubble_inner(&mut self, key: &str, skip_class: bool, skip_big: bool) -> f64 {
        let Some(bubble) = self.data.bubble(key).cloned() else {
            if !key.is_empty() {
                self.miss(&format!("Bubble: {key}"), "js/stats/systems/w2/alchemy.js");
            }
            return 0.0;
        };
        let level = fields::field(self.save(), "CauldronInfo")
            .and_then(|value| fields::object_row(&value, bubble.cauldron))
            .map(|row| fields::number_at(&row, bubble.index))
            .unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        // Big bubbles count only while the game has them equipped.
        if key.contains("ACTIVE") && self.decoder.companion(4) != 1.0 {
            let letter = ["_", "a", "b", "c"][bubble.cauldron.min(3)];
            let needle = format!("{letter}{}", bubble.index);
            let equipped = fields::field(self.save(), "CauldronBubbles")
                .and_then(|rows| idleon_save::json::index(&rows, &[self.character]).cloned())
                .and_then(|row| row.as_array().cloned())
                .is_some_and(|cells| cells.iter().any(|cell| cell.as_str() == Some(&needle)));
            if !equipped {
                return 0.0;
            }
        }
        let mut value = curve::eval(&bubble.formula, bubble.x1, bubble.x2, level);
        if self
            .decoder
            .bubble_prismad_at(bubble.cauldron, bubble.index)
        {
            value *= self.decoder.prisma_bonus_multi().max(1.0);
        }
        let class = self.class();
        if !skip_class
            && class > 6
            && bubble.index != 16
            && (1..30).contains(&bubble.index)
            && !key.contains("passz")
            && !key.contains("ACTIVE")
            && !key.contains("AllCharz")
        {
            let pass = match bubble.cauldron {
                0 if class < 18 && key != "Construction" => Some("Opassz"),
                1 if (18..30).contains(&class) => Some("Gpassz"),
                2 if (30..42).contains(&class) => Some("Ppassz"),
                _ => None,
            };
            if let Some(pass) = pass {
                value *= self.bubble(pass).max(1.0);
            }
        }
        if !skip_big
            && let Some(big) = self.data.big_bubbles.get(bubble.cauldron).cloned()
            && big.indices.contains(&bubble.index)
            && big.key != key
        {
            value *= self.bubble(&big.key).max(1.0);
        }
        value
    }

    /// `computeVialByKey(key)`.
    pub(crate) fn vial(&mut self, key: &str) -> f64 {
        let lab = if self.decoder.mainframe(10) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        let rift = self.rift_level();
        let rift_active = rift > 34.0;
        let maxed = if rift_active {
            self.max_level_vials()
        } else {
            0.0
        };
        let d_nzz = 2.0 * maxed + self.decoder.vault(42);
        let meritoc20 = self.decoder.meritoc(20);
        let mut total = 0.0;
        for vial in self.data.vials(key).to_vec() {
            let level = fields::field(self.save(), "CauldronInfo")
                .and_then(|value| fields::object_row(&value, 4))
                .map(|row| fields::number_at(&row, vial.index))
                .unwrap_or(0.0);
            if level <= 0.0 {
                continue;
            }
            total += lab
                * (1.0 + d_nzz / 100.0)
                * (1.0 + meritoc20 / 100.0)
                * curve::eval(&vial.formula, vial.x1, vial.x2, level);
        }
        total
    }

    fn max_level_vials(&self) -> f64 {
        let Some(rows) = fields::field(self.save(), "CauldronInfo")
            .and_then(|value| fields::object_row(&value, 4))
        else {
            return 0.0;
        };
        rows.iter()
            .filter(|level| level.unwrap_or(0.0) >= 13.0)
            .count() as f64
    }

    /// `sigil.resolve(index)`.
    pub(crate) fn sigil(&mut self, index: usize) -> f64 {
        let Some(level) = fields::at(self.save(), "CauldronP2W", &[4, 1 + 2 * index]) else {
            return 0.0;
        };
        if level < -0.1 {
            return 0.0;
        }
        let Some(tiers) = self.data.sigil(index) else {
            self.miss(&format!("Sigil {index}"), "js/stats/data/common/sigils.js");
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
        let tier16 = fields::at(self.save(), "Sailing", &[3, 16]).unwrap_or(0.0);
        let artifact = 1.0 + if tier16 == 0.0 { 0.0 } else { tier16.max(1.0) };
        let meritoc = 1.0 + self.decoder.meritoc(21) / 100.0;
        base * artifact * meritoc
    }

    pub(crate) fn rift_level(&self) -> f64 {
        fields::at(self.save(), "Rift", &[0]).unwrap_or(0.0)
    }

    /// `getEnabledStarSigns`.
    pub(crate) fn enabled_star_signs(&mut self) -> f64 {
        if self.rift_level() >= 10.0 {
            5.0 + self.decoder.shiny_bonus(3)
        } else {
            0.0
        }
    }

    /// The signs the character carries: its saved row plus every unlocked sign
    /// inside the Rift's infinite range.
    pub(crate) fn star_sign_active(&mut self, sign: usize, enabled: f64) -> bool {
        let equipped = fields::char_field(self.save(), "PVtStarSign", self.character)
            .map(|value| match value {
                serde_json::Value::String(text) => text,
                other => other.to_string(),
            })
            .unwrap_or_default();
        if equipped.split(',').any(|row| row == sign.to_string()) {
            return true;
        }
        if (sign as f64) >= enabled {
            return false;
        }
        let Some(row) = self.data.star_sign(sign) else {
            return false;
        };
        let name = row.first().cloned().unwrap_or_default();
        self.star_signs_unlocked()
            .is_some_and(|unlocked| unlocked.contains_key(&name))
    }

    /// `starSignsUnlocked`: the save writes `StarSg` as a string or as one
    /// character per key, and the loader joins the characters before parsing.
    fn star_signs_unlocked(&self) -> Option<serde_json::Map<String, serde_json::Value>> {
        let raw = self.save().get("StarSg")?.clone();
        let text = match raw {
            serde_json::Value::String(text) => text,
            serde_json::Value::Object(rows) => {
                let mut keys: Vec<(usize, String)> = rows
                    .iter()
                    .filter_map(|(key, value)| {
                        Some((key.parse().ok()?, value.as_str()?.to_string()))
                    })
                    .collect();
                keys.sort_by_key(|(index, _)| *index);
                keys.into_iter().map(|(_, cell)| cell).collect()
            }
            _ => return None,
        };
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()?
            .as_object()
            .cloned()
    }

    /// `computeSeraphMulti`.
    pub(crate) fn seraph_multi(&mut self) -> f64 {
        let unlocked = self
            .star_signs_unlocked()
            .is_some_and(|unlocked| unlocked.contains_key("Seraph_Cosmos"));
        if !unlocked {
            return 1.0;
        }
        let arcane40 = fields::at(self.save(), "Arcane", &[40])
            .unwrap_or(0.0)
            .min(10.0);
        let summon = self.level(18);
        let base = 1.1 + arcane40 / 100.0;
        let steps = ((summon + 1.0) / 20.0).ceil();
        let seraph = (base.powf(steps)).min(5.0);
        let chip = self.has_star_chip();
        let enabled = self.rift_level() >= 10.0;
        let chip_multi = if chip && enabled { 2.0 } else { 1.0 };
        let meritoc = 1.0 + self.decoder.meritoc(22) / 100.0;
        chip_multi * meritoc * seraph
    }

    fn has_star_chip(&self) -> bool {
        (0..7).any(|slot| fields::at(self.save(), "Lab", &[1 + self.character, slot]) == Some(15.0))
    }

    /// `computePositiveStarSignMultiplier`.
    pub(crate) fn positive_star_sign_multi(&mut self) -> f64 {
        let enabled = self.enabled_star_signs();
        let equipped_pass = if self.has_star_chip() && enabled < 1.0 {
            2.0
        } else {
            1.0
        };
        equipped_pass * self.seraph_multi()
    }

    /// `computeStarSignBonus(key)`.
    pub(crate) fn star_sign_bonus(&mut self, key: &str) -> f64 {
        let Some(map) = self.data.star_sign_bonuses.get(key).cloned() else {
            self.miss(
                &format!("Star Signs: {key}"),
                "js/stats/systems/common/starSign.js",
            );
            return 0.0;
        };
        let enabled = self.enabled_star_signs();
        let mut total = 0.0;
        for (sign, value) in map {
            let sign: usize = sign.parse().unwrap_or(0);
            if key == "MobRespawn" && sign == 49 && self.class_level() < 60.0 {
                continue;
            }
            if !self.star_sign_active(sign, enabled) {
                continue;
            }
            if value < 0.0 && (sign as f64) < enabled {
                continue;
            }
            total += value;
        }
        if total > 0.0 {
            total *= self.positive_star_sign_multi();
        }
        total
    }

    /// `computeStarSignStatBonuses(stat)`: the flat and percent totals parsed
    /// from each active sign's description strings.
    pub(crate) fn star_sign_stat(&mut self, stat: &str) -> (f64, f64) {
        let enabled = self.enabled_star_signs();
        let mut flat = 0.0;
        let mut percent = 0.0;
        for index in 0..self.data.star_signs.len() {
            if !self.star_sign_active(index, enabled) {
                continue;
            }
            let row = self.data.star_signs[index].clone();
            for effect in row.iter().skip(1) {
                let Some((value, is_pct, kind)) = parse_sign_effect(effect) else {
                    continue;
                };
                if kind != stat && kind != "All_Stats" && kind != "All_Stat" {
                    continue;
                }
                if is_pct {
                    percent += value;
                } else {
                    flat += value;
                }
            }
        }
        let multi = self.positive_star_sign_multi();
        (flat * multi, percent * multi)
    }

    /// `computeCardBonusByType(typeId)`.
    pub(crate) fn card_bonus(&mut self, type_id: usize) -> f64 {
        let Some(desc) = self.data.card_desc(type_id).map(str::to_owned) else {
            return 0.0;
        };
        let legend = 1.0 + self.decoder.legend(21) / 100.0;
        let equipped = fields::char_field(self.save(), "CardEquip", self.character)
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default();
        let mut total = 0.0;
        for (slot, card) in equipped.iter().enumerate() {
            let Some(key) = card.as_str() else {
                continue;
            };
            if key.is_empty() || key == "B" {
                continue;
            }
            let Some(bonus) = self.data.cards.bonus.get(key).cloned() else {
                continue;
            };
            if bonus.desc != desc {
                continue;
            }
            let level = self.decoder.card_level(key);
            if level <= 0.0 {
                continue;
            }
            let chip = if (slot == 0 && self.decoder.chip_count(self.character, 1) > 0.0)
                || (slot == 7 && self.decoder.chip_count(self.character, 2) > 0.0)
            {
                2.0
            } else {
                1.0
            };
            total += chip * level * bonus.val * legend;
        }
        total
    }

    /// `computeBoxReward(charIdx, key)`.
    pub(crate) fn box_reward(&mut self, key: &str) -> f64 {
        let Some(points) = fields::char_field(self.save(), "POu", self.character) else {
            return 0.0;
        };
        for (index, row) in self.data.box_rewards.iter().enumerate() {
            for slot in 0..3 {
                if row.get(16 + slot).and_then(|cell| cell.as_str()) != Some(key) {
                    continue;
                }
                let mut points =
                    fields::number_at(&fields::values(&points).unwrap_or_default(), index).round();
                if slot == 1 {
                    points -= row
                        .get(13)
                        .and_then(idleon_save::json::number)
                        .unwrap_or(0.0)
                        .round();
                }
                if slot == 2 {
                    points -= row
                        .get(14)
                        .and_then(idleon_save::json::number)
                        .unwrap_or(0.0)
                        .round();
                }
                if points <= 0.0 {
                    return 0.0;
                }
                let base = 1 + slot * 4;
                let x1 = row
                    .get(base)
                    .and_then(idleon_save::json::number)
                    .unwrap_or(0.0);
                let x2 = row
                    .get(base + 1)
                    .and_then(idleon_save::json::number)
                    .unwrap_or(0.0);
                let formula = row.get(base + 2).and_then(|v| v.as_str()).unwrap_or("");
                return curve::eval(formula, x1, x2, points);
            }
        }
        0.0
    }

    /// `computeFlurboShop(index)` from `js/stats/systems/w2/dungeon.js`.
    pub(crate) fn flurbo_shop(&mut self, index: usize) -> f64 {
        let level = fields::at(self.save(), "DungUpg", &[5, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let Some(param) = self.data.dung_passive(index) else {
            return 0.0;
        };
        curve::eval(&param.formula, param.x1, param.x2, level)
    }

    /// `guildBonusParams(index)` evaluated at the save's guild points.
    pub(crate) fn guild(&mut self, index: usize) -> f64 {
        let Some(points) = fields::at(self.save(), "Guild", &[0, index]) else {
            return 0.0;
        };
        if points <= 0.0 {
            return 0.0;
        }
        let Some(param) = self.data.guild(index) else {
            return 0.0;
        };
        curve::eval(&param.formula, param.x1, param.x2, points)
    }

    /// `achieveStatus(index)`: the tiered value of a completed achievement.
    pub(crate) fn achievement(&self, index: usize) -> f64 {
        let completed = fields::field(self.save(), "AchieveReg")
            .and_then(|value| idleon_save::json::index(&value, &[index]).cloned())
            .and_then(|value| idleon_save::json::number(&value));
        if completed != Some(-1.0) {
            return 0.0;
        }
        self.data.achievement(index).max(1.0)
    }

    /// `computeFamBonusQTYs`: the best class-family bonus per key.
    ///
    /// `result` is keyed by `2 * class + type`; the game keeps the maximum
    /// across every character's class chain and applies talent 144 when the
    /// active character is that winner.
    pub(crate) fn family_bonuses(&mut self) -> std::collections::BTreeMap<usize, f64> {
        let mut result = std::collections::BTreeMap::new();
        let raw_144 = self.decoder.talent_level(self.character, 144);
        for character in 0..self.characters() {
            let class = fields::char_field(self.save(), "CharacterClass", character)
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0) as usize;
            if class == 0 {
                continue;
            }
            let level = fields::char_field(self.save(), "Lv0", character)
                .and_then(|value| idleon_save::json::index(&value, &[0]).cloned())
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0);
            if level <= 0.0 {
                continue;
            }
            for class in class_chain(class) {
                let Some(family) = self.decoder.family_bonus(class) else {
                    continue;
                };
                let effective = (level - family.lv_offset).round().max(0.0);
                for kind in 0..2 {
                    let bonus = curve::eval(&family.formula, family.x1, family.x2, effective);
                    let key = 2 * class + kind;
                    if result.get(&key).is_some_and(|stored| bonus <= *stored) {
                        continue;
                    }
                    let mut value = bonus;
                    if key == 24 || key == 44 {
                        let best =
                            idleon_account::talent::max_bonus(self.decoder, 144, self.character);
                        value = bonus * (1.0 + best / 100.0);
                    } else if raw_144 > 0.0 && character == self.character {
                        // The game computes this lazily, reading FamBonusQTYs[68]
                        // as far as the loop has built it.
                        let family68 = Some(result.get(&68).copied().unwrap_or(0.0));
                        let levels = raw_144
                            + idleon_account::talent::bonus_levels_with_family68(
                                self.decoder,
                                144,
                                self.character,
                                self.character,
                                false,
                                false,
                                family68,
                            );
                        if let Some(param) = self.decoder.talent_param(144) {
                            let talent = curve::eval(&param.formula, param.x1, param.x2, levels);
                            value = bonus * (1.0 + talent / 100.0);
                        }
                    }
                    result.insert(key, value);
                }
            }
        }
        result
    }

    /// `computeAllStatPCT`: `0.1 * floor(10 * Σ terms)`.
    pub(crate) fn all_stat_pct(&mut self) -> f64 {
        let mut sum = 0.0;
        // 1. Vial 49 (PEARL_SELTZER), key AllStatPCT.
        let mut vials = 0.0;
        let lab = if self.decoder.mainframe(10) == Some(2.0) {
            2.0
        } else {
            1.0
        };
        let rift = self.rift_level();
        let maxed = if rift > 34.0 {
            self.max_level_vials()
        } else {
            0.0
        };
        let d_nzz = if rift > 34.0 { 2.0 * maxed } else { 0.0 } + self.decoder.vault(42);
        let meritoc20 = self.decoder.meritoc(20);
        for vial in self.data.vials("AllStatPCT").to_vec() {
            let level = fields::field(self.save(), "CauldronInfo")
                .and_then(|value| fields::object_row(&value, 4))
                .map(|row| fields::number_at(&row, vial.index))
                .unwrap_or(0.0);
            if level <= 0.0 {
                continue;
            }
            vials += lab
                * (1.0 + d_nzz / 100.0)
                * (1.0 + meritoc20 / 100.0)
                * curve::eval(&vial.formula, vial.x1, vial.x2, level);
        }
        sum += vials;
        // 2. Companion 0 with Cosmo (2, 0).
        let doot = if self.decoder.companion(0) == 1.0 {
            1.0
        } else {
            0.0
        };
        let cosmo = fields::at(self.save(), "Holes", &[6, 0]).unwrap_or(0.0);
        sum += self.data.companion_base(0) * doot * (self.data.cosmo(2, 0) * cosmo).floor();
        // 3. Mainframe 104.
        sum += self.decoder.mainframe(104).unwrap_or(0.0);
        // 4. AllStatPct stamps.
        sum += self.stamp("AllStatPct");
        // 5. Cards: All Stats.
        sum += self.card_bonus(82);
        // 6. Summoning win bonus 18, with no endless-win options.
        sum += self.decoder.win_bonus_without_options(18);
        // 7. Family bonus 72.
        sum += self.family_bonuses().get(&72).copied().unwrap_or(0.0);
        // 8. Sailing artifact 28.
        let tier = fields::at(self.save(), "Sailing", &[3, 28]).unwrap_or(0.0);
        if tier > 0.0 {
            sum += self.data.artifact(28) * tier;
        }
        // 9. Golden food "AllStatz".
        sum += self.gold_food_bonus("AllStatz");
        // 10. Achievement 309.
        sum += self.achievement(309);
        // 11. min(15, log(OLA[172]) * talent 653).
        let log = if self.option(172) > 0.0 {
            curve::log(self.option(172))
        } else {
            0.0
        };
        sum += (log * self.talent(653)).min(15.0);
        // 12. Achievement 362.
        sum += self.achievement(362);
        // 13. Highest sneaking mastery.
        sum += 10.0 * ((98.0 + self.option(232)) / 100.0).floor();
        // 14. Farming rank 19.
        sum += self.farm_rank(19);
        // 15. Voting 2. A save alone has no server vote, so `votingBonusz`
        // reads zero exactly as upstream does without `serverVars`.
        sum += self.decoder.voting(2);
        // 16. The Marbiglass set.
        sum += idleon_account::sets::set_bonus(self.decoder, "MARBIGLASS_SET", self.character);
        0.1 * (10.0 * sum).floor()
    }

    /// `farm.resolve('rank19')`: the Land Rank factor.
    pub(crate) fn farm_rank(&mut self, rank: usize) -> f64 {
        let per_level = self.data_farm_rank(rank);
        let level = fields::at(self.save(), "FarmRank", &[2, rank]).unwrap_or(0.0);
        if per_level <= 0.0 || level <= 0.0 {
            return 0.0;
        }
        let talent = self.talent_best(207).max(1.0);
        let exotic = 1.0 + self.decoder.exotic(14) / 100.0;
        talent * exotic * per_level * level
    }

    /// `ninjaInfo(36)[rank]`: the Land Rank bonus per level.
    fn data_farm_rank(&self, rank: usize) -> f64 {
        self.data.farm_rank_per_level(rank)
    }

    /// `gfoodBonusMULTI`: the golden-food multiplier.
    ///
    /// The chain reads the Emporium's food slots, the voting multiplier
    /// descriptor and the `calcTalentMAP209` map scan; this build models the
    /// save-derived terms and reports the rest.
    pub(crate) fn gold_food_multi(&mut self) -> f64 {
        let companion174 = self.decoder.companion(174);
        let set = 1.0
            + (idleon_account::sets::set_bonus(self.decoder, "SECRET_SET", self.character)
                + 50.0 * companion174)
                / 100.0;
        let family = self
            .family_bonuses()
            .get(&66)
            .copied()
            .unwrap_or(1.0)
            .max(1.0);
        let talent99 = self.talent(99);
        let stamp = self.stamp("GFood");
        let bubble = self.bubble("O18");
        let sigil = self.sigil(14);
        let meal = self.meal_bonus("zGoldFood");
        let stars = self.star_sign_bonus("GFood");
        let bribe = self.bribe(36);
        let pristine = self.decoder.pristine(14);
        let voting = self.decoder.voting(26);
        let talent209 = self.talent_best(209) * self.map209();
        let companion48 = self.decoder.companion(48);
        let legend25 = self.decoder.legend(25);
        let crop_card = (4.0 * self.decoder.card_level("cropfallEvent1")).min(50.0);
        let anni_card = (5.0 * self.decoder.card_level("anni5Event1")).min(50.0);
        let companion155 = self.decoder.companion(155);
        let jelly = self.decoder.jelly(10) + self.decoder.jelly(51);
        let vault86 = self.decoder.vault(86);
        let rest = self.decoder.etc(8, self.character)
            + talent99
            + stamp
            + self.achievement_tiered(37)
            + bubble
            + sigil
            + meal
            + stars
            + bribe
            + pristine
            + 2.0 * self.achievement_tiered(380)
            + 3.0 * self.achievement_tiered(383)
            + voting
            + talent209
            + companion48
            + legend25
            + crop_card
            + anni_card
            + companion155
            + 10_000.0 * companion174
            + jelly
            + vault86;
        set * (family + rest / 100.0)
    }

    /// `achieveStatusTiered`, the same rule as [`Self::achievement`].
    fn achievement_tiered(&self, index: usize) -> f64 {
        self.achievement(index)
    }

    /// `calcTalentMAP209`: the maps a Death Bringer has pushed past 1B kills.
    fn map209(&mut self) -> f64 {
        let mut holder = None;
        for character in 0..self.characters() {
            let class = fields::char_field(self.save(), "CharacterClass", character)
                .and_then(|value| idleon_save::json::number(&value))
                .unwrap_or(0.0) as usize;
            // Upstream tests `CLASS_TREES[class][3] === 10`: the fourth entry
            // of the chain, which only the Death Bringer branches carry.
            if idleon_account::class_tree(class).is_some_and(|tree| tree.get(3) == Some(&10u32)) {
                holder = Some(character);
            }
        }
        let Some(holder) = holder else {
            return 0.0;
        };
        let Some(kla) = fields::char_field(self.save(), "KLA", holder) else {
            return 0.0;
        };
        let maps: Vec<usize> = match &kla {
            serde_json::Value::Array(rows) => (0..rows.len()).collect(),
            serde_json::Value::Object(rows) => {
                rows.keys().filter_map(|map| map.parse().ok()).collect()
            }
            _ => return 0.0,
        };
        let mut count = 0.0;
        for map in maps {
            if !crate::is_fighting_map(map) {
                continue;
            }
            let Some(left) = fields::object_row(&kla, map) else {
                continue;
            };
            let done =
                stats::kill_requirement(map) - left.first().copied().flatten().unwrap_or(0.0);
            if done >= 1e9 {
                count += 1.0;
            }
        }
        count
    }

    /// `goldFoodBonuses(effect)`: the equipped golden food's own value.
    ///
    /// Upstream also pays the Emporium's own food slots; the save's `Ninja`
    /// row 104 carries them and is read here when present.
    pub(crate) fn gold_food_bonus(&mut self, effect: &str) -> f64 {
        let multi = self.gold_food_multi();
        let slots = fields::char_field(self.save(), "FoodSlO", self.character)
            .and_then(|value| idleon_save::json::number(&value))
            .unwrap_or(0.0)
            .round()
            .max(0.0) as usize;
        let Some(order) = fields::char_field(self.save(), "EquipOrder", self.character) else {
            return 0.0;
        };
        let qtys = fields::char_field(self.save(), "EquipQTY", self.character)
            .and_then(|value| fields::object_row(&value, 2))
            .unwrap_or_default();
        let names = field_row(&order, 2).map(value_cells).unwrap_or_default();
        let mut total = 0.0;
        for index in 0..slots {
            let Some(name) = names.get(index).and_then(|value| value.as_str()) else {
                continue;
            };
            let Some(info) = self.data.gold_food(name).cloned() else {
                continue;
            };
            if info.effect != effect {
                continue;
            }
            let qty = fields::number_at(&qtys, index);
            if qty <= 0.0 {
                continue;
            }
            let log = curve::log(1.0 + qty);
            total = info.amount * multi * 0.05 * log * (1.0 + log / 2.14);
        }
        // The Emporium's own food slots scale with `1000 * 10^level`.
        let emporium = self.decoder.emporium(1) > 0.0;
        if emporium {
            for (index, name) in self.data.gold_food.slots.iter().enumerate() {
                let Some(info) = self.data.gold_food(name).cloned() else {
                    continue;
                };
                if info.effect != effect {
                    continue;
                }
                let level = fields::at(self.save(), "Ninja", &[104, index]).unwrap_or(0.0);
                if level > 0.0 {
                    let qty = 1000.0 * 10f64.powf(level);
                    let log = curve::log(1.0 + qty);
                    total += info.amount * multi * 0.05 * log * (1.0 + log / 2.14);
                }
                break;
            }
        }
        total
    }

    /// `computeMealBonus(effect)`.
    pub(crate) fn meal_bonus(&mut self, effect: &str) -> f64 {
        self.decoder.meal(effect)
    }

    /// `getBribeBonus(index)`.
    pub(crate) fn bribe(&mut self, index: usize) -> f64 {
        if fields::at(self.save(), "BribeStatus", &[index]) != Some(1.0) {
            return 0.0;
        }
        stats::capacity().bribes.get(index).copied().unwrap_or(0.0)
    }

    /// `computeOwlBonus(index)`.
    pub(crate) fn owl(&mut self, index: usize) -> f64 {
        let base = self.data.owl(index);
        let progress = self.option(255);
        let count = ((progress - index as f64) / 6.0).ceil().max(0.0);
        if count <= 0.0 {
            return 0.0;
        }
        let legend = 1.0 + self.decoder.legend(26) / 100.0;
        let companion = 1.0 + self.decoder.companion(51);
        let megafeather = self.option(262);
        let feather = |threshold: f64, weight: f64| -> f64 {
            if megafeather > threshold { weight } else { 0.0 }
        };
        let ninth = if megafeather > 9.0 { 100.0 } else { 0.0 };
        let ninth_more = 50.0 * (megafeather - 10.0).max(0.0);
        let feathers = feather(1.0, 100.0)
            + feather(3.0, 100.0)
            + feather(5.0, 100.0)
            + feather(7.0, 100.0)
            + ninth
            + ninth_more;
        base * legend * companion * (1.0 + feathers / 100.0) * count
    }

    /// `computeStatueBonusGiven(index, character)`.
    pub(crate) fn statue_bonus(&mut self, index: usize) -> f64 {
        let level = fields::at(self.save(), "StatueLevels", &[self.character, index, 0])
            .or_else(|| {
                fields::field(self.save(), &format!("StatueLevels_{}", self.character))
                    .and_then(|value| fields::object_row(&value, index))
                    .map(|row| row.first().copied().flatten().unwrap_or(0.0))
            })
            .unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let mut value = level * self.data.statue(index);
        let talents: &[usize] = match index {
            0 | 2 => &[112, 127],
            7 => &[127],
            8 => &[112],
            1 | 11 => &[292, 307],
            9 => &[292],
            14 => &[307],
            10 | 6 => &[487, 472],
            12 => &[487],
            13 => &[472],
            3 | 5 | 17 => &[37],
            _ => &[],
        };
        for talent in talents {
            value *= (1.0 + self.talent_here(*talent) / 100.0).max(1.0);
        }
        let gold = fields::at(self.save(), "StuG", &[index]).unwrap_or(0.0);
        if gold >= 2.0 {
            let tier = fields::at(self.save(), "Sailing", &[3, 30]).unwrap_or(0.0);
            let art = if tier > 0.0 {
                self.data.artifact(30) * tier.max(1.0)
            } else {
                0.0
            };
            value *= (1.0 + (100.0 + art) / 100.0).max(1.0);
        }
        if gold >= 3.0 {
            let zenith = fields::at(self.save(), "Spelunk", &[45, 0]).unwrap_or(0.0);
            value *= (1.0 + (50.0 + (2.0 * zenith).floor()) / 100.0).max(1.0);
        }
        if [0, 1, 2, 6].contains(&index) {
            value *= (1.0 + self.decoder.vault(25) / 100.0).max(1.0);
        }
        if index != 29 {
            value *= (1.0 + self.statue_bonus(29) / 100.0).max(1.0);
        }
        let event = self.decoder.event_shop(19);
        value *= 1.0 + 0.3 * event;
        value *= (1.0 + self.talent_best(56) / 100.0).max(1.0);
        value *= 1.0 + self.decoder.meritoc(26) / 100.0;
        value
    }
}

/// A row of a top-level field, accepting numeric-keyed objects as well.
fn field_row(value: &serde_json::Value, index: usize) -> Option<&serde_json::Value> {
    match value {
        serde_json::Value::Array(rows) => rows.get(index),
        serde_json::Value::Object(rows) => rows.get(&index.to_string()),
        _ => None,
    }
}

/// A row's cells, whichever shape the save wrote them in.
fn value_cells(row: &serde_json::Value) -> Vec<serde_json::Value> {
    match row {
        serde_json::Value::Array(cells) => cells.clone(),
        serde_json::Value::Object(cells) => {
            let mut keys: Vec<(usize, &serde_json::Value)> = cells
                .iter()
                .filter_map(|(key, value)| Some((key.parse().ok()?, value)))
                .collect();
            keys.sort_by_key(|(index, _)| *index);
            keys.into_iter().map(|(_, value)| value.clone()).collect()
        }
        _ => Vec::new(),
    }
}

/// `ReturnClasses(classId)`: the class and its ancestors.
fn class_chain(class: usize) -> Vec<usize> {
    let data = data::data();
    let mut chain = vec![class];
    let mut current = class;
    while let Some(parent) = data.class_parent(current) {
        chain.insert(0, parent);
        current = parent;
    }
    chain
}

/// `_classZeroedBubble`: the bubbles a class branch never pays.
fn class_zeroed(key: &str, class: usize) -> bool {
    let zeroed: &[&str] = if class < 7 {
        &["W1", "A1", "A5", "M5"]
    } else if class < 18 {
        &["A1", "A5", "A6", "M1", "M5", "M6"]
    } else if class < 30 {
        &["M1", "M5", "M6", "W1", "W5", "W6"]
    } else {
        &["W1", "W5", "W6", "A1", "A5", "A6"]
    };
    zeroed.contains(&key)
}

/// The `+N` / `+N%` effect strings the stat page parses.
fn parse_sign_effect(effect: &str) -> Option<(f64, bool, &str)> {
    let rest = effect.strip_prefix('+')?;
    let split = rest.find('_')?;
    let (number, kind) = rest.split_at(split);
    let kind = kind.trim_start_matches('_');
    let (number, percent) = match number.strip_suffix('%') {
        Some(number) => (number, true),
        None => (number, false),
    };
    Some((number.parse().ok()?, percent, kind))
}
