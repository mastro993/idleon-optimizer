//! Sushi Station formulas, ported from `js/stats/systems/w7/sushi.js`.
//!
//! `Station` holds the save's `Sushi` rows: what each slot cooks, the upgrade
//! levels, the fireplace column types and the knowledge levels. `External`
//! holds the account terms the currency and fuel formulas read. Every function
//! names its upstream counterpart so the fixture from `script/sushi-oracle.mjs`
//! can be compared term by term.

use crate::data::{GameData, game};
use idleon_common::curve;

/// The account terms the station's formulas read, all decoded from the save.
#[derive(Clone, Debug, PartialEq)]
pub struct External {
    /// Arcade upgrade 67.
    pub arcade67: f64,
    /// Grid square 188, the free shaker chance.
    pub grid188: f64,
    /// Grid square 189, the bucks multiplier.
    pub grid189: f64,
    /// Minehead floor 11's reward.
    pub minehead11: f64,
    /// Atom collider 14.
    pub atom14: f64,
    /// Sailing artifact 39.
    pub sailing39: f64,
    /// Bundles Received `ban_v`.
    pub bundle_v: bool,
    /// Gaming super bit 67 (0 or 1).
    pub super_bit67: f64,
    /// Button slot 2's bonus.
    pub button2: f64,
    /// Jelly Operator reward 18, the upgrade cost discount.
    pub jelly18: f64,
    /// Event shop item 66, the fuel capacity purchase (0 or 1).
    pub event_shop66: f64,
}

impl Default for External {
    fn default() -> Self {
        Self {
            arcade67: 0.0,
            grid188: 0.0,
            grid189: 0.0,
            minehead11: 0.0,
            atom14: 0.0,
            sailing39: 0.0,
            bundle_v: false,
            super_bit67: 0.0,
            button2: 0.0,
            jelly18: 0.0,
            event_shop66: 0.0,
        }
    }
}

/// The save's `Sushi` rows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Station {
    /// Row 0: the sushi tier cooking in each slot, `-1` when empty.
    pub tiers: Vec<f64>,
    /// Row 1: the slot type (0 normal, 1 hot, 2 cold, 3 milktoast).
    pub types: Vec<f64>,
    /// Row 2: the upgrade levels.
    pub levels: Vec<f64>,
    /// Row 3: the fireplace type of each of the fifteen columns.
    pub fires: Vec<f64>,
    /// Row 4: spa amount, sparks, bucks in bank, combo score.
    pub misc: Vec<f64>,
    /// Row 5: how far each sushi tier is discovered (0/1/2 with perfecto).
    pub found: Vec<f64>,
    /// Row 7: the knowledge level of each sushi tier.
    pub knowledge: Vec<f64>,
    /// `cachedUniqueSushi`.
    pub unique: u32,
    /// The Menagerie-doubled upgrade indices (`Spelunk[52]`).
    pub doublers: Vec<usize>,
}

impl Station {
    /// The level of one upgrade.
    pub fn level(&self, upg: usize) -> f64 {
        self.levels.get(upg).copied().unwrap_or(0.0)
    }

    /// `upgradeQTY(upgIdx)` at the saved levels.
    pub fn qty(&self, upg: usize) -> f64 {
        self.qty_at(upg, &self.levels)
    }

    /// `upgradeQTY(upgIdx)` at a candidate level row.
    pub fn qty_at(&self, upg: usize, levels: &[f64]) -> f64 {
        upgrade_qty(game(), upg, levels, &self.doublers)
    }

    /// `rogBonusQTY(idx)`, zero while the tier is locked.
    pub fn rog(&self, index: usize) -> f64 {
        rog_bonus_qty(game(), index, self.unique)
    }

    /// The sparks the station has collected (`Sushi[4][2]`).
    pub fn sparks(&self) -> f64 {
        self.misc.get(2).copied().unwrap_or(0.0)
    }

    /// The bucks in the bank (`Sushi[4][3]`).
    pub fn bucks(&self) -> f64 {
        self.misc.get(3).copied().unwrap_or(0.0)
    }

    /// The spa amount (`Sushi[4][1]`), which drives the overtuned multiplier.
    pub fn spa(&self) -> f64 {
        self.misc.get(1).copied().unwrap_or(0.0)
    }

    /// The persisted combo score (`Sushi[4][8]`).
    pub fn combo_score(&self) -> f64 {
        self.misc.get(8).copied().unwrap_or(0.0)
    }

    /// `knowledgeBonusTotals`: the eleven category totals.
    pub fn knowledge_totals(&self) -> Vec<f64> {
        knowledge_bonus_totals(game(), &self.knowledge, &self.found)
    }

    /// `knowledgeBonusSpecific(tier)`.
    pub fn knowledge_bonus(&self, tier: usize) -> f64 {
        knowledge_bonus_specific(game(), tier, &self.knowledge, &self.found)
    }

    /// `fireplaceEffectBase` at the saved sparks.
    pub fn fireplace_base(&self) -> f64 {
        fireplace_effect_base(&self.knowledge_totals(), self.sparks())
    }

    /// `computeOrangeFireSum`: the red fireplaces' fuel bonus.
    pub fn orange_fire_sum(&self) -> f64 {
        orange_fire_sum(&self.tiers, &self.fires, self.fireplace_base())
    }

    /// `fuelGenPerHr`.
    pub fn fuel_gen(&self, levels: &[f64], ext: &External) -> f64 {
        fuel_gen_per_hr(
            game(),
            levels,
            &self.knowledge,
            &self.found,
            &self.knowledge_totals(),
            self.orange_fire_sum(),
            ext.bundle_v,
            &self.doublers,
        )
    }

    /// `fuelCapacity`.
    pub fn fuel_capacity(&self, levels: &[f64], ext: &External) -> f64 {
        fuel_capacity(
            game(),
            levels,
            &self.knowledge_totals(),
            ext.bundle_v,
            ext.event_shop66 > 0.0,
            &self.doublers,
        )
    }

    /// `computeCurrencyMulti`.
    pub fn currency_multi(&self, levels: &[f64], ext: &External) -> f64 {
        let totals = self.knowledge_totals();
        let surcharge = self.qty_at(30, levels)
            + self.qty_at(31, levels)
            + self.qty_at(32, levels)
            + self.qty_at(33, levels)
            + self.qty_at(34, levels)
            + 100.0 * ext.super_bit67;
        (1.0 + ext.arcade67 / 100.0)
            * 1.1_f64.powi(self.unique as i32)
            * (1.0 + if ext.bundle_v { 1.0 } else { 0.0 })
            * (1.0 + surcharge / 100.0)
            * (1.0 + totals.first().copied().unwrap_or(0.0) / 100.0)
            * (1.0 + ext.button2 / 100.0)
            * (1.0 + ext.grid189 / 100.0)
            * (1.0 + self.qty_at(40, levels) / 100.0)
            * (1.0 + ext.minehead11 / 100.0).clamp(1.0, 1.25)
            * (1.0 + (self.qty_at(41, levels) + self.qty_at(43, levels)) / 100.0)
            * (1.0 + compute_overtuned_multi(self.spa()) / 100.0)
            * (1.0 + ext.atom14 / 100.0)
            * (1.0 + ext.sailing39)
            * combo_multiplier(self.combo_score())
    }

    /// `totalBucksPerHr`.
    pub fn bucks_per_hour(&self, levels: &[f64], ext: &External) -> f64 {
        let multi = self.currency_multi(levels, ext);
        (0..game().max_slots)
            .map(|slot| self.currency_per_slot(slot, multi))
            .sum()
    }

    /// `currencyPerSlot`.
    pub fn currency_per_slot(&self, slot: usize, multi: f64) -> f64 {
        currency_per_slot(
            game(),
            slot,
            &self.tiers,
            &self.types,
            &self.fires,
            self.sparks(),
            multi,
            &self.knowledge_totals(),
        )
    }

    /// `upgCost(slot)`.
    pub fn upgrade_cost(&self, slot: usize, levels: &[f64], ext: &External) -> f64 {
        upg_cost(
            game(),
            slot,
            levels,
            &self.knowledge_totals(),
            self.unique,
            ext.jelly18,
            &self.doublers,
        )
    }

    /// `slotsOwned`: the Menagerie doublers do not apply.
    pub fn slots_owned(&self) -> f64 {
        slots_owned(upgrade_qty(game(), 0, &self.levels, &[]))
    }

    /// `countActiveSlots`.
    pub fn active_slots(&self) -> usize {
        count_active_slots(&self.tiers)
    }

    /// `maxCookTier`: the Menagerie doublers do not apply.
    pub fn max_cook_tier(&self) -> f64 {
        max_cook_tier(upgrade_qty(game(), 6, &self.levels, &[]))
    }

    /// `bonusCookTierPct`: the Menagerie doublers do not apply.
    pub fn bonus_cook_tier_pct(&self) -> f64 {
        upgrade_qty(game(), 7, &self.levels, &[])
            + self.knowledge_totals().get(2).copied().unwrap_or(0.0)
    }

    /// `freeShakerChance`.
    pub fn free_shaker_chance(&self, ext: &External) -> f64 {
        free_shaker_chance(
            self.qty(21),
            self.knowledge_totals().get(5).copied().unwrap_or(0.0),
            ext.grid188,
        )
    }

    /// `saffronHrs`.
    pub fn saffron_hrs(&self) -> f64 {
        saffron_hrs(self.qty(22))
    }

    /// The category bonus one more knowledge level in `tier` would pay
    /// (`knNext` in `_renderSushi`), which ignores the current level.
    pub fn knowledge_bonus_next(&self, tier: usize) -> f64 {
        knowledge_bonus_next(game(), tier, &self.found)
    }

    /// `perfectoOdds(tier)`.
    pub fn perfecto_odds(&self, tier: usize) -> f64 {
        perfecto_odds(tier, &self.knowledge_totals())
    }

    /// `knowledgeXPBase`.
    pub fn knowledge_xp_base(&self) -> f64 {
        knowledge_xp_base(self.qty(37))
    }

    /// `knowledgeXPMulti(slot)`.
    pub fn knowledge_xp_multi(&self, slot: usize) -> f64 {
        knowledge_xp_multi(
            slot,
            &self.fires,
            self.qty(38),
            self.fireplace_base(),
            &self.knowledge_totals(),
        )
    }

    /// `isPerfecto(tier)`: the discovery row's positive value.
    pub fn is_perfecto(&self, tier: usize) -> bool {
        self.found.get(tier).copied().unwrap_or(0.0) > 0.0
    }

    /// `discovered(tier)`: a tier below `cachedUniqueSushi`.
    pub fn discovered(&self, tier: usize) -> bool {
        tier < self.unique as usize
    }
}

/// `rogBonusQTY(idx, uniqueSushi)`.
pub fn rog_bonus_qty(data: &GameData, index: usize, unique_sushi: u32) -> f64 {
    if unique_sushi as usize > index {
        data.rog_qty_at(index)
    } else {
        0.0
    }
}

/// `buildRogArray`.
pub fn build_rog_array(data: &GameData, unique_sushi: u32) -> Vec<f64> {
    (0..data.rog_qty.len())
        .map(|index| rog_bonus_qty(data, index, unique_sushi))
        .collect()
}

/// `computeUniqueSushi`: the run of non-negative discovery tiers.
pub fn compute_unique_sushi(found: &[f64]) -> u32 {
    let mut count = 0;
    for (index, tier) in found.iter().enumerate() {
        if *tier >= 0.0 {
            count = index as u32 + 1;
        } else {
            break;
        }
    }
    count
}

/// The bucks one sushi tier earns before the currency multiplier.
pub fn currency_per_tier(data: &GameData, tier: usize) -> f64 {
    if tier < 10 {
        return data.currency_per_tier.get(tier).copied().unwrap_or(0.0);
    }
    let t = tier as f64;
    if tier < 16 {
        (2.46 - tier as f64 / 100.0).powf(t) + 5.0 * t + t.powi(2)
    } else {
        2.31_f64.powf(t)
    }
}

/// The fuel one cook of a tier costs.
pub fn fuel_cost_per_tier(tier: usize) -> f64 {
    if tier == 5 {
        return 176.0;
    }
    let t = tier as f64;
    10.0 * 1.83_f64.powf(t) - t.powi(2)
}

/// `upgLvREQ(t)`: the research level a slot unlocks at.
pub fn upg_lv_req(t: usize) -> f64 {
    let t = t as f64;
    (1.0 + t.min(2.0)
        + t.min(4.0)
        + (3.0 * t - (t - 4.0).max(0.0) - (t - 8.0).max(0.0)
            + (t / 6.0).floor()
            + (t / 17.0).floor()))
    .floor()
}

/// `sushiDoublerMulti`: `2` when the Menagerie doubles this upgrade.
pub fn doubler_multi(doublers: &[usize], upg_idx: usize) -> f64 {
    if doublers.contains(&upg_idx) {
        2.0
    } else {
        1.0
    }
}

/// `upgradeQTY(upgIdx, upgLevels, doublers)`.
pub fn upgrade_qty(data: &GameData, upg_idx: usize, levels: &[f64], doublers: &[usize]) -> f64 {
    let level = levels.get(upg_idx).copied().unwrap_or(0.0);
    let bonus = data
        .upgrade(upg_idx)
        .map(|upgrade| upgrade.bonus_per_lv)
        .unwrap_or(0.0);
    doubler_multi(doublers, upg_idx) * bonus * level
}

/// `upgCost(slot, upgLevels, knowledgeTotals, saveData)`.
pub fn upg_cost(
    data: &GameData,
    slot: usize,
    levels: &[f64],
    knowledge_totals: &[f64],
    unique_sushi: u32,
    jelly18: f64,
    doublers: &[usize],
) -> f64 {
    let Some(upg_idx) = data.slot_upg(slot) else {
        return f64::INFINITY;
    };
    let Some(upgrade) = data.upgrade(upg_idx) else {
        return f64::INFINITY;
    };
    let cost_dn = if upgrade.cost_dn != 0.0 {
        upgrade.cost_dn.max(0.1)
    } else {
        1.0
    };
    let t = slot as f64;
    let wholesale = 1.0 / (1.0 + upgrade_qty(data, 36, levels, doublers) / 100.0);
    let cheaper_by_rog = (1.0
        - rog_bonus_qty(data, 26, unique_sushi).max(rog_bonus_qty(data, 44, unique_sushi)) / 100.0)
        .max(0.1);
    let knowledge_cheaper = 1.0 / (1.0 + knowledge_totals.get(6).copied().unwrap_or(0.0) / 100.0);
    let jelly_cheaper = 1.0 / (1.0 + jelly18 / 100.0);
    let current = levels.get(upg_idx).copied().unwrap_or(0.0);
    cost_dn
        * (5.0 + t + (t - 1.0).max(0.0).powi(2))
        * (1.5 + (t - 3.0).max(0.0) / 16.0).powf((t - 4.0).max(0.0))
        * 1.3_f64.powf((t - 20.0).max(0.0))
        * wholesale
        * cheaper_by_rog
        * knowledge_cheaper
        * jelly_cheaper
        * upgrade.cost_base.powf(current)
}

/// `knowledgeBonusSpecific(tier)`.
pub fn knowledge_bonus_specific(
    data: &GameData,
    tier: usize,
    knowledge: &[f64],
    found: &[f64],
) -> f64 {
    let Some(cat) = data.knowledge_cat(tier) else {
        return 0.0;
    };
    let base = data.knowledge_value(cat);
    let level = knowledge.get(tier).copied().unwrap_or(0.0);
    let discovery = (1.0 + found.get(tier).copied().unwrap_or(0.0)).min(2.0);
    (base * level * discovery * (1.0 + tier as f64 / 30.0)).max(0.0)
}

/// `knNext` from `_renderSushi`: the bonus one more knowledge level pays.
pub fn knowledge_bonus_next(data: &GameData, tier: usize, found: &[f64]) -> f64 {
    let Some(cat) = data.knowledge_cat(tier) else {
        return 0.0;
    };
    let discovery = (1.0 + found.get(tier).copied().unwrap_or(0.0)).min(2.0);
    data.knowledge_value(cat) * discovery * (1.0 + tier as f64 / 30.0)
}

/// `knowledgeBonusTotals`.
pub fn knowledge_bonus_totals(data: &GameData, knowledge: &[f64], found: &[f64]) -> Vec<f64> {
    let mut totals = vec![0.0; data.knowledge_cat_desc.len()];
    for tier in 0..=data.max_tier() {
        if let Some(cat) = data.knowledge_cat(tier)
            && let Some(total) = totals.get_mut(cat)
        {
            *total += knowledge_bonus_specific(data, tier, knowledge, found);
        }
    }
    totals
}

/// `fuelGenPerHr`.
#[allow(clippy::too_many_arguments)]
pub fn fuel_gen_per_hr(
    data: &GameData,
    levels: &[f64],
    knowledge: &[f64],
    found: &[f64],
    knowledge_totals: &[f64],
    orange_fire_sum: f64,
    bundle_v: bool,
    doublers: &[usize],
) -> f64 {
    let bundle = 1.0 + if bundle_v { 1.0 } else { 0.0 };
    let upgrade_fuel = (8..=12)
        .map(|upg| upgrade_qty(data, upg, levels, doublers))
        .sum::<f64>();
    let knowledge_fuel = knowledge_totals.get(4).copied().unwrap_or(0.0);
    let spec = |tier: usize| knowledge_bonus_specific(data, tier, knowledge, found);
    let raw = |upg: usize| levels.get(upg).copied().unwrap_or(0.0);
    let unique = (1.0 + raw(9) / 100.0)
        * (1.0 + raw(10) / 100.0)
        * (1.0 + raw(11) / 100.0)
        * (1.0 + raw(12) / 100.0);
    50.0 * bundle
        * (1.0 + upgrade_fuel / 100.0)
        * (1.0 + orange_fire_sum / 100.0)
        * (1.0 + knowledge_fuel / 100.0)
        * (1.0 + spec(27) / 100.0)
        * (1.0 + spec(36) / 100.0)
        * (1.0 + spec(45) / 100.0)
        * unique
}

/// `fuelCapacity`.
pub fn fuel_capacity(
    data: &GameData,
    levels: &[f64],
    knowledge_totals: &[f64],
    bundle_v: bool,
    fuel_cap_purchased: bool,
    doublers: &[usize],
) -> f64 {
    let bundle = 1.0 + if bundle_v { 1.0 } else { 0.0 };
    let upgrade_cap = (1..=5)
        .map(|upg| upgrade_qty(data, upg, levels, doublers))
        .sum::<f64>();
    let knowledge_cap = knowledge_totals.get(3).copied().unwrap_or(0.0);
    let raw = |upg: usize| levels.get(upg).copied().unwrap_or(0.0);
    let unique = (1.0 + raw(2) / 100.0)
        * (1.0 + raw(3) / 100.0)
        * (1.0 + raw(4) / 100.0)
        * (1.0 + raw(5) / 100.0);
    (200.0 + knowledge_cap)
        * bundle
        * (1.0 + if fuel_cap_purchased { 9.0 } else { 0.0 })
        * (1.0 + upgrade_cap / 100.0)
        * unique
}

/// `computeOrangeFireSum`: each active slot on a red fireplace column.
pub fn orange_fire_sum(tiers: &[f64], fires: &[f64], fireplace_base: f64) -> f64 {
    let mut sum = 0.0;
    for slot in 0..game().max_slots {
        let tier = tiers.get(slot).copied().unwrap_or(f64::NAN);
        let fire = fires.get(slot % 15).copied().unwrap_or(f64::NAN);
        if tier >= 0.0 && fire == 0.0 {
            sum += (tier + 1.0) * fireplace_base;
        }
    }
    sum
}

/// `fireplaceEffectBase(knowledgeTotals, sparks)`.
pub fn fireplace_effect_base(knowledge_totals: &[f64], sparks: f64) -> f64 {
    let spark_multi = if sparks > 0.0 {
        0.2 * (sparks.ln() / std::f64::consts::LN_2) + curve::log(sparks)
    } else {
        0.0
    };
    (1.0 + knowledge_totals.get(9).copied().unwrap_or(0.0) / 100.0) * (1.0 + spark_multi / 100.0)
}

/// `slotEffectBase`.
pub fn slot_effect_base(knowledge_totals: &[f64]) -> f64 {
    1.0 + knowledge_totals.get(8).copied().unwrap_or(0.0) / 100.0
}

/// `fireplaceEffectByType`.
pub fn fireplace_effect_by_type(fire_type: usize, base: f64) -> f64 {
    match fire_type {
        0 => base,
        1 => 150.0 * base,
        2 => 20.0 * base,
        3 => 100.0 * base,
        _ => 0.0,
    }
}

/// `currencyPerSlot`.
#[allow(clippy::too_many_arguments)]
pub fn currency_per_slot(
    data: &GameData,
    slot: usize,
    tiers: &[f64],
    types: &[f64],
    fires: &[f64],
    sparks: f64,
    currency_multi: f64,
    knowledge_totals: &[f64],
) -> f64 {
    let tier = tiers.get(slot).copied().unwrap_or(f64::NAN);
    if tier < 0.0 || tier.is_nan() {
        return 0.0;
    }
    let mut slot_dn = 1.0;
    if types.get(slot).copied().unwrap_or(0.0) == 1.0 {
        slot_dn *= 1.0 + 50.0 * slot_effect_base(knowledge_totals) / 100.0;
    }
    let fire_type = fires.get(slot % 15).copied().unwrap_or(0.0);
    if fire_type == 1.0 {
        slot_dn *= 1.0 + 150.0 * fireplace_effect_base(knowledge_totals, sparks) / 100.0;
    }
    slot_dn * currency_multi * currency_per_tier(data, tier as usize)
}

/// `computeComboMultiplier`.
pub fn combo_multiplier(score: f64) -> f64 {
    let safe = if score.is_finite() {
        score.max(0.0)
    } else {
        0.0
    };
    let excess = (safe - 1500.0).max(0.0);
    1.0 + safe.powf(0.3).min(10.0) + (excess / (20000.0 + excess)) * 90.0
}

/// `comboMeterUnlocked`.
pub fn combo_meter_unlocked(qty39: f64) -> bool {
    qty39 > 0.0
}

/// `computeOvertunedMulti`.
pub fn compute_overtuned_multi(spa: f64) -> f64 {
    if spa <= 0.0 {
        return 0.0;
    }
    let x = (spa / 1e6).max(1.0);
    5.0 * (x.ln() / std::f64::consts::LN_2) + 10.0 * curve::log(x)
}

/// `slotsOwned`.
pub fn slots_owned(qty_slots: f64) -> f64 {
    (10.0 + qty_slots).min(game().max_slots as f64)
}

/// `countActiveSlots`.
pub fn count_active_slots(tiers: &[f64]) -> usize {
    (0..game().max_slots)
        .filter(|slot| tiers.get(*slot).copied().unwrap_or(0.0) >= 0.0)
        .count()
}

/// `maxCookTier`.
pub fn max_cook_tier(qty6: f64) -> f64 {
    qty6.round()
}

/// `freeShakerChance`.
pub fn free_shaker_chance(qty21: f64, knowledge_shakers: f64, grid188: f64) -> f64 {
    ((qty21 + knowledge_shakers + grid188) / 100.0).min(0.6)
}

/// `saffronHrs`.
pub fn saffron_hrs(qty22: f64) -> f64 {
    (1.0 + qty22).round()
}

/// `perfectoOdds`.
pub fn perfecto_odds(tier: usize, knowledge_totals: &[f64]) -> f64 {
    let t = tier as f64;
    0.6 * 0.81_f64.powf(t) / (1.0 + t / 8.0)
        * (1.0 + knowledge_totals.get(10).copied().unwrap_or(0.0) / 100.0)
}

/// `knowledgeXPReq`.
pub fn knowledge_xp_req(level: f64) -> f64 {
    (3.0 + level + level.powf(1.5)) * 1.5_f64.powf((level - 2.0).max(0.0))
}

/// `knowledgeXPBase`.
pub fn knowledge_xp_base(qty37: f64) -> f64 {
    1.0 + qty37 / 10.0
}

/// `knowledgeXPMulti`.
pub fn knowledge_xp_multi(
    slot: usize,
    fires: &[f64],
    qty38: f64,
    fireplace_base: f64,
    knowledge_totals: &[f64],
) -> f64 {
    let fire_type = fires.get(slot % 15).copied().unwrap_or(0.0);
    let pink = if fire_type == 3.0 {
        100.0 * fireplace_base
    } else {
        0.0
    };
    (1.0 + qty38 / 100.0)
        * (1.0 + pink / 100.0)
        * (1.0 + knowledge_totals.get(1).copied().unwrap_or(0.0) / 100.0)
}
