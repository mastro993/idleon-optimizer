//! `buildExt` for an imported save: the Opal Optimizer's villager state.
//!
//! `opal-calc.html` builds its account bonuses inline from a handful of
//! upstream systems. This module reads them with the account decoder, so the
//! game tables and the field names live in one place:
//!
//! - the villager rows are `Holes[1]` (levels), `Holes[3]` (opals) and
//!   `Holes[23]` (stars), with the cosmo levels in `Holes[4 + tier]`;
//! - the cross-system terms come from the decoder's own ports of compass,
//!   pristine charms, legend points, the vault, meritocracy, arcane and
//!   grimoire upgrades, the arcade, the exotic market, companions, the summing
//!   sigil, the event shop and the jar's collectibles;
//! - the monument, statue and measurement terms follow `hole.js`, `fountain.js`
//!   and the statue chain in `opal-calc.html` itself.
//!
//! A save without `Holes` has no villager state, and the section says so.

use super::Decoder;
use super::fields;
use crate::opal::data;
use crate::opal::{Ext, Opal, Terms, VILLAGERS, Villager};
use crate::save::json;
use serde_json::Value;

/// The jar collectibles the Villager EXP jar sums, in the page's order.
const JAR_COLLECTIBLES: [usize; 6] = [4, 10, 12, 22, 29, 35];

impl Decoder<'_> {
    /// `buildExt`: the villagers and every account bonus their EXP rates apply.
    pub(super) fn opal(&mut self) -> Option<Opal> {
        let holes = fields::field(self.save, "Holes")?;
        let holes = holes.as_array()?;

        let cell = |row: usize, column: usize| -> f64 {
            holes
                .get(row)
                .and_then(Value::as_array)
                .and_then(|cells| cells.get(column))
                .and_then(json::number)
                .unwrap_or(0.0)
        };
        let row = |index: usize| -> Vec<f64> {
            holes
                .get(index)
                .and_then(Value::as_array)
                .map(|cells| {
                    cells
                        .iter()
                        .map(|cell| json::number(cell).unwrap_or(0.0))
                        .collect()
                })
                .unwrap_or_default()
        };

        let villagers: [Villager; VILLAGERS] = std::array::from_fn(|villager| Villager {
            level: cell(1, villager),
            opals: cell(3, villager),
            stars: cell(23, villager),
        });
        let star_mul = villagers.map(|villager| 1.0 + villager.stars);
        let holeoz = std::array::from_fn(|villager| self.holeoz(villager, &villagers).max(1.0));

        let cosmo = [row(4), row(5)];
        let cosmo_level = |tier: usize, index: usize| {
            cosmo
                .get(tier)
                .and_then(|row| row.get(index))
                .copied()
                .unwrap_or(0.0)
        };
        let enthusiasm = (data::cosmo_base(1, 0) * cosmo_level(1, 0)).floor();
        let equal_spread = (data::cosmo_base(1, 5) * cosmo_level(1, 5)).floor();
        let cosmo_1_1 = (data::cosmo_base(1, 1) * cosmo_level(1, 1)).floor();
        let cosmo_1_2 = (data::cosmo_base(1, 2) * cosmo_level(1, 2)).floor();
        let scholars = row(13).iter().filter(|cell| **cell == 1.0).count() as f64;
        let lore_episode1 = self.lore_episode(1);

        let terms = Terms {
            base_plus_upg: 100.0 + if cell(13, 0) >= 1.0 { 25.0 } else { 0.0 },
            exotic: self.exotic(51),
            arcade: self.arcade(38),
            grimoire: self.grimoire(29),
            arcane: self.arcane(32),
            mon_bravery: self.monument(0, 3),
            mon_justice: self.monument(1, 3),
            mon_wisdom: self.monument(2, 3),
            meas_slab: self.measurement(7),
            meas_tome: self.measurement(0),
            b_upg48: 10.0 * cell(11, 26),
            card_cave_b: (4.0 * self.card_level("caveB")).min(50.0),
            bell1: cell(17, 1) * crate::fountain::bell_per_level(1),
            summon_win21: self.win_bonus_without_options(21),
            vault73: self.vault(73),
            cosmo_1_1,
            scholars,
            cosmo_1_2,
            compass59: self.compass(59),
            legend12: self.legend(12),
            pristine21: data::pristine(21),
            companion13: self.companion(13),
            statue28: self.statue(28),
            jar_sum: self.jar_collectibles(self.legend(29)),
            event_shop6: self.event_shop(6),
            lore_episode1,
            bundle_u: if self.bundle("bun_u") { 1.0 } else { 0.0 },
        };

        Some(Opal {
            villagers,
            cosmo,
            ext: Ext {
                star_mul,
                enthusiasm,
                equal_spread,
                holeoz,
                terms,
            },
        })
    }

    /// `holeozDxp`: the per-villager Holeoz multiplier.
    ///
    /// The Explorer's is 1.5^Rupie Slugs until thirteen caverns are owned; the
    /// Conjuror's reads the fountain's Cosmo EXP upgrade; the rest stay at 1.
    fn holeoz(&self, villager: usize, villagers: &[Villager; VILLAGERS]) -> f64 {
        match villager {
            0 => {
                if villagers[0].level.min(18.0) < 13.0 {
                    1.5_f64.powf(self.ola(355))
                } else {
                    1.0
                }
            }
            2 => 1.0 + self.fountain(0, 14) / 100.0,
            _ => 1.0,
        }
    }

    /// `_measBonus`: one measurement's bonus, with the page's own `_measMulti`.
    ///
    /// The opal page only implements measurement types 0 and 7; every other
    /// type reads as a multiplier of 1, as its `switch` does.
    fn measurement(&self, index: usize) -> f64 {
        let level = fields::at(self.save, "Holes", &[22, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let cosmo = (crate::fountain::cosmo_measurement()
            * fields::at(self.save, "Holes", &[5, 3]).unwrap_or(0.0))
        .floor();
        let Some((base, is_total)) = crate::fountain::measurement(index) else {
            return 0.0;
        };
        let base = if is_total {
            (1.0 + cosmo / 100.0) * (base * level / (100.0 + level))
        } else {
            (1.0 + cosmo / 100.0) * base * level
        };
        base * self.measurement_multi(index)
    }

    /// The page's `_measMulti`, for the two types it implements.
    fn measurement_multi(&self, index: usize) -> f64 {
        let qty = match crate::fountain::measurement_type(index) {
            0 => {
                let raw = fields::at(self.save, "Holes", &[11, 28]).unwrap_or(0.0);
                if raw > 0.0 {
                    crate::save::game_log(raw)
                } else {
                    0.0
                }
            }
            7 => {
                let high = fields::at(self.save, "TaskZZ0", &[1, 0]).unwrap_or(0.0);
                if high > 0.0 {
                    crate::save::game_log(high) / 2.0
                } else {
                    0.0
                }
            }
            _ => 0.0,
        };
        if qty < 5.0 {
            1.0 + 18.0 * qty / 100.0
        } else {
            1.0 + (18.0 * qty + 8.0 * (qty - 5.0)) / 100.0
        }
    }

    /// `computeCompassBonus`: a compass upgrade's value.
    fn compass(&self, index: usize) -> f64 {
        let level = fields::at(self.save, "Compass", &[0, index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let (per_level, conditional) = data::compass(index);
        if conditional {
            let multi = 1.0 + (self.compass(39) + self.compass(80)) / 100.0;
            return multi * level * per_level;
        }
        if index == 45 {
            return level * per_level * 2_f64.powf((level / 50.0).floor());
        }
        level * per_level
    }

    /// `grimoireUpgBonus`: a grimoire upgrade's value.
    fn grimoire(&self, index: usize) -> f64 {
        let level = fields::at(self.save, "Grimoire", &[index]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let (per_level, no_multi) = data::grimoire(index);
        if no_multi {
            return level * per_level;
        }
        level * per_level * (1.0 + self.grimoire(36) / 100.0)
    }

    /// The statue chain's `_statueBonusGiven(idx)`.
    ///
    /// The statue's own level × its per-level base, then the Zenith, artifact,
    /// vault, Dragon statue, event shop, All Statues talent and meritocracy
    /// factors, in the page's order.
    fn statue(&self, index: usize) -> f64 {
        let level = self.statue_level(index);
        if level <= 0.0 {
            return 0.0;
        }
        let mut value = level * data::statue_base(index);
        let grade = fields::at(self.save, "StuG", &[index]).unwrap_or(0.0);
        if grade >= 2.0 {
            let rarity = fields::at(self.save, "Sailing", &[3, 30]).unwrap_or(0.0);
            let artifact = if rarity > 0.0 {
                self.data.artifacts.get(30).copied().unwrap_or(0.0) * rarity
            } else {
                0.0
            };
            value *= (1.0 + (100.0 + artifact) / 100.0).max(1.0);
        }
        if grade >= 3.0 {
            let zenith = fields::at(self.save, "Spelunk", &[45, 0]).unwrap_or(0.0);
            let level: f64 = (data::zenith(0) * zenith).floor();
            value *= (1.0 + (50.0 + level) / 100.0).max(1.0);
        }
        if matches!(index, 0 | 1 | 2 | 6) {
            value *= (1.0 + self.vault(25) / 100.0).max(1.0);
        }
        if index != 29 {
            value *= (1.0 + self.statue(29) / 100.0).max(1.0);
        }
        value *= 1.0 + 0.3 * self.event_shop(19);
        value *= (1.0 + self.statue_talent() / 100.0).max(1.0);
        value *= 1.0 + self.meritoc(26) / 100.0;
        value
    }

    /// `statueData[idx]`: the first character's saved statue level.
    ///
    /// The game writes `StatueLevels_<char>` as `[level, exp]` pairs, and the
    /// page reads the first character's row.
    fn statue_level(&self, index: usize) -> f64 {
        fields::at(self.save, "StatueLevels_0", &[index, 0]).unwrap_or(0.0)
    }

    /// `maxTalentBonus(56, -1)`: the All Statues talent at its best character.
    fn statue_talent(&self) -> f64 {
        let curve = data::talent_curve();
        (0..self.save_character_count())
            .map(|character| curve.value(self.talent_level(character, 56)))
            .fold(0.0, f64::max)
    }

    /// A talent level: `SL_<char>[talent]`, written as an array or a keyed
    /// object.
    fn talent_level(&self, character: usize, talent: usize) -> f64 {
        let key = format!("SL_{character}");
        let Some(value) = fields::field(self.save, &key) else {
            return 0.0;
        };
        if let Some(cells) = value.as_array() {
            return cells.get(talent).and_then(json::number).unwrap_or(0.0);
        }
        value
            .as_object()
            .and_then(|fields| fields.get(&talent.to_string()))
            .and_then(json::number)
            .unwrap_or(0.0)
    }

    /// How many characters the save's talent rows cover.
    fn save_character_count(&self) -> usize {
        self.save
            .keys()
            .filter_map(|key| key.strip_prefix("SL_"))
            .filter_map(|index| index.parse::<usize>().ok())
            .map(|index| index + 1)
            .max()
            .unwrap_or(0)
    }

    /// `_loreEpiBon`: a Lore Episode's Villager EXP.
    fn lore_episode(&mut self, index: usize) -> f64 {
        let episodes = fields::at(self.save, "Spelunk", &[13, 2]).unwrap_or(0.0);
        if episodes <= index as f64 {
            return 0.0;
        }
        let (base, threshold) = data::lore_episode(index);
        let steps = ((self.tome_points().unwrap_or(0.0) - threshold).max(0.0) / 100.0).floor();
        if steps <= 0.0 {
            return 0.0;
        }
        let power = steps.powf(0.7);
        let decay = power / (25.0 + power);
        let grimoire = self.grimoire(17);
        let troll = if self.ola_text(379).contains("TROLL_SET") {
            self.data
                .equipment_sets
                .get("TROLL_SET")
                .copied()
                .unwrap_or(0.0)
        } else {
            0.0
        };
        (1.0 + (grimoire + troll) / 100.0) * base * decay
    }

    /// The six Villager EXP jar collectibles, each scaled by Legend 29.
    fn jar_collectibles(&self, legend: f64) -> f64 {
        let Some(jar) = crate::jar::Jar::from_save(self.save) else {
            return 0.0;
        };
        JAR_COLLECTIBLES
            .iter()
            .map(|index| jar.collectible_bonus(*index, legend))
            .sum()
    }

    /// `computeMonumentROGbonus(t, index)`: a monument's villager EXP.
    fn monument(&self, row: usize, index: usize) -> f64 {
        let slot = 10 * row + index;
        let level = fields::at(self.save, "Holes", &[15, slot]).unwrap_or(0.0);
        if level <= 0.0 {
            return 0.0;
        }
        let info = data::monument_slot(slot);
        if info <= 0.0 {
            return 0.0;
        }
        let mut holeoz = 1.0;
        if index != 9 {
            let cosmo = (data::cosmo_base(0, 0)
                * fields::at(self.save, "Holes", &[4, 0]).unwrap_or(0.0))
            .floor();
            holeoz = 1.0 + self.monument_dragon(row) / 100.0 + cosmo / 100.0;
        }
        holeoz = (holeoz * (1.0 + self.fountain(row, 13) / 100.0)).max(1.0);
        if info < 30.0 {
            level * info * holeoz
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * info * holeoz).ceil()
        }
    }

    /// `computeMonumentROGbonus(t, 9)`: the monument slot that boosts the rest.
    fn monument_dragon(&self, row: usize) -> f64 {
        let slot = 10 * row + 9;
        let level = fields::at(self.save, "Holes", &[15, slot]).unwrap_or(0.0);
        let info = data::monument_slot(slot);
        if level <= 0.0 || info <= 0.0 {
            return 0.0;
        }
        let holeoz = (1.0 + self.fountain(row, 13) / 100.0).max(1.0);
        if info < 30.0 {
            level * info * holeoz
        } else {
            0.1 * (level / (250.0 + level) * 10.0 * info * holeoz).ceil()
        }
    }
}
