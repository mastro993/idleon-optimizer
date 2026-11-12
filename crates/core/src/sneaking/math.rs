//! The loadout's formulas: the charm bonus map, the Gold inventory scan, the
//! action speed, the KO recovery, the stealth factors and the detection chance.
//!
//! Ported from `js/stats/systems/w6/sneaking-math.js` (`nkBonus`, `charmStat`,
//! `weaponItemStat`, `actionSpeed`, `koDuration`, `detectionChance`,
//! `floorDetectionBase`, `stealthValue`, `stealthForDetection`) and the loadout
//! half of `js/stats/systems/w6/sneaking-stealth.js` (`twinCharmBonuses`,
//! `goldInventoryBonusDetails`, `computeActionSpeed`, `computeTwinStealth`,
//! `_gemstoneBonus`, `_gloveSPD`).
//!
//! Every value is recomputed from the slots, so a view that reads a twin after
//! an edit never shows a stale figure.

use super::data;
use super::{Bonuses, GoldBonuses, GoldPick, INVENTORY_SCAN, Loadout, SlotKind};

impl Loadout {
    // ----- Ninja Knowledge -----

    /// `NLbonuses(idx)`: `NinjaUpg[idx][5]` times the saved level, plus the
    /// Omni-Jade levels for the first seventeen upgrades.
    pub fn ninja_bonus(&self, index: usize) -> f64 {
        ninja_bonus(&self.knowledge, &self.omni, index)
    }

    // ----- Items -----

    /// `SneakSymbolBon(slotLVID)` = `50 * Spelunk[14][lvid]`.
    fn symbol_bonus(&self, lvid: usize) -> f64 {
        50.0 * self.symbols.get(lvid).copied().unwrap_or(0.0)
    }

    /// The symbol level of a twin's charm slot.
    pub fn slot_symbol(&self, twin: usize, charm: usize) -> f64 {
        self.symbols.get(2 * twin + charm).copied().unwrap_or(0.0)
    }

    /// The symbol level of an inventory row.
    pub fn inventory_symbol(&self, row: usize) -> f64 {
        self.symbols.get(24 + row).copied().unwrap_or(0.0)
    }

    /// The `LegendTalents` points a legend talent holds.
    pub fn legend_points(&self, index: usize) -> f64 {
        let level = self.legend_points.get(index).copied().unwrap_or(0.0);
        (level * data::legend_per_point(index)).round()
    }

    /// `GemstoneBonus(idx)`: the NjGem value at its saved level, scaled by the
    /// first gem and the all-talent multiplier.
    pub fn gemstone(&self, index: usize) -> f64 {
        let key = format!("NjGem{index}");
        let Some(def) = data::item(&key) else {
            return 0.0;
        };
        let level = self.gem_levels.get(index).copied().unwrap_or(0.0);
        if level <= 0.5 {
            return 0.0;
        }
        let base: f64 = def.stat.trim().parse().unwrap_or(0.0);
        let value = base + def.max * (level / (1000.0 + level));
        if index == 5 {
            return value;
        }
        let first = self.gemstone(5);
        value * (1.0 + first / 100.0) * self.account.talent_432.max(1.0)
    }

    /// `goldInventoryBonusDetails`: the best Gold charm per bonus type, with
    /// the gemstone, legend-point and symbol multipliers.
    pub fn gold_bonuses(&self) -> GoldBonuses {
        let gemstone = self.gemstone(3);
        let legend = self.legend_points(6);
        let mut bonuses = GoldBonuses::default();
        for row in 0..INVENTORY_SCAN {
            let Some(item) = self.inventory.get(row) else {
                continue;
            };
            if item.is_blank() {
                continue;
            }
            let Some(def) = item.def() else {
                continue;
            };
            if !def.is_gold() {
                continue;
            }
            let Ok(bonus_type) = usize::try_from(def.bonus_type) else {
                continue;
            };
            let Some(current) = bonuses.values.get(bonus_type).copied() else {
                continue;
            };
            let raw = charm_stat(&item.key, f64::from(item.level));
            if raw <= current {
                continue;
            }
            let symbol = self.symbol_bonus(24 + row);
            bonuses.values[bonus_type] =
                raw * (1.0 + gemstone / 100.0) * (1.0 + legend / 100.0) * (1.0 + symbol / 100.0);
            bonuses.selected[bonus_type] = Some(GoldPick {
                row,
                key: item.key.clone(),
                raw,
            });
        }
        bonuses
    }

    /// `twinCharmBonuses`: the equipped charms' totals, each scaled by the Gold
    /// charm-power bonus and its slot symbol.
    pub fn bonuses(&self, twin: usize) -> Bonuses {
        let mut bonuses = Bonuses::default();
        let Some(entry) = self.twins.get(twin) else {
            return bonuses;
        };
        let charm_power = self.gold_bonuses().get(12);
        for charm in 0..2 {
            let item = entry.slot(SlotKind::Charm(charm));
            if item.is_blank() {
                continue;
            }
            let Some(def) = item.def() else {
                continue;
            };
            if def.kind != 2 {
                continue;
            }
            let symbol = self.symbol_bonus(2 * twin + charm);
            let stat = charm_stat(&item.key, f64::from(item.level))
                * (1.0 + charm_power / 100.0)
                * (1.0 + symbol / 100.0);
            if let Ok(bonus_type) = usize::try_from(def.bonus_type)
                && let Some(value) = bonuses.values.get_mut(bonus_type)
            {
                *value += stat;
            }
        }
        bonuses
    }

    /// `gloveSPD`: a Gloves weapon's action-speed stat, zero for other weapons.
    fn glove_speed(&self, twin: usize) -> f64 {
        let Some(entry) = self.twins.get(twin) else {
            return 0.0;
        };
        let item = entry.slot(SlotKind::Weapon);
        match item.def() {
            Some(def) if def.kind == 1 && def.bonus_type == 0 => {
                weapon_stat(&item.key, f64::from(item.level))
            }
            _ => 0.0,
        }
    }

    // ----- Formulas -----

    /// `detectionChance`'s base for a floor at the saved mastery.
    pub fn detection_base(&self, floor: usize) -> f64 {
        detection_base(floor, self.mastery)
    }

    /// `funeralStealthBonus`: the flowers on a floor times Ninja Knowledge 21.
    pub fn funeral_percent(&self, floor: usize) -> f64 {
        self.flowers.get(floor).copied().unwrap_or(0.0) * self.ninja_bonus(21)
    }

    /// The loadout's stealth: `stealthValue` with the loadout's own multiplier —
    /// the level base, the charm stealth factors, the floor allies and the
    /// flowers. The account-wide stack belongs to the breakdowns.
    pub fn stealth(&self, twin: usize) -> f64 {
        let Some(entry) = self.twins.get(twin) else {
            return 0.0;
        };
        let bonuses = self.bonuses(twin);
        let charm = (1.0 + bonuses.get(4) / 100.0)
            * (1.0 + bonuses.get(7) / 100.0)
            * (1.0 + bonuses.get(17) / 100.0);
        let multi = charm * (1.0 + self.ally_stealth(twin));
        stealth_value(
            f64::from(entry.level),
            &self.knowledge,
            &self.omni,
            multi,
            self.funeral_percent(entry.floor),
        )
    }

    /// The other twins' floor-stealth charm totals on this twin's floor:
    /// the ally bonus `computeTwinStealth` adds.
    fn ally_stealth(&self, twin: usize) -> f64 {
        let Some(entry) = self.twins.get(twin) else {
            return 0.0;
        };
        self.twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .filter(|other| other.index != twin && other.floor == entry.floor)
            .map(|other| {
                let bonuses = self.bonuses(other.index);
                (bonuses.get(8) + bonuses.get(16)) / 100.0
            })
            .sum()
    }

    /// `computeActionSpeed`: Ninja Knowledge 4, the charm speed bonus (doubled
    /// for a solo twin), the Gloves stat and the Gold inventory bonus.
    pub fn action_speed(&self, twin: usize) -> f64 {
        let mut speed = self.bonuses(twin).get(2);
        if self.is_solo(twin) {
            speed *= 2.0;
        }
        let equip = speed + self.glove_speed(twin) + self.gold_bonuses().get(24);
        action_speed(&self.knowledge, &self.omni, equip)
    }

    /// Seconds between attempts at the current action speed.
    pub fn attempt_seconds(&self, twin: usize) -> f64 {
        let speed = self.action_speed(twin);
        if speed <= 0.0 {
            return f64::INFINITY;
        }
        3600.0 / speed
    }

    /// `koDuration`: the recovery before action speed, in seconds.
    pub fn ko_seconds(&self, twin: usize) -> f64 {
        ko_duration(&self.knowledge, &self.omni, self.bonuses(twin).get(1))
    }

    /// The detection chance at the twin's saved floor.
    pub fn detection(&self, twin: usize) -> f64 {
        let Some(entry) = self.twins.get(twin) else {
            return 0.0;
        };
        detection_chance(self.stealth(twin), self.detection_base(entry.floor))
    }

    /// The stealth a target detection chance needs on a floor.
    pub fn stealth_for_detection(&self, floor: usize, target: f64) -> f64 {
        stealth_for_detection(floor, self.mastery, target)
    }

    /// The fastest twin and the hours until its first 100-action batch.
    pub fn first_batch(&self) -> Option<(String, f64)> {
        let fastest = self
            .twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .max_by(|left, right| {
                self.action_speed(left.index)
                    .total_cmp(&self.action_speed(right.index))
            })?;
        let speed = self.action_speed(fastest.index);
        (speed > 0.0).then(|| (fastest.name.clone(), 100.0 / speed))
    }
}

/// `nkBonus`: `NinjaUpg[idx][5]` times the level, plus the Omni-Jade levels for
/// the first seventeen upgrades. Index 99 is the game's constant one.
pub fn ninja_bonus(knowledge: &[f64], omni: &[f64], index: usize) -> f64 {
    if index == 99 {
        return 1.0;
    }
    let level = knowledge.get(index).copied().unwrap_or(0.0);
    let level = if index < 17 {
        level + omni.get(index).copied().unwrap_or(0.0)
    } else {
        level
    };
    data::upgrade(index).map_or(0.0, |upgrade| upgrade.per_level) * level
}

/// `actionSpeed`: `1 + (NLbonuses(4) + equipSpd) / 100`.
pub fn action_speed(knowledge: &[f64], omni: &[f64], equip_speed: f64) -> f64 {
    1.0 + (ninja_bonus(knowledge, omni, 4) + equip_speed) / 100.0
}

/// `koDuration`: `7200 / (1 + (NLbonuses(15) + equipKO) / 100)`.
pub fn ko_duration(knowledge: &[f64], omni: &[f64], equip_ko: f64) -> f64 {
    7200.0 / (1.0 + (ninja_bonus(knowledge, omni, 15) + equip_ko) / 100.0)
}

/// `floorDetectionBase`: the detection base a floor starts with at a mastery.
pub fn detection_base(floor: usize, mastery: u32) -> f64 {
    let raw = data::data()
        .info
        .detection_base
        .get(floor)
        .copied()
        .unwrap_or(0.0);
    if mastery == 0 {
        return raw;
    }
    if floor == 0 {
        return 0.01;
    }
    0.1 * raw * data::data().scales.detection.powf(f64::from(mastery))
}

/// `stealthForDetection`: the stealth a target chance needs on a floor.
pub fn stealth_for_detection(floor: usize, mastery: u32, target: f64) -> f64 {
    let base = detection_base(floor, mastery);
    if base <= 0.0 {
        return 0.0;
    }
    let target = target.clamp(0.0, 1.0);
    (1.0 - target) * base / (0.1 + target)
}

/// `stealthValue`: `(10 + NLbonuses(13) * sneakLv) * stealthMulti
/// * (1 + funeralPct / 100) * (1 + NLbonuses(23) / 100)`.
///
/// Upstream's extras also carry the fractal, emperor and sushi percentages; they
/// are account terms, so this port's caller supplies the multiplier instead.
pub fn stealth_value(
    sneak_level: f64,
    knowledge: &[f64],
    omni: &[f64],
    mult: f64,
    funeral_pct: f64,
) -> f64 {
    let base = 10.0 + ninja_bonus(knowledge, omni, 13) * sneak_level;
    let shh = 1.0 + ninja_bonus(knowledge, omni, 23) / 100.0;
    base * mult * (1.0 + funeral_pct / 100.0) * shh
}

/// `charmStat`: the charm's value at a level.
pub fn charm_stat(key: &str, level: f64) -> f64 {
    let Some(def) = data::item(key) else {
        return 0.0;
    };
    if def.kind != 2 {
        return 0.0;
    }
    let base: f64 = def.stat.trim().parse().unwrap_or(0.0);
    let max = def.max;
    let first = (base + max * (level / (level + 50.0))).min(max);
    let second = (max * (level / (level + 900.0))).min(max);
    first + second
}

/// `weaponItemStat`: the weapon's damage stat at a level.
pub fn weapon_stat(key: &str, level: f64) -> f64 {
    let Some(def) = data::item(key) else {
        return 0.0;
    };
    if def.kind != 1 {
        return 0.0;
    }
    let base = def.damage();
    if def.bonus_type == 0 {
        // Gloves: 10 * base * (lv + 10) / (lv + 40)
        return 10.0 * base * ((level + 10.0) / (level + 40.0));
    }
    if level <= 110.0 {
        return base
            * 1.23_f64.powf(level)
            * 0.92_f64.powf((level - 80.0).max(0.0))
            * 0.94_f64.powf((level - 110.0).max(0.0));
    }
    base * 1.23_f64.powi(110) * 0.92_f64.powi(30) * 1.063_704_f64.powf(level - 110.0)
}

/// `detectionChance`: `1 - 1.1 * stealth / (stealth + base)`, clamped.
pub fn detection_chance(stealth: f64, base: f64) -> f64 {
    if base <= 0.0 {
        return 0.0;
    }
    (1.0 - 1.1 * stealth / (stealth + base)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sneaking::data;

    #[test]
    fn detection_matches_the_upstream_clamp() {
        assert_eq!(detection_chance(0.0, 0.0), 0.0);
        assert_eq!(detection_chance(1000.0, 0.0), 0.0);
        assert!((detection_chance(0.0, 500.0) - 1.0).abs() < 1e-12);
        assert!((detection_chance(500.0, 500.0) - 0.45).abs() < 1e-12);
        assert_eq!(detection_chance(5000.0, 500.0), 0.0);
    }

    #[test]
    fn a_weapon_stat_follows_the_upstream_curves() {
        // Gloves: 10 * base * (lv + 10) / (lv + 40).
        let gloves = data::item("NjGl2").expect("the catalog carries NjGl2");
        assert_eq!(gloves.kind, 1);
        let expected = 10.0 * gloves.damage() * ((100.0 + 10.0) / (100.0 + 40.0));
        assert!((weapon_stat("NjGl2", 100.0) - expected).abs() < 1e-9);
    }
}
