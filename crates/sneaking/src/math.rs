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
    pub(crate) fn funeral_percent(&self, floor: usize) -> f64 {
        self.flowers.get(floor).copied().unwrap_or(0.0) * self.ninja_bonus(21)
    }

    /// The twin's stealth: `stealthValue` with the level base, the charm stealth
    /// factors, the account-wide stack, the floor allies and the flowers.
    pub fn stealth(&self, twin: usize) -> f64 {
        let Some(entry) = self.twins.get(twin) else {
            return 0.0;
        };
        let bonuses = self.bonuses(twin);
        let charm = (1.0 + bonuses.get(4) / 100.0)
            * (1.0 + bonuses.get(7) / 100.0)
            * (1.0 + bonuses.get(17) / 100.0);
        let multi = charm * (1.0 + self.ally_stealth(twin)) * self.account_multi(twin);
        stealth_value(
            f64::from(entry.level),
            &self.knowledge,
            &self.omni,
            multi,
            self.funeral_percent(entry.floor),
        )
    }

    /// The other twins on this twin's floor, with the charm totals they add.
    pub(crate) fn ally_contributions(&self, twin: usize) -> Vec<AllyContribution> {
        let Some(entry) = self.twins.get(twin) else {
            return Vec::new();
        };
        self.twins
            .iter()
            .take(self.players.min(self.twins.len()))
            .filter(|other| other.index != twin && other.floor == entry.floor)
            .map(|other| {
                let bonuses = self.bonuses(other.index);
                let type8 = bonuses.get(8);
                let type16 = bonuses.get(16);
                AllyContribution {
                    name: other.name.clone(),
                    total: type8 + type16,
                    type8,
                    type16,
                }
            })
            .collect()
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

/// One other twin's contribution to the ally stealth multiplier.
#[derive(Clone, Debug, PartialEq)]
pub struct AllyContribution {
    pub name: String,
    pub total: f64,
    pub type8: f64,
    pub type16: f64,
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
/// are account terms, so the caller's multiplier folds them in.
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
        // Gloves.
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
    use crate::data;

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
        let gloves = data::item("NjGl2").expect("the catalog carries NjGl2");
        assert_eq!(gloves.kind, 1);
        let expected = 10.0 * gloves.damage() * ((100.0 + 10.0) / (100.0 + 40.0));
        assert!((weapon_stat("NjGl2", 100.0) - expected).abs() < 1e-9);
    }
}
/// `floorJadeBase`: the Jade a floor pays before the account multipliers.
pub(crate) fn jade_base(floor: usize, mastery: u32) -> f64 {
    let data = data::data();
    let raw = data.info.jade_base.get(floor).copied().unwrap_or(0.0);
    if mastery == 0 {
        return raw;
    }
    0.1 * raw * data.scales.jade.powf(f64::from(mastery))
}

/// `floorExpMult`: the EXP a floor pays before the account multipliers.
pub(crate) fn exp_base(floor: usize, mastery: u32) -> f64 {
    let data = data::data();
    let raw = data.info.exp_base.get(floor).copied().unwrap_or(0.0);
    if mastery == 0 {
        return raw;
    }
    let scale = data.info.exp_base.get(11).copied().unwrap_or(1.0);
    0.1 * raw * scale.powf(f64::from(mastery))
}

/// `computeJadeActionsPerHour`'s result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct JadeRate {
    pub actions_per_hour: f64,
    pub detection_cycles: f64,
    pub hit_chance: f64,
    pub ko_chance: f64,
    pub dodge: f64,
}

/// `computeJadeActionsPerHour`.
pub(crate) fn jade_actions_per_hour(
    action_speed: f64,
    detection: f64,
    ko_time: f64,
    dodge_pct: f64,
) -> JadeRate {
    let dodge = (dodge_pct / 100.0).min(0.9);
    let hit_chance = (1.0 - detection).max(0.0);
    let ko_chance = detection * (1.0 - dodge);
    if detection >= 1.0 {
        return JadeRate {
            actions_per_hour: 0.0,
            detection_cycles: f64::INFINITY,
            hit_chance,
            ko_chance,
            dodge,
        };
    }
    let detection_cycles = (1.0 / (1.0 - detection) - 1.0) * (1.0 - dodge);
    JadeRate {
        actions_per_hour: 3600.0 * action_speed / (3600.0 + detection_cycles * (3600.0 + ko_time)),
        detection_cycles,
        hit_chance,
        ko_chance,
        dodge,
    }
}

/// `computeDoorHitsPerHour`'s result.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DoorRate {
    pub hits_per_hour: f64,
    pub attempts_per_hour: f64,
    pub hit_chance: f64,
    pub ko_chance: f64,
    pub dodge: f64,
    pub progress_per_attempt: f64,
    pub expected_ko_progress: f64,
    pub taunt_target: Option<usize>,
    pub taunting: bool,
    pub coupled: bool,
    pub flowers_per_hour: f64,
}

/// `computeDoorHitsPerHour`.
pub(crate) fn door_hits_per_hour(
    action_speed: f64,
    detection: f64,
    ko_time: f64,
    dodge_pct: f64,
) -> DoorRate {
    let detection = detection.clamp(0.0, 1.0);
    let dodge = (dodge_pct / 100.0).min(0.9);
    let hit_chance = 1.0 - detection;
    let ko_chance = detection * (1.0 - dodge);
    let progress_per_attempt = 3600.0 + ko_chance * ko_time;
    let attempts_per_hour = if progress_per_attempt > 0.0 {
        3600.0 * action_speed / progress_per_attempt
    } else {
        0.0
    };
    DoorRate {
        hits_per_hour: attempts_per_hour * hit_chance,
        attempts_per_hour,
        hit_chance,
        ko_chance,
        dodge,
        progress_per_attempt,
        expected_ko_progress: progress_per_attempt - 3600.0,
        ..DoorRate::default()
    }
}

/// One twin's input to the team door model.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TeamMember {
    pub action_speed: f64,
    pub detection: f64,
    pub ko_time: f64,
    pub dodge_pct: f64,
    pub taunting: bool,
    /// `NK 30`'s flower gain, `1 + bonus / 100`.
    pub flower_gain: f64,
}

/// `computeTeamDoorRates`: the Taunting Mark redirect loop's expected rates.
pub(crate) fn team_door_rates(members: &[TeamMember]) -> Vec<DoorRate> {
    let count = members.len();
    if count == 0 {
        return Vec::new();
    }
    let mut targets = vec![None; count];
    let mut ko_chance = vec![0.0; count];
    let mut matrix = vec![vec![0.0; count]; count];
    let mut values = vec![0.0; count];
    for (index, member) in members.iter().enumerate() {
        for (other, candidate) in members.iter().enumerate() {
            if other != index && candidate.taunting {
                targets[index] = Some(other);
                break;
            }
        }
        let detection = member.detection.clamp(0.0, 1.0);
        let dodge = (member.dodge_pct / 100.0).min(0.9);
        ko_chance[index] = detection * (1.0 - dodge);
        matrix[index][index] = 3600.0
            + match targets[index] {
                Some(_) => -ko_chance[index],
                None => ko_chance[index] * member.ko_time,
            };
        values[index] = 3600.0 * member.action_speed;
    }
    for (victim, target) in targets.iter().enumerate() {
        if let Some(taunter) = target {
            matrix[*taunter][victim] +=
                ko_chance[victim] * ko_chance[*taunter] * members[*taunter].ko_time;
        }
    }
    let mut attempts = solve_linear(matrix.clone(), values.clone());
    if attempts
        .as_ref()
        .is_none_or(|rows| rows.iter().any(|value| !value.is_finite() || *value < 0.0))
    {
        attempts = Some(
            members
                .iter()
                .map(|member| {
                    door_hits_per_hour(
                        member.action_speed,
                        member.detection,
                        member.ko_time,
                        member.dodge_pct,
                    )
                    .attempts_per_hour
                })
                .collect(),
        );
    }
    let attempts = attempts.unwrap_or_else(|| vec![0.0; count]);
    let mut rates: Vec<DoorRate> = members
        .iter()
        .enumerate()
        .map(|(index, member)| {
            let detection = member.detection.clamp(0.0, 1.0);
            let dodge = (member.dodge_pct / 100.0).min(0.9);
            let progress_per_attempt = if attempts[index] > 0.0 {
                3600.0 * member.action_speed / attempts[index]
            } else {
                0.0
            };
            DoorRate {
                attempts_per_hour: attempts[index],
                hits_per_hour: attempts[index] * (1.0 - detection),
                hit_chance: 1.0 - detection,
                ko_chance: ko_chance[index],
                dodge,
                progress_per_attempt,
                expected_ko_progress: progress_per_attempt - 3600.0,
                taunt_target: targets[index],
                taunting: member.taunting,
                coupled: targets[index].is_some() || targets.contains(&Some(index)),
                flowers_per_hour: 0.0,
            }
        })
        .collect();
    for (source, target) in targets.iter().enumerate() {
        match target {
            None => {
                rates[source].flowers_per_hour +=
                    attempts[source] * ko_chance[source] * members[source].flower_gain;
            }
            Some(target) => {
                rates[*target].flowers_per_hour += attempts[source]
                    * ko_chance[source]
                    * (members[source].flower_gain - 1.0 + ko_chance[*target]);
            }
        }
    }
    rates
}

/// A small Gaussian elimination for the team model's linear system.
#[allow(clippy::needless_range_loop)]
fn solve_linear(mut matrix: Vec<Vec<f64>>, values: Vec<f64>) -> Option<Vec<f64>> {
    let size = values.len();
    for (row, value) in matrix.iter_mut().zip(&values) {
        row.push(*value);
    }
    for column in 0..size {
        let mut pivot = column;
        for candidate in column + 1..size {
            if matrix[candidate][column].abs() > matrix[pivot][column].abs() {
                pivot = candidate;
            }
        }
        if matrix[pivot][column].abs() < 1e-12 {
            return None;
        }
        matrix.swap(column, pivot);
        let divisor = matrix[column][column];
        for cell in column..=size {
            matrix[column][cell] /= divisor;
        }
        for row in 0..size {
            if row == column {
                continue;
            }
            let factor = matrix[row][column];
            if factor == 0.0 {
                continue;
            }
            for cell in column..=size {
                matrix[row][cell] -= factor * matrix[column][cell];
            }
        }
    }
    Some(matrix.iter().map(|row| row[size]).collect())
}

/// `itemFindExpectedFromRate` without the probability-state batch: the rate
/// model the tree uses when a save has no accumulated actions.
pub(crate) fn item_find_per_hour(
    nk14: f64,
    equip_item_find: f64,
    successful_per_hour: f64,
    hours: f64,
    bubble_y9: f64,
    phase: f64,
    omni: &[f64],
) -> f64 {
    let batch_hours = hours.max(0.01);
    let expected = successful_per_hour.max(0.0) * batch_hours;
    let probability_zero = (-expected).exp();
    let fraction = match phase.round() as i64 {
        0 => 1.0 - probability_zero,
        1 => 0.4 * (1.0 - probability_zero),
        _ if expected <= 0.0 => 0.0,
        _ => expected / (expected + 50.0) - 50.0 * expected / (expected + 50.0).powi(3),
    };
    let ninja = ninja_bonus(&[], omni, 14) * 0.0 + nk14;
    let bonus = 1.0 + (ninja + equip_item_find) / 100.0;
    let y9 = 1.0 + (100.0 * (bubble_y9 - 1.0)).clamp(0.0, 100.0) / 100.0;
    3.0 * fraction.clamp(0.0, 1.0) * bonus * y9
}
