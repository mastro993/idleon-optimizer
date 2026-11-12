//! The Depth Charge formulas, ported from `js/stats/systems/w7/minehead.js`.
//!
//! Everything here is pure: it takes the save-derived numbers and returns the
//! value the game shows. The upstream functions that read `saveData` take the
//! decoded scalars instead, so the tests can pin them against the upstream
//! oracle without building a save.

use crate::data::{GameData, game};
use idleon_common::curve::log;

/// Minimum research level that unlocks upgrade `t`.
pub fn upg_lv_req(t: usize) -> u32 {
    1 + 3 * t as u32 + t as u32 / 3 + t as u32 / 11
}

/// `UpgradeQTY`: the bonus one upgrade pays at `level`, doubled by Menagerie.
pub fn upgrade_qty(data: &GameData, t: usize, level: f64, doubled: f64) -> f64 {
    let bonus = data.upgrade(t).map(|upgrade| upgrade.bonus).unwrap_or(0.0);
    doubled * bonus * level
}

/// The multiplier a Menagerie doubler applies to upgrade `t`.
pub fn doubler_multi(doublers: &[usize], t: usize) -> f64 {
    if doublers.contains(&t) { 2.0 } else { 1.0 }
}

/// The QTY-equivalent levels: a doubled upgrade's level counts twice.
pub fn qty_levels(levels: &[f64], doublers: &[usize]) -> Vec<f64> {
    let mut levels = levels.to_vec();
    for (index, level) in levels.iter_mut().enumerate() {
        if doublers.contains(&index) {
            *level *= 2.0;
        }
    }
    levels
}

/// What one upgrade costs at its current level.
pub fn upg_cost(
    data: &GameData,
    t: usize,
    level: f64,
    qty26: f64,
    server_var: f64,
    rog_cost_pct: f64,
) -> f64 {
    let Some(upgrade) = data.upgrade(t) else {
        return f64::INFINITY;
    };
    let t = t as f64;
    let base = 5.0 + t + (t - 2.0).max(0.0).powf(1.3);
    let pow2 = 2f64.powf((t - 4.0).max(0.0));
    let sv = server_var.max(1.0).powf((t - 9.0).max(0.0));
    let cheapo = 1.0 / (1.0 + qty26 / 100.0);
    let rog = (1.0 - rog_cost_pct / 100.0).max(0.1);
    let exp = upgrade.cost_exp.powf(level);
    base * pow2 * sv * cheapo * rog * exp
}

/// Whether `level` more of upgrade `t` can be bought.
pub fn can_buy(
    data: &GameData,
    t: usize,
    level: f64,
    currency: f64,
    qty26: f64,
    server_var: f64,
    rog_cost_pct: f64,
) -> bool {
    let Some(upgrade) = data.upgrade(t) else {
        return false;
    };
    currency >= upg_cost(data, t, level, qty26, server_var, rog_cost_pct)
        && (level < upgrade.max_lv || upgrade.max_lv > 998.0)
}

/// The grid size at a Grid Expansion level.
pub fn grid_dims(data: &GameData, grid_exp_lv: usize) -> (usize, usize) {
    let index = grid_exp_lv.min(data.grid_dims.len().saturating_sub(1));
    let dims = data
        .grid_dims
        .get(index)
        .map(String::as_str)
        .unwrap_or("3,3");
    let mut parts = dims.split(',').map(|part| part.trim().parse().unwrap_or(0));
    (parts.next().unwrap_or(0), parts.next().unwrap_or(0))
}

/// The tiles on the grid at a Grid Expansion level.
pub fn total_tiles(data: &GameData, grid_exp_lv: usize) -> usize {
    let (cols, rows) = grid_dims(data, grid_exp_lv);
    cols * rows
}

/// `GoldTilesTotal`, including Jelly reward 45.
pub fn gold_tiles_total(data: &GameData, levels: &[f64], jelly_gold_tiles: f64) -> f64 {
    (upgrade_qty(data, 8, level(levels, 8), 1.0) + jelly_gold_tiles).round()
}

/// `BlocksTotal`.
pub fn blocks_total(data: &GameData, levels: &[f64]) -> f64 {
    upgrade_qty(data, 10, level(levels, 10), 1.0)
}

/// `FlagsTotal`.
pub fn flags_total(data: &GameData, levels: &[f64]) -> f64 {
    upgrade_qty(data, 20, level(levels, 20), 1.0).round()
}

/// `InstaRevealsTotal`.
pub fn insta_reveals_total(data: &GameData, levels: &[f64]) -> f64 {
    upgrade_qty(data, 16, level(levels, 16), 1.0).round()
}

/// `DailyTries`: `round(3 + Grid_Bonus(147, 1))`.
pub fn daily_tries(grid_bonus_147: f64) -> f64 {
    (3.0 + grid_bonus_147).round()
}

/// `MaxHP_You`: the lives one game starts with.
pub fn max_hp_you(data: &GameData, levels: &[f64]) -> f64 {
    (3.0 + upgrade_qty(data, 6, level(levels, 6), 1.0)).round()
}

/// `floorHP`: the boss's health on `floor`.
pub fn floor_hp(floor: usize, server_var: f64) -> f64 {
    let t = floor as f64;
    (5.0 + 2.0 * t + t * t)
        * 1.8f64.powf(t)
        * 1.85f64.powf(((t - 4.0).max(0.0) / 3.0).floor())
        * 4f64.powf(((t - 5.0).max(0.0) / 7.0).floor())
        * server_var.max(1.0).powf((t - 9.0).max(0.0))
}

/// Legend 40's Mines_Opp reduction: `min(3, ceil(points / 2))`.
pub fn legend40_mine_reduction(points: f64) -> f64 {
    (points / 2.0).ceil().min(3.0)
}

/// `MinesOnFloor`: how many depth charges one grid carries.
pub fn mines_on_floor(floor: usize, reduction: f64, legend40_pts: f64) -> f64 {
    let t = floor as f64;
    let red = reduction + legend40_mine_reduction(legend40_pts);
    (1.0 - red
        + (t / 3.0).floor()
        + (t / 7.0).floor()
        + (t / 13.0).floor()
        + (t / 15.0).floor().min(1.0)
        + (t / 17.0).floor())
    .clamp(1.0, 40.0)
    .round()
}

/// `BaseDMG`: the flat damage one tile's number multiplies.
#[allow(clippy::too_many_arguments)]
pub fn base_dmg(
    data: &GameData,
    levels: &[f64],
    grid_bonus_167: f64,
    sailing38: f64,
    dancing_coral_5: f64,
    jelly_damage_9: f64,
    legend40: f64,
) -> f64 {
    let flat = 1.0
        + upgrade_qty(data, 0, level(levels, 0), 1.0)
        + upgrade_qty(data, 7, level(levels, 7), 1.0)
        + upgrade_qty(data, 25, level(levels, 25), 1.0)
        + dancing_coral_5;
    let pct_mega = 1.0
        + (upgrade_qty(data, 4, level(levels, 4), 1.0)
            + upgrade_qty(data, 21, level(levels, 21), 1.0)
            + upgrade_qty(data, 27, level(levels, 27), 1.0))
            / 100.0;
    let grid = 1.0 + grid_bonus_167 / 100.0;
    let sail = 1.0 + 50.0 * sailing38 / 100.0;
    flat * pct_mega * grid * sail * (1.0 + legend40 / 100.0) * (1.0 + jelly_damage_9 / 100.0)
}

/// `BonusDMGperTilePCT`.
pub fn bonus_dmg_per_tile_pct(data: &GameData, levels: &[f64], grid_bonus_146: f64) -> f64 {
    upgrade_qty(data, 9, level(levels, 9), 1.0) + grid_bonus_146
}

/// `BluecrownMulti`.
pub fn bluecrown_multi(data: &GameData, levels: &[f64]) -> f64 {
    1.5 + upgrade_qty(data, 14, level(levels, 14), 1.0) / 100.0
}

/// `BluecrownOdds`.
pub fn bluecrown_odds(data: &GameData, levels: &[f64]) -> f64 {
    if upgrade_qty(data, 14, level(levels, 14), 1.0) == 0.0 {
        return 0.0;
    }
    (1.0 / 15.0 * (1.0 + upgrade_qty(data, 15, level(levels, 15), 1.0) / 100.0)).min(0.1)
}

/// `JackpotOdds`.
pub fn jackpot_odds(data: &GameData, levels: &[f64]) -> f64 {
    if upgrade_qty(data, 23, level(levels, 23), 1.0) == 0.0 {
        return 0.0;
    }
    0.01 * (1.0 + upgrade_qty(data, 23, level(levels, 23), 1.0) / 100.0)
}

/// `JackpotTiles`: how many tiles a jackpot cascades.
pub fn jackpot_tiles(data: &GameData, levels: &[f64]) -> f64 {
    (3.0 + upgrade_qty(data, 24, level(levels, 24), 1.0)).round()
}

/// Everything one turn's damage reads.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageInputs {
    pub grid_167: f64,
    pub grid_146: f64,
    pub wep_pow_dmg_pct: f64,
    pub sailing38: f64,
    pub dancing_coral_5: f64,
    pub jelly_damage_9: f64,
    pub legend40: f64,
}

/// The damage one turn commits: the seven-stage upstream pipeline.
pub fn current_outgoing_dmg(
    data: &GameData,
    revealed: &[u32],
    bluecrown_count: u32,
    is_last_life: bool,
    levels: &[f64],
    inputs: &DamageInputs,
) -> f64 {
    let mut add_sum = 0.0;
    let mut multi_prod = 1.0;
    let mut tile_count = 0.0;
    for value in revealed {
        let value = *value;
        if (1..10).contains(&value) {
            add_sum += f64::from(value);
        } else if (10..19).contains(&value) {
            add_sum += f64::from(value) + 1.0;
        } else if value == 19 {
            add_sum -= 1.0;
        } else if (20..29).contains(&value) {
            let index = (value - 20) as usize;
            multi_prod *= data.tile_multipliers.get(index).copied().unwrap_or(1.0);
        }
        tile_count += 1.0;
    }

    let mut dmg = add_sum
        * base_dmg(
            data,
            levels,
            inputs.grid_167,
            inputs.sailing38,
            inputs.dancing_coral_5,
            inputs.jelly_damage_9,
            inputs.legend40,
        );
    dmg *= multi_prod;
    dmg *= 1.0 + inputs.wep_pow_dmg_pct / 100.0;
    dmg *= 1.0 + tile_count * bonus_dmg_per_tile_pct(data, levels, inputs.grid_146) / 100.0;
    dmg *= bluecrown_multi(data, levels).powi(bluecrown_count as i32);
    if is_last_life {
        dmg *= 1.0 + upgrade_qty(data, 11, level(levels, 11), 1.0) / 100.0;
    }
    dmg
}

/// The account terms the currency formula multiplies in.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CurrencyInputs {
    pub grid_129: f64,
    pub grid_148: f64,
    pub grid_147: f64,
    pub grid_166: f64,
    pub comp_143: f64,
    pub comp_143_level2: f64,
    pub bonus_qty_6: f64,
    pub atom13: f64,
    pub meal_mine_curr: f64,
    pub arcade62: f64,
    pub rog_bonus_12: f64,
    pub button_bonus_1: f64,
    pub event_shop_44: f64,
    pub royal_currency_multi: f64,
    pub jelly_currency_8: f64,
    pub bundle_j: bool,
    pub task_currency_level: f64,
    pub dancing_coral_5: f64,
    pub legend40: f64,
    /// The QTY-equivalent upgrade levels.
    pub levels: Vec<f64>,
    pub highest_dmg: f64,
}

/// `CurrencyGain/hr`.
pub fn currency_per_hour(data: &GameData, inputs: &CurrencyInputs) -> f64 {
    let level = |t: usize| inputs.levels.get(t).copied().unwrap_or(0.0);
    let event_shop_multi = 1.0 + 100.0 * inputs.event_shop_44 / 100.0;
    let multi_148 = 1.0 + inputs.grid_148 / 100.0;
    let rog = 1.0 + inputs.rog_bonus_12 / 100.0;
    let comp = inputs.comp_143.clamp(1.0, 2.0) + inputs.comp_143_level2;
    let bq = (1.0 + inputs.bonus_qty_6 / 100.0).min(3.0);
    let log_dmg = if inputs.highest_dmg > 0.0 {
        log(inputs.highest_dmg)
    } else {
        0.0
    };
    let farm_pct = 1.0
        + (upgrade_qty(data, 5, level(5), 1.0)
            + upgrade_qty(data, 22, level(22), 1.0)
            + upgrade_qty(data, 28, level(28), 1.0) * log_dmg
            + inputs.dancing_coral_5)
            / 100.0;
    let atom = 1.0 + inputs.atom13 / 100.0;
    let button = 1.0 + inputs.button_bonus_1 / 100.0;
    let arcade = 1.0 + inputs.arcade62 / 100.0;
    let legend = 1.0 + inputs.legend40 / 100.0;
    let passive = 1.0 + (inputs.grid_147 + inputs.grid_166 + inputs.meal_mine_curr) / 100.0;
    let task = 1.0 + 10.0 * inputs.task_currency_level / 100.0;
    let jelly = 1.0 + inputs.jelly_currency_8 / 100.0;
    let bundle = 1.0 + if inputs.bundle_j { 1.0 } else { 0.0 };

    inputs.grid_129
        * event_shop_multi
        * multi_148
        * inputs.royal_currency_multi
        * rog
        * jelly
        * bundle
        * task
        * comp
        * bq
        * farm_pct
        * button
        * arcade
        * atom
        * legend
        * passive
}

/// `WiggleMaxPerGame`.
pub fn wiggle_max_per_game(grid_bonus_166: f64) -> f64 {
    grid_bonus_166.round()
}

/// `glimboCost`.
pub fn glimbo_cost(trade_lv: f64, cost_exp: f64, event_shop_38: f64) -> f64 {
    let raw = (1.0 + trade_lv + 1.5 * trade_lv)
        * cost_exp.powf(trade_lv)
        * (1.0 - 25.0 * event_shop_38 / 100.0).max(0.1);
    if raw < 1e9 { raw.max(1.0).floor() } else { raw }
}

/// The tile values the shape ranking grows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShapeContext {
    /// Grid 129, 148, 147, 166, 125, 167 and 146, in that order.
    pub grid: [f64; 7],
    pub meal_mine_curr: f64,
    /// Button slot 1's bonus without grid 125.
    pub button_no_grid: f64,
    /// Button slot 1's bonus with grid 125.
    pub button1: f64,
    /// `UpgradeQTY(9)`, the per-tile damage percentage.
    pub per_tile: f64,
    pub shape_pct: f64,
    pub tiles: f64,
}

/// One row of the shape ranking.
#[derive(Clone, Debug, PartialEq)]
pub struct ShapeValue {
    pub idx: usize,
    pub kind: ShapeKind,
    pub level: f64,
    pub bonus: f64,
    pub shaped_bonus: f64,
    pub ratio: f64,
    pub gain_pct: f64,
    pub factor: &'static str,
}

/// Whether a grid node feeds currency or damage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShapeKind {
    Currency,
    Damage,
}

/// `mineheadGridShapeValues`: the exact gain each node gets from a shape.
pub fn grid_shape_values(context: &ShapeContext) -> Vec<ShapeValue> {
    let p = context.shape_pct;
    let grown = |value: f64| value * (1.0 + p / 100.0);
    let [g129, g148, g147, g166, g125, g167, g146] = context.grid;
    let pool = g147 + g166 + context.meal_mine_curr;
    let button_shaped = context.button_no_grid * (1.0 + grown(g125) / 100.0);
    let row =
        |idx: usize, kind: ShapeKind, bonus: f64, ratio: f64, factor: &'static str| ShapeValue {
            idx,
            kind,
            level: 0.0,
            bonus,
            shaped_bonus: grown(bonus),
            ratio,
            gain_pct: (ratio - 1.0) * 100.0,
            factor,
        };
    vec![
        row(
            129,
            ShapeKind::Currency,
            g129,
            if g129 > 0.0 { 1.0 + p / 100.0 } else { 1.0 },
            "Base Minehead Currency/hr",
        ),
        row(
            148,
            ShapeKind::Currency,
            g148,
            (100.0 + grown(g148)) / (100.0 + g148),
            "×(1 + Grid/100)",
        ),
        row(
            147,
            ShapeKind::Currency,
            g147,
            (100.0 + pool - g147 + grown(g147)) / (100.0 + pool),
            "Shared pool with Grid 166 + meals",
        ),
        row(
            166,
            ShapeKind::Currency,
            g166,
            (100.0 + pool - g166 + grown(g166)) / (100.0 + pool),
            "Shared pool with Grid 147 + meals",
        ),
        row(
            125,
            ShapeKind::Currency,
            g125,
            (100.0 + button_shaped) / (100.0 + context.button1),
            "Button multi → Button_Bonuses(1)",
        ),
        row(
            167,
            ShapeKind::Damage,
            g167,
            (100.0 + grown(g167)) / (100.0 + g167),
            "Base damage multiplier",
        ),
        row(
            146,
            ShapeKind::Damage,
            g146,
            (100.0 + context.tiles * (context.per_tile + grown(g146)))
                / (100.0 + context.tiles * (context.per_tile + g146)),
            "Per-tile damage",
        ),
    ]
}

/// `mineheadShapePriority`: currency rows first, then damage, each by gain.
pub fn shape_priority(context: &ShapeContext) -> Vec<usize> {
    let rows = grid_shape_values(context);
    let order = |kind: ShapeKind| {
        let mut rows: Vec<(usize, f64)> = rows
            .iter()
            .filter(|row| row.kind == kind)
            .map(|row| (row.idx, row.ratio))
            .collect();
        rows.sort_by(|left, right| right.1.total_cmp(&left.1));
        rows.into_iter().map(|(idx, _)| idx).collect::<Vec<usize>>()
    };
    let mut priority = order(ShapeKind::Currency);
    priority.extend(order(ShapeKind::Damage));
    priority
}

/// The level at `index`, zero when the array is short.
fn level(levels: &[f64], index: usize) -> f64 {
    levels.get(index).copied().unwrap_or(0.0)
}

/// The game data, for callers that only need the table.
pub fn data() -> &'static GameData {
    game()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_upgrade_requirement_follows_the_research_level() {
        assert_eq!(upg_lv_req(0), 1);
        assert_eq!(upg_lv_req(3), 11);
        assert_eq!(upg_lv_req(11), 38);
    }

    #[test]
    fn doubling_an_upgrade_doubles_its_quantity() {
        let data = game();
        assert_eq!(upgrade_qty(data, 6, 4.0, 1.0), 4.0);
        assert_eq!(upgrade_qty(data, 6, 4.0, 2.0), 8.0);
        assert_eq!(qty_levels(&[3.0, 4.0], &[1])[1], 8.0);
    }

    #[test]
    fn costs_grow_with_the_level_and_shrink_with_discounts() {
        let data = game();
        let plain = upg_cost(data, 5, 10.0, 0.0, 1.0, 0.0);
        let later = upg_cost(data, 5, 11.0, 0.0, 1.0, 0.0);
        assert!(later > plain);
        let discounted = upg_cost(data, 5, 10.0, 100.0, 1.0, 50.0);
        assert!(discounted < plain);
    }

    #[test]
    fn the_grid_grows_to_twelve_by_six() {
        let data = game();
        assert_eq!(grid_dims(data, 0), (3, 3));
        assert_eq!(grid_dims(data, 16), (12, 6));
        assert_eq!(total_tiles(data, 16), 72);
        assert_eq!(grid_dims(data, 99), (12, 6));
    }
}
